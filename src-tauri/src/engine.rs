//! Transcription with whisper.cpp.
//!
//! 1. The sound is decoded to 16 kHz mono (media::decode).
//! 2. Silero VAD finds the speech. Only the speech is sent to Whisper, joined
//!    with short silences, which keeps the model from inventing text in long
//!    pauses and saves time. Every timestamp is mapped back to the original.
//! 3. Whisper runs with DTW token timestamps, so each word gets its own time
//!    and the subtitle layout can be redone freely afterwards.

use crate::media;
use crate::models::{self, Dtw, ModelSpec};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

/// Progress (stage, 0..1) and live text (start, end, text) callbacks.
pub type OnProgress = Arc<dyn Fn(&str, f64) + Send + Sync>;
pub type OnSegment = Arc<dyn Fn(f64, f64, String) + Send + Sync>;
use whisper_rs::{
    DtwMode, DtwModelPreset, DtwParameters, FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters,
    WhisperVadContext, WhisperVadContextParams, WhisperVadParams,
};

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub job_id: String,
    pub path: String,
    pub model_id: String,
    /// names and terms that help the model spell things right
    pub prompt: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Word {
    pub text: String,
    pub start: f64,
    pub end: f64,
    /// lowest token probability in the word, 0..1
    pub p: f32,
}

#[derive(Serialize, Clone, Debug)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub words: Vec<Word>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub language: String,
    pub model_id: String,
    pub model_name: String,
    pub duration: f64,
    pub segments: Vec<Segment>,
    pub elapsed: f64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress<'a> {
    job_id: &'a str,
    /// "loading", "decoding", "detecting", "transcribing"
    stage: &'a str,
    progress: f64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LiveSegment {
    job_id: String,
    start: f64,
    end: f64,
    text: String,
}

struct Loaded {
    id: String,
    ctx: Arc<WhisperContext>,
}

static LOADED: Mutex<Option<Loaded>> = Mutex::new(None);

pub fn backend() -> &'static str {
    if cfg!(target_os = "macos") {
        "Metal"
    } else if cfg!(feature = "cuda") {
        "CUDA"
    } else if cfg!(feature = "vulkan") {
        "Vulkan"
    } else {
        "CPU"
    }
}

fn load(m: &ModelSpec, path: &str) -> Result<Arc<WhisperContext>, String> {
    let mut slot = LOADED.lock().unwrap();
    if let Some(l) = slot.as_ref() {
        if l.id == m.id {
            return Ok(l.ctx.clone());
        }
    }
    // free the previous model before loading the next, they are large
    *slot = None;
    let preset = match m.dtw {
        Dtw::LargeV3 => DtwModelPreset::LargeV3,
        Dtw::LargeV3Turbo => DtwModelPreset::LargeV3Turbo,
        Dtw::Medium => DtwModelPreset::Medium,
    };
    let mut cp = WhisperContextParameters::default();
    cp.use_gpu(true);
    cp.flash_attn(false); // not compatible with DTW timestamps
    cp.dtw_parameters(DtwParameters { mode: DtwMode::ModelPreset { model_preset: preset }, ..Default::default() });
    let ctx = WhisperContext::new_with_params(path, cp).map_err(|e| format!("Could not load the model: {e:?}"))?;
    let ctx = Arc::new(ctx);
    *slot = Some(Loaded { id: m.id.to_string(), ctx: ctx.clone() });
    Ok(ctx)
}

pub fn unload() {
    *LOADED.lock().unwrap() = None;
}

/// A stretch of speech: where it sits in the audio sent to Whisper and where in the original.
#[derive(Clone, Copy, Debug)]
struct Piece {
    proc_start: f64,
    orig_start: f64,
    len: f64,
}

const GAP: f64 = 0.4; // silence put between pieces

fn map_time(t: f64, pieces: &[Piece]) -> f64 {
    if pieces.is_empty() {
        return t;
    }
    // last piece starting at or before t
    let i = pieces.partition_point(|p| p.proc_start <= t).saturating_sub(1);
    let p = pieces[i];
    p.orig_start + (t - p.proc_start).clamp(0.0, p.len)
}

fn speech_pieces(vad_path: Option<&std::path::Path>, audio: &[f32]) -> Result<Option<Vec<(f64, f64)>>, String> {
    let Some(path) = vad_path else { return Ok(None) };
    let mut cp = WhisperVadContextParams::new();
    cp.set_n_threads(num_threads());
    cp.set_use_gpu(false); // tiny model, CPU is fast and avoids a second GPU context
    let mut vad = WhisperVadContext::new(&path.to_string_lossy(), cp).map_err(|e| format!("VAD: {e:?}"))?;
    let mut vp = WhisperVadParams::new();
    vp.set_threshold(0.45);
    vp.set_min_speech_duration(200);
    vp.set_min_silence_duration(600);
    vp.set_speech_pad(250);
    let segs = vad.segments_from_samples(vp, audio).map_err(|e| format!("VAD: {e:?}"))?;
    let total = audio.len() as f64 / media::SAMPLE_RATE as f64;
    let mut out: Vec<(f64, f64)> = Vec::new();
    for s in segs {
        let (a, b) = ((s.start as f64 / 100.0).max(0.0), (s.end as f64 / 100.0).min(total));
        if b <= a {
            continue;
        }
        match out.last_mut() {
            // keep natural pauses up to 1.5 s as they are
            Some(last) if a - last.1 < 1.5 => last.1 = b,
            _ => out.push((a, b)),
        }
    }
    Ok(Some(out))
}

