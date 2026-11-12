//! The model catalogue and downloads.
//!
//! Models live in `<app data>/models`. A download goes to `<file>.part`
//! (resumed with an HTTP range request if it is interrupted), its SHA-256 is
//! checked against the catalogue, and only then is it renamed into place.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Dtw {
    LargeV3,
    LargeV3Turbo,
    Medium,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSpec {
    pub id: &'static str,
    /// "en" or "sv"; the language the model is used for
    pub language: &'static str,
    /// "accurate" or "fast"
    pub quality: &'static str,
    pub name: &'static str,
    pub repo: &'static str,
    pub file: &'static str,
    pub local_name: &'static str,
    pub size: u64,
    pub sha256: &'static str,
    #[serde(skip)]
    pub dtw: Dtw,
}

pub const MODELS: &[ModelSpec] = &[
    ModelSpec {
        id: "en-accurate",
        language: "en",
        quality: "accurate",
        name: "Whisper large-v3",
        repo: "ggerganov/whisper.cpp",
        file: "ggml-large-v3.bin",
        local_name: "openai-large-v3.bin",
        size: 3_095_033_483,
        sha256: "64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2",
        dtw: Dtw::LargeV3,
    },
    ModelSpec {
        id: "en-fast",
        language: "en",
        quality: "fast",
        name: "Whisper large-v3 turbo (q8)",
        repo: "ggerganov/whisper.cpp",
        file: "ggml-large-v3-turbo-q8_0.bin",
        local_name: "openai-large-v3-turbo-q8_0.bin",
        size: 874_188_075,
        sha256: "317eb69c11673c9de1e1f0d459b253999804ec71ac4c23c17ecf5fbe24e259a1",
        dtw: Dtw::LargeV3Turbo,
    },
    ModelSpec {
        id: "sv-accurate",
        language: "sv",
        quality: "accurate",
        name: "KB-Whisper large",
        repo: "KBLab/kb-whisper-large",
        file: "ggml-model.bin",
        local_name: "kb-whisper-large.bin",
        size: 3_095_033_483,
        sha256: "b66f2dda369a88f6c03fe37326d7cc37aa216f6f34e6fc1be686e497ba9c2f39",
        dtw: Dtw::LargeV3,
    },
    ModelSpec {
        id: "sv-fast",
        language: "sv",
        quality: "fast",
        name: "KB-Whisper medium (q5)",
        repo: "KBLab/kb-whisper-medium",
        file: "ggml-model-q5_0.bin",
        local_name: "kb-whisper-medium-q5_0.bin",
        size: 539_212_484,
        sha256: "7f8762e0ade9e0073674c0d5acae942a0b1ea98add9baa008ee89c94eaba43d0",
        dtw: Dtw::Medium,
    },
    // Voice activity detection, used with every model to skip silence.
    ModelSpec {
        id: "vad",
        language: "",
        quality: "",
        name: "Silero VAD v5.1.2",
        repo: "ggml-org/whisper-vad",
        file: "ggml-silero-v5.1.2.bin",
        local_name: "silero-v5.1.2.bin",
        size: 885_098,
        sha256: "29940d98d42b91fbd05ce489f3ecf7c72f0a42f027e4875919a28fb4c04ea2cf",
        dtw: Dtw::Medium,
    },
];

pub fn spec(id: &str) -> Option<&'static ModelSpec> {
    MODELS.iter().find(|m| m.id == id)
}

pub fn models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("models");
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    Ok(dir)
}

pub fn model_path(app: &AppHandle, m: &ModelSpec) -> Result<PathBuf, String> {
    Ok(models_dir(app)?.join(m.local_name))
}

pub fn is_installed(app: &AppHandle, m: &ModelSpec) -> bool {
    model_path(app, m)
        .ok()
        .and_then(|p| fs::metadata(p).ok())
        .map(|md| md.len() == m.size)
        .unwrap_or(false)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    #[serde(flatten)]
    pub spec: ModelSpec,
    pub installed: bool,
    /// bytes of an interrupted download that can be resumed
    pub partial: u64,
}

pub fn status(app: &AppHandle) -> Result<Vec<ModelStatus>, String> {
    let dir = models_dir(app)?;
    Ok(MODELS
        .iter()
        .map(|m| ModelStatus {
            spec: m.clone(),
            installed: is_installed(app, m),
            partial: fs::metadata(dir.join(format!("{}.part", m.local_name)))
                .map(|md| md.len())
                .unwrap_or(0),
        })
        .collect())
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress<'a> {
    id: &'a str,
    received: u64,
    total: u64,
    /// "downloading", "verifying", "done", "error", "cancelled"
    state: &'a str,
    bytes_per_sec: f64,
    message: Option<String>,
}

