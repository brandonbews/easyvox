//! transcribe.cpp adapter (ggml, Metal by default): Parakeet TDT/Unified and Whisper GGUF models,
//! all decoded offline with `Session::run`.
//!
//! ggml-metal behaviours to know about (transcribe-cpp-sys 0.3.1, ggml 0.25.3):
//! - **Exit abort.** Quitting with a model loaded aborts in a C++ static destructor; the
//!   `atexit` hook below fixes it.
//! - **First load per app identity** compiles the Metal shaders into a per-bundle cache
//!   (~14 s once, ~0.2 s afterwards). The app should warm this up during onboarding.
//! - **Idle wake-ups.** ggml-metal's residency-set loop wakes the process ~210 times a second
//!   for as long as its Metal device exists, even after every model is unloaded.
//!   `GGML_METAL_NO_RESIDENCY=1` stops it (3.5 wake-ups/s measured); its effect on the first
//!   decode after a long idle is unmeasured. See `ggml-metal-device.m` ~lines 954–1003.

use crate::models::ModelInfo;
use crate::{DecodeMode, EngineConfig, Hypothesis, SpeechModel, Word, spread_words};
use anyhow::{Context, bail};
use std::sync::{Arc, Mutex};
use transcribe_cpp::{Model, RunOptions, Session, SessionOptions, TimestampKind};

pub struct TcppModel {
    info: &'static ModelInfo,
    native: Arc<Native>,
    /// The finest timings the family gives: words for Parakeet, segments for Whisper.
    max_timestamps: TimestampKind,
}

/// The native handles, kept in `Option`s so they can be released before process exit.
struct Native {
    model: Mutex<Option<Model>>,
    session: Mutex<Option<Session>>,
}

/// ggml-metal 0.25 asserts (`[rsets->data count] == 0`, SIGABRT, exit code 134) in a C++
/// static destructor if a model is still loaded when the process exits; apps that quit with
/// the engine in global state would crash on every quit. We register an `atexit` hook after
/// the first load (so it runs before ggml's destructor) that frees every live model.
static LIVE: Mutex<Vec<std::sync::Weak<Native>>> = Mutex::new(Vec::new());
static ATEXIT: std::sync::Once = std::sync::Once::new();

extern "C" fn release_at_exit() {
    release_native_models();
}

/// Frees every loaded transcribe.cpp model now; engines using them return errors afterwards.
/// Runs automatically at process exit; call it yourself before `_exit`-style shutdowns.
pub fn release_native_models() {
    let Ok(live) = LIVE.try_lock() else { return };
    for n in live.iter().filter_map(|w| w.upgrade()) {
        if let Ok(mut s) = n.session.try_lock() {
            s.take();
        }
        if let Ok(mut m) = n.model.try_lock() {
            m.take();
        }
    }
}

impl TcppModel {
    pub fn load(info: &'static ModelInfo, cfg: &EngineConfig) -> anyhow::Result<Self> {
        let path = info.resolve(&cfg.models_dir);
        if !path.exists() {
            bail!("missing model file {}", path.display());
        }
        transcribe_cpp::disable_logging(); // silence per-decode timing lines on stderr
        let model =
            Model::load(&path).map_err(|e| anyhow::anyhow!("transcribe.cpp could not load {}: {e}", path.display()))?;
        let session = model
            .session_with(&SessionOptions { n_threads: cfg.threads as i32, ..Default::default() })
            .map_err(|e| anyhow::anyhow!("transcribe.cpp session: {e}"))?;
        let max_timestamps = model.capabilities().max_timestamp_kind;
        let native = Arc::new(Native { model: Mutex::new(Some(model)), session: Mutex::new(Some(session)) });
        {
            let mut live = LIVE.lock().unwrap();
            live.retain(|w| w.strong_count() > 0);
            live.push(Arc::downgrade(&native));
        }
        ATEXIT.call_once(|| unsafe {
            libc::atexit(release_at_exit);
        });
        Ok(Self { info, native, max_timestamps })
    }
}

impl SpeechModel for TcppModel {
    fn info(&self) -> &ModelInfo {
        self.info
    }

    fn transcribe(&self, pcm: &[f32], _mode: DecodeMode) -> anyhow::Result<Hypothesis> {
        if pcm.len() < 1600 {
            return Ok(Hypothesis::default());
        }
        let timestamps = match self.max_timestamps {
            // Parakeet: word timings come free with the decode.
            TimestampKind::Word | TimestampKind::Token => TimestampKind::Word,
            // Whisper has segment timings only, and needs them past 30 s: with no timestamps,
            // its seek loop moves on a full 30 s window even when the decoder stopped early,
            // silently dropping the rest of that window. Within one window it does better
            // without (they changed its output on our clips), and the session doesn't need
            // them: it spreads the words, and joins Whisper's chunks without context.
            TimestampKind::Segment if pcm.len() > 30 * 16_000 => TimestampKind::Segment,
            _ => TimestampKind::None,
        };
        let opts = RunOptions { timestamps, ..Default::default() };
        let mut guard = self.native.session.lock().unwrap();
        let session = guard.as_mut().context("transcribe.cpp model was released (process exiting)")?;
        let t = session.run(pcm, &opts).map_err(|e| anyhow::anyhow!("transcribe.cpp run: {e}"))?;
        let secs = |ms: i64| ms as f32 / 1e3;
        let words = if !t.words.is_empty() {
            t.words
                .iter()
                .map(|w| Word { text: w.text.trim().to_string(), start: secs(w.t0_ms), end: secs(w.t1_ms) })
                .filter(|w| !w.text.is_empty())
                .collect()
        } else if t.timestamp_kind == TimestampKind::Segment {
            t.segments.iter().flat_map(|s| spread_words(&s.text, secs(s.t0_ms), secs(s.t1_ms))).collect()
        } else {
            Vec::new()
        };
        Ok(Hypothesis { text: t.text.trim().to_string(), words })
    }

    fn word_timings(&self) -> bool {
        matches!(self.max_timestamps, TimestampKind::Word | TimestampKind::Token)
    }
}