fn num_threads() -> i32 {
    std::thread::available_parallelism().map(|n| n.get().min(8) as i32).unwrap_or(4)
}

pub fn transcribe(app: &AppHandle, job: &Job, cancel: Arc<AtomicBool>) -> Result<Transcript, String> {
    let m = models::spec(&job.model_id).ok_or("Unknown model")?;
    if !models::is_installed(app, m) {
        return Err(format!("{} is not downloaded yet.", m.name));
    }
    let model_path = models::model_path(app, m)?;
    let vad_path = models::spec("vad")
        .filter(|v| models::is_installed(app, v))
        .and_then(|v| models::model_path(app, v).ok());
    let (a1, a2) = (app.clone(), app.clone());
    let (id1, id2) = (job.job_id.clone(), job.job_id.clone());
    let on_progress: OnProgress = Arc::new(move |stage, progress| {
        let _ = a1.emit("transcribe-progress", Progress { job_id: &id1, stage, progress });
    });
    let on_segment: OnSegment = Arc::new(move |start, end, text| {
        let _ = a2.emit("transcribe-segment", LiveSegment { job_id: id2.clone(), start, end, text });
    });
    run(m, &model_path, vad_path.as_deref(), job, cancel, on_progress, on_segment)
}

/// The transcription itself, independent of the app (also used by the CLI example).
pub fn run(
    m: &ModelSpec,
    model_path: &std::path::Path,
    vad_path: Option<&std::path::Path>,
    job: &Job,
    cancel: Arc<AtomicBool>,
    on_progress: OnProgress,
    on_segment: OnSegment,
) -> Result<Transcript, String> {
    let started = std::time::Instant::now();
    let emit = |stage: &str, progress: f64| on_progress(stage, progress);

    let info = media::probe(&job.path)?;

    emit("decoding", 0.0);
    let audio = media::decode(&job.path, &cancel, |secs| {
        if info.duration > 0.0 {
            emit("decoding", (secs / info.duration).min(1.0));
        }
    })?;
    let total = audio.len() as f64 / media::SAMPLE_RATE as f64;

    emit("detecting", 0.0);
    let speech = speech_pieces(vad_path, &audio)?;
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled".into());
    }

    // Build the audio Whisper hears, and the map back to the original.
    let sr = media::SAMPLE_RATE as f64;
    let (input, pieces): (Vec<f32>, Vec<Piece>) = match &speech {
        None => (audio.clone(), vec![Piece { proc_start: 0.0, orig_start: 0.0, len: total }]),
        Some(list) if list.is_empty() => {
            return Ok(Transcript {
                language: m.language.into(),
                model_id: m.id.into(),
                model_name: m.name.into(),
                duration: total,
                segments: vec![],
                elapsed: started.elapsed().as_secs_f64(),
            })
        }
        Some(list) => {
            let mut buf = Vec::with_capacity(audio.len());
            let mut pieces = Vec::new();
            for &(a, b) in list {
                let (ia, ib) = ((a * sr) as usize, ((b * sr) as usize).min(audio.len()));
                if ib <= ia {
                    continue;
                }
                pieces.push(Piece { proc_start: buf.len() as f64 / sr, orig_start: a, len: (ib - ia) as f64 / sr });
                buf.extend_from_slice(&audio[ia..ib]);
                buf.extend(std::iter::repeat_n(0.0f32, (GAP * sr) as usize));
            }
            (buf, pieces)
        }
    };
    drop(audio);

    emit("loading", 0.0);
    let ctx = load(m, &model_path.to_string_lossy())?;
    let mut state = ctx.create_state().map_err(|e| format!("Could not start the model: {e:?}"))?;

    let accurate = m.quality == "accurate";
    let strategy = if accurate {
        SamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 }
    } else {
        SamplingStrategy::Greedy { best_of: 5 }
    };
    let mut params = FullParams::new(strategy);
    params.set_n_threads(num_threads());
    params.set_language(Some(m.language));
    params.set_translate(false);
    params.set_token_timestamps(true);
    params.set_suppress_nst(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    let prompt = job.prompt.clone().unwrap_or_default();
    if !prompt.trim().is_empty() {
        params.set_initial_prompt(prompt.trim());
    }

    {
        let on_progress = on_progress.clone();
        params.set_progress_callback_safe(move |p: i32| on_progress("transcribing", (p as f64 / 100.0).clamp(0.0, 1.0)));
    }
    {
        let pieces = pieces.clone();
        let on_segment = on_segment.clone();
        params.set_segment_callback_safe_lossy(move |d: whisper_rs::SegmentCallbackData| {
            on_segment(
                map_time(d.start_timestamp as f64 / 100.0, &pieces),
                map_time(d.end_timestamp as f64 / 100.0, &pieces),
                d.text.trim().to_string(),
            )
        });
    }
    {
        let cancel = cancel.clone();
        params.set_abort_callback_safe(move || cancel.load(Ordering::Relaxed));
    }

    emit("transcribing", 0.0);
    let result = state.full(params, &input);
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled".into());
    }
    result.map_err(|e| format!("Transcription failed: {e:?}"))?;

    let eot = ctx.token_eot();
    let mut segments = Vec::new();
    for seg in state.as_iter() {
        let s_start = seg.start_timestamp() as f64 / 100.0;
        let s_end = seg.end_timestamp() as f64 / 100.0;
        // (bytes, start, last-token end, min p)
        let mut raw: Vec<(Vec<u8>, f64, f64, f32)> = Vec::new();
        for i in 0..seg.n_tokens() {
            let Some(tok) = seg.get_token(i) else { continue };
            if tok.token_id() >= eot {
                continue; // timestamps and other special tokens
            }
            let Ok(bytes) = tok.to_bytes() else { continue };
            if bytes.is_empty() {
                continue;
            }
            let d = tok.token_data();
            let t = if d.t_dtw >= 0 { d.t_dtw as f64 / 100.0 } else { d.t0 as f64 / 100.0 };
            let t_end = (d.t1 as f64 / 100.0).max(t);
            let starts_word = bytes[0] == b' ' || raw.is_empty();
            // punctuation and pieces of a word stay with the word they belong to
            if starts_word && !is_punct_only(bytes) {
                raw.push((bytes.to_vec(), t, t_end, tok.token_probability()));
            } else if let Some(last) = raw.last_mut() {
                last.0.extend_from_slice(bytes);
                last.2 = last.2.max(t_end);
                if !is_punct_only(bytes) {
                    last.3 = last.3.min(tok.token_probability());
                }
            } else {
                raw.push((bytes.to_vec(), t, t_end, tok.token_probability()));
            }
        }

        let mut words: Vec<Word> = Vec::new();
        for (k, (bytes, t, t_end, p)) in raw.iter().enumerate() {
            let text = String::from_utf8_lossy(bytes).trim().to_string();
            if text.is_empty() {
                continue;
            }
            let start = t.clamp(s_start, s_end);
            let next = raw.get(k + 1).map(|r| r.1.clamp(s_start, s_end)).unwrap_or(s_end);
            let chars = text.chars().count() as f64;
            let natural = start + 0.25 + 0.085 * chars;
            // whisper's token end times are unreliable; the next word's start and the
            // word's length are better guides
            let _ = t_end;
            let end = natural.min(next - 0.02).max(start + 0.05);
            words.push(Word { text, start, end, p: *p });
        }
        // map back to the original timeline
        for w in &mut words {
            w.start = map_time(w.start, &pieces);
            w.end = map_time(w.end, &pieces).max(w.start + 0.05);
        }
        if words.is_empty() || is_noise_segment(&words) {
            continue;
        }
        segments.push(Segment {
            start: words.first().unwrap().start,
            end: words.last().unwrap().end,
            words,
        });
    }

    Ok(Transcript {
        language: m.language.into(),
        model_id: m.id.into(),
        model_name: m.name.into(),
        duration: total,
        segments,
        elapsed: started.elapsed().as_secs_f64(),
    })
}

fn is_punct_only(bytes: &[u8]) -> bool {
    let s = String::from_utf8_lossy(bytes);
    let t = s.trim();
    !t.is_empty() && t.chars().all(|c| !c.is_alphanumeric())
}

/// Whisper's classic inventions in silence or music.
fn is_noise_segment(words: &[Word]) -> bool {
    let text: String = words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ").to_lowercase();
    let t = text.trim_matches(|c: char| !c.is_alphanumeric());
    const JUNK: &[&str] = &[
        "textning.nu",
        "svensktextning.nu",
        "undertexter från amara.org-gemenskapen",
        "thanks for watching",
        "thank you for watching",
        "subtitles by the amara.org community",
        "tack för att du tittade",
        "tack för att ni tittade",
    ];
    if JUNK.iter().any(|j| t.contains(j)) {
        return true;
    }
    // bracketed sound labels like [Musik] or (applåder)
    let raw: String = words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
    let r = raw.trim();
    (r.starts_with('[') && r.ends_with(']')) || (r.starts_with('(') && r.ends_with(')')) || r.chars().all(|c| c == '♪' || c.is_whitespace())
}

