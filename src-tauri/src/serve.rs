//! A tiny HTTP server on the loopback interface that streams media files to
//! the player. WebKitGTK's media pipeline cannot read Tauri's custom URI
//! schemes, but it plays http:// with range requests everywhere. Only files
//! the app has registered are served, each under a random token.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

struct Server {
    port: u16,
    files: Mutex<HashMap<String, PathBuf>>,
}

static SERVER: OnceLock<Server> = OnceLock::new();

fn server() -> Result<&'static Server, String> {
    if let Some(s) = SERVER.get() {
        return Ok(s);
    }
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let s = SERVER.get_or_init(|| Server { port, files: Mutex::new(HashMap::new()) });
    if s.port == port {
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                std::thread::spawn(move || {
                    let _ = handle(stream);
                });
            }
        });
    }
    Ok(s)
}

fn token() -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(format!("{:?}{:?}", std::time::SystemTime::now(), std::thread::current().id()).as_bytes());
    h.update(std::process::id().to_le_bytes());
    let addr = &h as *const _ as usize;
    h.update(addr.to_le_bytes());
    hex::encode(&h.finalize()[..16])
}

/// Register a file and return the URL the player can load it from.
pub fn url_for(path: &str) -> Result<String, String> {
    let s = server()?;
    let mut files = s.files.lock().unwrap();
    let existing = files.iter().find(|(_, p)| p.to_string_lossy() == path).map(|(k, _)| k.clone());
    let tok = existing.unwrap_or_else(|| {
        let t = token();
        files.insert(t.clone(), PathBuf::from(path));
        t
    });
    let name = std::path::Path::new(path)
        .extension()
        .map(|e| format!("media.{}", e.to_string_lossy()))
        .unwrap_or_else(|| "media".into());
    Ok(format!("http://127.0.0.1:{}/{tok}/{name}", s.port))
}

fn mime(path: &PathBuf) -> &'static str {
    match path.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() {
        Some("mp4") | Some("m4v") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("webm") => "video/webm",
        Some("mkv") => "video/x-matroska",
        Some("ogg") | Some("ogv") => "video/ogg",
        Some("opus") => "audio/ogg",
        Some("mp3") => "audio/mpeg",
        Some("m4a") | Some("aac") => "audio/mp4",
        Some("wav") => "audio/wav",
        Some("flac") => "audio/flac",
        _ => "application/octet-stream",
    }
}

fn handle(stream: TcpStream) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut out = stream;
    loop {
        // request line
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(());
        }
        let mut parts = line.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let target = parts.next().unwrap_or("").to_string();
        let mut range: Option<String> = None;
        let mut keep_alive = true;
        loop {
            let mut h = String::new();
            if reader.read_line(&mut h)? == 0 {
                return Ok(());
            }
            let h = h.trim_end();
            if h.is_empty() {
                break;
            }
            if let Some((k, v)) = h.split_once(':') {
                match k.trim().to_ascii_lowercase().as_str() {
                    "range" => range = Some(v.trim().to_string()),
                    "connection" if v.trim().eq_ignore_ascii_case("close") => keep_alive = false,
                    _ => {}
                }
            }
        }

        let tok = target.trim_start_matches('/').split('/').next().unwrap_or("");
        let path = SERVER.get().and_then(|s| s.files.lock().unwrap().get(tok).cloned());
        let Some(path) = path.filter(|_| method == "GET" || method == "HEAD") else {
            out.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")?;
            return Ok(());
        };
        let mut f = match File::open(&path) {
            Ok(f) => f,
            Err(_) => {
                out.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")?;
                return Ok(());
            }
        };
        let size = f.metadata()?.len();
        let (start, end, partial) = match range.as_deref().and_then(|r| r.strip_prefix("bytes=")) {
            Some(r) => {
                let (a, b) = r.split_once('-').unwrap_or((r, ""));
                let (start, end) = if a.is_empty() {
                    // suffix range: the last n bytes
                    let n: u64 = b.parse().unwrap_or(0);
                    (size.saturating_sub(n), size.saturating_sub(1))
                } else {
                    let s: u64 = a.parse().unwrap_or(0);
                    let e: u64 = b.parse().unwrap_or(size.saturating_sub(1)).min(size.saturating_sub(1));
                    (s, e)
                };
                if start >= size || start > end {
                    let msg = format!("HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{size}\r\nContent-Length: 0\r\n\r\n");
                    out.write_all(msg.as_bytes())?;
                    continue;
                }
                (start, end, true)
            }
            None => (0, size.saturating_sub(1), false),
        };
        let len = if size == 0 { 0 } else { end - start + 1 };
        let mut head = if partial {
            format!("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{end}/{size}\r\n")
        } else {
            "HTTP/1.1 200 OK\r\n".to_string()
        };
        head += &format!(
            "Content-Type: {}\r\nContent-Length: {len}\r\nAccept-Ranges: bytes\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-cache\r\nConnection: {}\r\n\r\n",
            mime(&path),
            if keep_alive { "keep-alive" } else { "close" }
        );
        out.write_all(head.as_bytes())?;
        if method == "GET" && len > 0 {
            f.seek(SeekFrom::Start(start))?;
            let mut left = len;
            let mut buf = vec![0u8; 256 * 1024];
            while left > 0 {
                let cap = left.min(buf.len() as u64) as usize;
                let n = f.read(&mut buf[..cap])?;
                if n == 0 {
                    break;
                }
                out.write_all(&buf[..n])?; // the player closing the connection ends this
                left -= n as u64;
            }
        }
        out.flush()?;
        if !keep_alive {
            return Ok(());
        }
    }
}
