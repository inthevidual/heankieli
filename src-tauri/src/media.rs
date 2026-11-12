//! Reading media with ffmpeg: probing, decoding to 16 kHz mono for the model,
//! and making a preview copy the web view can play when it cannot play the
//! original (Linux WebKit without the libav GStreamer plugin plays neither
//! H.264 nor AAC, for example).

use serde::Serialize;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub const SAMPLE_RATE: u32 = 16_000;

fn find_tool(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    // the copy bundled with the app (macOS and Windows) sits next to the executable
    if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
        let p = dir.join(&exe);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let p = dir.join(&exe);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    // A Mac app started from Finder does not get the shell's PATH.
    for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin", "/usr/bin"] {
        let p = Path::new(dir).join(&exe);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// A command for ffmpeg/ffprobe; on Windows without a console window popping up.
fn tool(path: PathBuf) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

pub fn ffmpeg() -> Result<PathBuf, String> {
    find_tool("ffmpeg").ok_or_else(|| "ffmpeg was not found. Install it (`brew install ffmpeg` on a Mac, `winget install ffmpeg` on Windows, or your package manager) and restart.".into())
}

pub fn ffprobe() -> Result<PathBuf, String> {
    find_tool("ffprobe").ok_or_else(|| "ffprobe was not found. It comes with ffmpeg; install ffmpeg and restart.".into())
}

#[derive(Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub path: String,
    pub name: String,
    pub duration: f64,
    pub has_video: bool,
    pub has_audio: bool,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub video_codec: String,
    pub audio_codec: String,
    pub container: String,
}