/// Download a model, blocking. Emits `model-download` events. Returns Ok(true)
/// when installed, Ok(false) when cancelled.
pub fn download(app: &AppHandle, id: &str, cancel: Arc<AtomicBool>) -> Result<bool, String> {
    let m = spec(id).ok_or_else(|| format!("Unknown model {id}"))?;
    if is_installed(app, m) {
        return Ok(true);
    }
    let result = download_inner(app, m, &cancel);
    let (state, message) = match &result {
        Ok(true) => ("done", None),
        Ok(false) => ("cancelled", None),
        Err(e) => ("error", Some(e.clone())),
    };
    let _ = app.emit(
        "model-download",
        DownloadProgress { id: m.id, received: 0, total: m.size, state, bytes_per_sec: 0.0, message },
    );
    result
}

fn download_inner(app: &AppHandle, m: &ModelSpec, cancel: &AtomicBool) -> Result<bool, String> {
    let final_path = model_path(app, m)?;
    let part_path = final_path.with_file_name(format!("{}.part", m.local_name));

    // Hash what an earlier attempt left behind so the checksum covers the whole file.
    let mut hasher = Sha256::new();
    let mut have = 0u64;
    if let Ok(mut f) = File::open(&part_path) {
        let mut buf = vec![0u8; 1 << 20];
        loop {
            let n = f.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            have += n as u64;
        }
        if have > m.size {
            fs::remove_file(&part_path).ok();
            hasher = Sha256::new();
            have = 0;
        }
    }

    let url = format!("https://huggingface.co/{}/resolve/main/{}?download=true", m.repo, m.file);
    let client = reqwest::blocking::Client::builder()
        .user_agent("PlumppuVox-Heankieli/0.1")
        .connect_timeout(Duration::from_secs(20))
        .timeout(None)
        .build()
        .map_err(|e| e.to_string())?;

    let mut req = client.get(&url);
    if have > 0 && have < m.size {
        req = req.header("Range", format!("bytes={have}-"));
    }
    let mut resp = if have == m.size {
        None
    } else {
        let r = req.send().map_err(|e| format!("Could not reach Hugging Face: {e}"))?;
        if !r.status().is_success() {
            return Err(format!("Download failed: HTTP {}", r.status()));
        }
        if have > 0 && r.status() != reqwest::StatusCode::PARTIAL_CONTENT {
            // the server ignored the range: start over
            hasher = Sha256::new();
            have = 0;
            fs::remove_file(&part_path).ok();
        }
        Some(r)
    };

    let mut out = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&part_path)
        .map_err(|e| format!("Could not write {}: {e}", part_path.display()))?;

    if let Some(resp) = resp.as_mut() {
        let mut buf = vec![0u8; 1 << 18];
        let mut last_emit = Instant::now() - Duration::from_secs(1);
        let started = Instant::now();
        let start_have = have;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Ok(false);
            }
            let n = resp.read(&mut buf).map_err(|e| format!("Download interrupted: {e}"))?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).map_err(|e| format!("Could not write model file: {e}"))?;
            hasher.update(&buf[..n]);
            have += n as u64;
            if last_emit.elapsed() > Duration::from_millis(200) {
                last_emit = Instant::now();
                let secs = started.elapsed().as_secs_f64().max(0.001);
                let _ = app.emit(
                    "model-download",
                    DownloadProgress {
                        id: m.id,
                        received: have,
                        total: m.size,
                        state: "downloading",
                        bytes_per_sec: (have - start_have) as f64 / secs,
                        message: None,
                    },
                );
            }
        }
    }
    out.flush().map_err(|e| e.to_string())?;
    drop(out);

    let _ = app.emit(
        "model-download",
        DownloadProgress { id: m.id, received: have, total: m.size, state: "verifying", bytes_per_sec: 0.0, message: None },
    );
    if have != m.size {
        return Err(format!("Download incomplete ({have} of {} bytes). Try again to resume.", m.size));
    }
    let digest = hex::encode(hasher.finalize());
    if digest != m.sha256 {
        fs::remove_file(&part_path).ok();
        return Err("The downloaded file was damaged (checksum mismatch). Please download again.".into());
    }
    fs::rename(&part_path, &final_path).map_err(|e| e.to_string())?;
    Ok(true)
}

pub fn delete(app: &AppHandle, id: &str) -> Result<(), String> {
    let m = spec(id).ok_or_else(|| format!("Unknown model {id}"))?;
    let p = model_path(app, m)?;
    for path in [p.clone(), p.with_file_name(format!("{}.part", m.local_name))] {
        if Path::new(&path).exists() {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
