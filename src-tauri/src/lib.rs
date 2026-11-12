pub mod engine;
pub mod media;
pub mod models;
mod serve;

use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

static DOWNLOADS: Mutex<Option<HashMap<String, Arc<AtomicBool>>>> = Mutex::new(None);
static TRANSCRIBE_CANCEL: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: String,
    backend: &'static str,
    locale: String,
    platform: &'static str,
    ffmpeg: Option<String>,
    models_dir: String,
}

#[tauri::command]
fn app_info(app: AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        backend: engine::backend(),
        locale: sys_locale::get_locale().unwrap_or_else(|| "en".into()),
        platform: std::env::consts::OS,
        ffmpeg: media::ffmpeg().ok().map(|p| p.to_string_lossy().to_string()),
        models_dir: models::models_dir(&app).map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
    }
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct Startup {
    files: Vec<String>,
    lang: Option<String>,
    quality: Option<String>,
    start: bool,
    at: Option<f64>,
    ui: Option<String>,
}

/// Files and options from the command line:
///   heankieli [--ui sv|en] [--lang sv|en] [--fast|--accurate] [--start] [--at SECONDS] FILE...
/// ("Open with…" from a file manager passes just the files.)
#[tauri::command]
fn startup() -> Startup {
    let mut s = Startup::default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--lang" => s.lang = args.next().filter(|l| l == "sv" || l == "en"),
            "--fast" => s.quality = Some("fast".into()),
            "--accurate" => s.quality = Some("accurate".into()),
            "--start" => s.start = true,
            "--ui" => s.ui = args.next().filter(|l| l == "sv" || l == "en"),
            "--at" => s.at = args.next().and_then(|v| v.parse().ok()),
            _ if !a.starts_with('-') && std::path::Path::new(&a).is_file() => {
                if let Ok(p) = std::fs::canonicalize(&a) {
                    let p = p.to_string_lossy().to_string();
                    s.files.push(p.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(p));
                }
            }
            _ => {}
        }
    }
    s
}

/// Messages from the web view, for diagnosing problems.
#[tauri::command]
fn log(msg: String) {
    eprintln!("[ui] {msg}");
}

/// A loopback URL the player can stream the file from.
#[tauri::command]
fn media_url(path: String) -> Result<String, String> {
    serve::url_for(&path)
}

#[tauri::command]
fn list_models(app: AppHandle) -> Result<Vec<models::ModelStatus>, String> {
    models::status(&app)
}

#[tauri::command]
async fn download_model(app: AppHandle, id: String) -> Result<bool, String> {
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut d = DOWNLOADS.lock().unwrap();
        let map = d.get_or_insert_with(HashMap::new);
        if map.contains_key(&id) {
            return Err("Already downloading".into());
        }
        map.insert(id.clone(), cancel.clone());
    }
    let id2 = id.clone();
    let res = tauri::async_runtime::spawn_blocking(move || models::download(&app, &id2, cancel))
        .await
        .map_err(|e| e.to_string())?;
    if let Some(map) = DOWNLOADS.lock().unwrap().as_mut() {
        map.remove(&id);
    }
    res
}

#[tauri::command]
fn cancel_download(id: String) {
    if let Some(map) = DOWNLOADS.lock().unwrap().as_ref() {
        if let Some(c) = map.get(&id) {
            c.store(true, Ordering::Relaxed);
        }
    }
}

#[tauri::command]
fn delete_model(app: AppHandle, id: String) -> Result<(), String> {
    engine::unload();
    models::delete(&app, &id)
}

#[tauri::command]
async fn probe_media(path: String) -> Result<media::MediaInfo, String> {
    tauri::async_runtime::spawn_blocking(move || media::probe(&path)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn transcribe(app: AppHandle, job: engine::Job) -> Result<engine::Transcript, String> {
    let cancel = Arc::new(AtomicBool::new(false));
    *TRANSCRIBE_CANCEL.lock().unwrap() = Some(cancel.clone());
    let res = tauri::async_runtime::spawn_blocking(move || engine::transcribe(&app, &job, cancel))
        .await
        .map_err(|e| e.to_string())?;
    *TRANSCRIBE_CANCEL.lock().unwrap() = None;
    res
}

#[tauri::command]
fn cancel_transcription() {
    if let Some(c) = TRANSCRIBE_CANCEL.lock().unwrap().as_ref() {
        c.store(true, Ordering::Relaxed);
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PreviewProgress {
    path: String,
    progress: f64,
}

#[tauri::command]
async fn make_preview(app: AppHandle, path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let info = media::probe(&path)?;
        let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("previews");
        let out = media::make_proxy(&path, &info, &dir, |p| {
            let _ = app.emit("preview-progress", PreviewProgress { path: path.clone(), progress: p });
        })?;
        Ok(out.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn waveform(path: String) -> Result<tauri::ipc::Response, String> {
    let peaks = tauri::async_runtime::spawn_blocking(move || media::peaks(&path, 50))
        .await
        .map_err(|e| e.to_string())??;
    Ok(tauri::ipc::Response::new(peaks))
}

#[tauri::command]
fn cancel_preview() {
    media::cancel_proxy();
}

fn project_file(app: &AppHandle, media_path: &str) -> Result<PathBuf, String> {
    use sha2::{Digest, Sha256};
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("projects");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let key = hex::encode(&Sha256::digest(media_path.as_bytes())[..12]);
    Ok(dir.join(format!("{key}.json")))
}

/// Edits are kept per media file, so closing the app loses nothing.
#[tauri::command]
fn save_project(app: AppHandle, media_path: String, data: String) -> Result<(), String> {
    let p = project_file(&app, &media_path)?;
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, data).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_project(app: AppHandle, media_path: String) -> Result<Option<String>, String> {
    let p = project_file(&app, &media_path)?;
    match std::fs::read_to_string(&p) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
fn delete_project(app: AppHandle, media_path: String) -> Result<(), String> {
    let p = project_file(&app, &media_path)?;
    if p.exists() {
        std::fs::remove_file(p).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            app_info,
            media_url,
            log,
            startup,
            list_models,
            download_model,
            cancel_download,
            delete_model,
            probe_media,
            transcribe,
            cancel_transcription,
            make_preview,
            cancel_preview,
            waveform,
            save_project,
            load_project,
            delete_project,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PlumppuVox Heänkieli");
}