pub fn probe(path: &str) -> Result<MediaInfo, String> {
    let out = tool(ffprobe()?)
        .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams", path])
        .output()
        .map_err(|e| format!("Could not run ffprobe: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "Could not read {}: {}",
            Path::new(path).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let mut info = MediaInfo {
        path: path.to_string(),
        name: Path::new(path).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
        ..Default::default()
    };
    info.duration = v["format"]["duration"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    info.container = v["format"]["format_name"].as_str().unwrap_or("").to_string();
    for s in v["streams"].as_array().cloned().unwrap_or_default() {
        let disposition_pic = s["disposition"]["attached_pic"].as_i64() == Some(1);
        match s["codec_type"].as_str() {
            Some("video") if !info.has_video && !disposition_pic => {
                info.has_video = true;
                info.width = s["width"].as_u64().unwrap_or(0) as u32;
                info.height = s["height"].as_u64().unwrap_or(0) as u32;
                info.video_codec = s["codec_name"].as_str().unwrap_or("").to_string();
                info.fps = parse_rate(s["avg_frame_rate"].as_str().unwrap_or(""))
                    .or_else(|| parse_rate(s["r_frame_rate"].as_str().unwrap_or("")))
                    .unwrap_or(25.0);
            }
            Some("audio") if !info.has_audio => {
                info.has_audio = true;
                info.audio_codec = s["codec_name"].as_str().unwrap_or("").to_string();
                if info.duration == 0.0 {
                    info.duration = s["duration"].as_str().and_then(|d| d.parse().ok()).unwrap_or(0.0);
                }
            }
            _ => {}
        }
    }
    if !info.has_audio {
        return Err(format!("{} has no sound to transcribe.", info.name));
    }
    Ok(info)
}

fn parse_rate(s: &str) -> Option<f64> {
    let (n, d) = s.split_once('/')?;
    let (n, d): (f64, f64) = (n.parse().ok()?, d.parse().ok()?);
    (d > 0.0 && n > 0.0).then(|| n / d)
}

/// Decode the first audio stream to 16 kHz mono f32. `on_progress` gets the
/// position in seconds as decoding goes.
pub fn decode(path: &str, cancel: &AtomicBool, mut on_progress: impl FnMut(f64)) -> Result<Vec<f32>, String> {
    let mut child = tool(ffmpeg()?)
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-i", path, "-map", "0:a:0", "-vn", "-sn", "-dn"])
        .args(["-ac", "1", "-ar", &SAMPLE_RATE.to_string(), "-f", "f32le", "-acodec", "pcm_f32le", "pipe:1"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not run ffmpeg: {e}"))?;

    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let err_thread = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });

    let mut bytes = Vec::with_capacity(1 << 24);
    let mut buf = vec![0u8; 1 << 16];
    let mut last_report = 0usize;
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("cancelled".into());
        }
        let n = stdout.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..n]);
        if bytes.len() - last_report > (SAMPLE_RATE as usize) * 4 * 30 {
            last_report = bytes.len();
            on_progress(bytes.len() as f64 / 4.0 / SAMPLE_RATE as f64);
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let err = err_thread.join().unwrap_or_default();
    if !status.success() {
        return Err(format!("ffmpeg could not decode the sound: {}", err.trim()));
    }
    let samples: Vec<f32> = bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    if samples.is_empty() {
        return Err("The file's sound track is empty.".into());
    }
    Ok(samples)
}

/// The running preview conversion, so it can be cancelled.
pub static PROXY_CHILD: Mutex<Option<Child>> = Mutex::new(None);

/// Make a copy of `path` the web view can play: VP8/Opus WebM on Linux (decoded by
/// GStreamer's own vpx and opus plugins), H.264/AAC MP4 on macOS.
pub fn make_proxy(path: &str, info: &MediaInfo, out_dir: &Path, mut on_progress: impl FnMut(f64)) -> Result<PathBuf, String> {
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let mac = cfg!(target_os = "macos");
    let ext = if mac { "mp4" } else { "webm" };

    // Name by path, size and mtime so a changed file gets a new copy.
    let md = std::fs::metadata(path).map_err(|e| e.to_string())?;
    let mtime = md.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    let key = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(path.as_bytes());
        h.update(md.len().to_le_bytes());
        h.update(mtime.to_le_bytes());
        hex::encode(&h.finalize()[..10])
    };
    let out = out_dir.join(format!("{key}.{ext}"));
    if out.exists() {
        return Ok(out);
    }
    let tmp = out_dir.join(format!("{key}.part.{ext}"));

    let mut cmd = tool(ffmpeg()?);
    cmd.args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i", path, "-map", "0:a:0"]);
    if info.has_video {
        cmd.args(["-map", "0:v:0", "-vf", "scale=-2:'min(720,ih)'", "-r", &format!("{:.3}", info.fps.min(60.0))]);
        if mac {
            cmd.args(["-c:v", "libx264", "-preset", "veryfast", "-crf", "26", "-pix_fmt", "yuv420p"]);
        } else {
            cmd.args(["-c:v", "libvpx", "-deadline", "realtime", "-cpu-used", "8", "-b:v", "2M", "-threads", "8"]);
        }
    } else {
        cmd.arg("-vn");
    }
    if mac {
        cmd.args(["-c:a", "aac", "-b:a", "128k", "-movflags", "+faststart"]);
    } else {
        cmd.args(["-c:a", "libopus", "-b:a", "96k"]);
    }
    cmd.args(["-progress", "pipe:1", "-nostats"]).arg(&tmp);
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not run ffmpeg: {e}"))?;
    let stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    *PROXY_CHILD.lock().unwrap() = Some(child);

    let err_thread = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if let Some(us) = line.strip_prefix("out_time_us=").and_then(|v| v.parse::<f64>().ok()) {
            if info.duration > 0.0 {
                on_progress((us / 1e6 / info.duration).clamp(0.0, 1.0));
            }
        }
    }
    let child = PROXY_CHILD.lock().unwrap().take();
    let ok = match child {
        Some(mut c) => c.wait().map(|s| s.success()).unwrap_or(false),
        None => false, // killed by cancel_proxy
    };
    let err = err_thread.join().unwrap_or_default();
    if !ok {
        let _ = std::fs::remove_file(&tmp);
        return Err(if err.trim().is_empty() { "cancelled".into() } else { format!("Could not make a preview: {}", err.trim()) });
    }
    std::fs::rename(&tmp, &out).map_err(|e| e.to_string())?;
    Ok(out)
}

pub fn cancel_proxy() {
    if let Some(mut c) = PROXY_CHILD.lock().unwrap().take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}

/// Peak levels for drawing a waveform: `per_second` values per second, 0..255.
pub fn peaks(path: &str, per_second: u32) -> Result<Vec<u8>, String> {
    let never = AtomicBool::new(false);
    let audio = decode(path, &never, |_| {})?;
    let hop = (SAMPLE_RATE / per_second).max(1) as usize;
    let mut out = Vec::with_capacity(audio.len() / hop + 1);
    let mut max_all = 1e-4f32;
    let raw: Vec<f32> = audio
        .chunks(hop)
        .map(|c| {
            let m = c.iter().fold(0.0f32, |a, &x| a.max(x.abs()));
            max_all = max_all.max(m);
            m
        })
        .collect();
    for m in raw {
        // a gentle curve so quiet speech is still visible
        out.push(((m / max_all).sqrt() * 255.0).round() as u8);
    }
    Ok(out)
}
