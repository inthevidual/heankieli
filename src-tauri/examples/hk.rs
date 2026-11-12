//! Developer tool: transcribe a file from the command line with the app's engine.
//!
//!     cargo run --release --features cuda --example hk -- <model-id> <media> [models dir]
//!
//! Prints the transcript as JSON on stdout.

use heankieli_lib::{engine, models};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: hk <model-id> <media> [models dir]");
        std::process::exit(2);
    }
    let dir = args.get(3).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var("HOME").unwrap()).join(".local/share/com.plumppu.heankieli/models")
    });
    let m = models::spec(&args[1]).expect("unknown model id");
    let vad = models::spec("vad").map(|v| dir.join(v.local_name)).filter(|p| p.exists());
    let job = engine::Job { job_id: "cli".into(), path: args[2].clone(), model_id: m.id.into(), prompt: std::env::var("HK_PROMPT").ok() };
    let t = engine::run(
        m,
        &dir.join(m.local_name),
        vad.as_deref(),
        &job,
        Arc::new(AtomicBool::new(false)),
        Arc::new(|stage, p| eprint!("\r{stage} {:>3.0}%   ", p * 100.0)),
        Arc::new(|s, e, text| eprintln!("\n[{s:7.2} – {e:7.2}] {text}")),
    )
    .unwrap_or_else(|e| {
        eprintln!("\nerror: {e}");
        std::process::exit(1)
    });
    eprintln!("\ndone in {:.1}s, {} segments", t.elapsed, t.segments.len());
    println!("{}", serde_json::to_string_pretty(&t).unwrap());
}
