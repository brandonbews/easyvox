//! easyvox-core: the UI-agnostic speech core for EasyVox.
//!
//! It captures audio, runs a swappable speech model for a live preview and the final text,
//! pastes the result and keeps a history. No UI code lives here, so the app shell can change
//! without touching it.
//!
//! Every model takes the same path, and all it has to do is turn audio into words with timings
//! ([`SpeechModel`]). While the key is held, the session re-decodes the recent audio every few
//! hundred ms for the preview, and polishes finished chunks of a long dictation in the
//! background. On release it decodes only what's left, so the final text arrives in well under
//! a second at any length.
//!
//! ```no_run
//! use easyvox_core::*;
//! // Blocking (~0.2 s once the Metal shaders are cached): call it off the UI thread.
//! let engine = Engine::load(EngineConfig::recommended(paths::models_dir()))?;
//! let (session, events) = engine.start(AudioSource::WavReplay("clip.wav".into()))?;
//! for ev in events {
//!     match ev {
//!         TranscriptEvent::Partial { confirmed, provisional } => println!("{confirmed} [{provisional}]"),
//!         TranscriptEvent::Final { text, .. } => { println!("{text}"); break; }
//!         TranscriptEvent::Error(e) => eprintln!("{e}"),
//!     }
//! }
//! # drop(session);
//! # anyhow::Ok(())
//! ```
//!
//! Push-to-talk: `start(AudioSource::Microphone)` on key-down, `Session::stop()` on key-up, then
//! wait for `Final`. Dropping a `Session` joins its worker thread, so drop it off the UI thread.
//!
//! An idle app can free the model with [`Engine::unload`] (about 600 MB for the default model).
//! The next `start` reloads it while it already records, so nothing said meanwhile is lost; the
//! preview simply starts once the model is ready.
//!
//! Binaries set [`process_env`] first thing in `main`, before any thread starts.

pub mod audio;
mod chunks;
pub mod history;
pub mod metrics;
pub mod models;
pub mod partials;
pub mod paste;
pub mod paths;
mod pause;
mod session;

#[cfg(feature = "sherpa")]
mod sherpa;
#[cfg(feature = "transcribe-cpp")]
mod tcpp;

use std::ops::Range;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub use audio::{MicPermission, mic_permission_status, request_mic_permission};
pub use history::{History, HistoryEntry};
pub use models::{ModelInfo, Runtime, catalog};
pub use paste::{accessibility_trusted, copy_to_clipboard, paste_text};
pub use session::{Session, SessionStats};
#[cfg(feature = "transcribe-cpp")]
pub use tcpp::release_native_models;

/// Environment variables a binary using the engine sets first thing in `main`, before it starts
/// any thread. `std::env::set_var` is only safe while no other thread can read the environment,
/// so this library never sets them itself:
///
/// ```no_run
/// for (key, value) in easyvox_core::process_env() {
///     // SAFETY: the first thing `main` does, before any thread exists.
///     unsafe { std::env::set_var(key, value) };
/// }
/// ```
///
/// Today that's `GGML_METAL_NO_RESIDENCY=1`: without it, ggml-metal wakes the process about
/// 150–200 times a second for as long as it runs, even with no model loaded (see `tcpp.rs`).
/// Set `EASYVOX_METAL_RESIDENCY=1` to keep ggml's default, for comparisons.
pub fn process_env() -> Vec<(&'static str, &'static str)> {
    let mut env = Vec::new();
    if cfg!(feature = "transcribe-cpp") && std::env::var_os("EASYVOX_METAL_RESIDENCY").is_none() {
        env.push(("GGML_METAL_NO_RESIDENCY", "1"));
    }
    env
}

/// Events a session sends to the UI.
#[derive(Clone, Debug, PartialEq)]
pub enum TranscriptEvent {
    /// Live preview: `confirmed` then `provisional` make up the transcript so far.
    ///
    /// `provisional` may change with every event. `confirmed` is stable and only grows, with one
    /// exception: in a long dictation the session polishes each finished chunk (about 30 s of
    /// audio, cut at a pause) while you keep talking, and that chunk's preview text is then
    /// replaced by its polished text, which is what `Final` will contain. So a change only ever
    /// touches text from a finished chunk, seconds behind the newest words.
    Partial { confirmed: String, provisional: String },
    /// Final text, sent after `Session::stop()` (push-to-talk key released), or when a
    /// `WavReplay` file runs out. Every word comes from a `Final`-mode decode (the chunks
    /// polished while recording, then the rest), so it can differ from the preview, and the UI
    /// replaces the preview with it.
    Final { text: String, audio_secs: f32, finalize_ms: u32 },
    /// A problem during the session (the mic delivered silence, a decode failed, ...).
    /// A `Final` still follows.
    Error(String),
}

/// Where audio comes from.
#[derive(Clone, Debug)]
pub enum AudioSource {
    /// Default input device via cpal, downmixed and resampled to 16 kHz mono.
    /// Refused until the Microphone permission is granted (see [`request_mic_permission`]).
    Microphone,
    /// Feeds a WAV file at real-time speed, as if someone were speaking it. When the file
    /// ends the session stops itself (as if the key was released) and sends `Final`.
    WavReplay(PathBuf),
}

/// Engine configuration. Build with [`EngineConfig::recommended`] or [`EngineConfig::new`]
/// and tweak fields as needed.
#[derive(Clone, Debug)]
pub struct EngineConfig {
    /// One of the ids from [`catalog()`].
    pub model_id: String,
    /// Root models directory, normally [`paths::models_dir()`].
    pub models_dir: PathBuf,
    /// CPU threads for inference. Default 4 (an M-series Mac has at least 4 performance cores).
    pub threads: usize,
    /// How often to re-decode for live partials, in ms. 0 disables partials.
    pub partial_interval_ms: u32,
    /// Max audio window (seconds) re-decoded per partial pass. Older audio that has been
    /// confirmed is frozen and dropped from the window.
    pub partial_window_secs: f32,
    /// Trim leading/trailing silence with Silero VAD before the final pass
    /// (needs `models_dir/vad/silero_vad.onnx` and the `sherpa` feature).
    pub vad_trim: bool,
    /// Eager final pass: once this many seconds of audio wait unpolished, the session cuts off
    /// the oldest stretch at a pause and decodes it in `Final` mode while the user keeps
    /// talking, so release only waits for the rest (see `chunks.rs`). Default 30.
    /// 0 decodes the whole recording in one pass on release, for comparisons: that pass grows
    /// quadratically (21.6 s after 9 minutes).
    pub chunk_secs: f32,
}

/// The default model: Parakeet TDT 0.6B v2 on transcribe.cpp (see `DESIGN.md`, "Models").
pub const RECOMMENDED_MODEL: &str = "parakeet-tdt-0.6b-v2-q5_k_m-gguf";

impl EngineConfig {
    /// [`RECOMMENDED_MODEL`] with the default settings (needs the `transcribe-cpp` feature).
    pub fn recommended(models_dir: impl Into<PathBuf>) -> Self {
        Self::new(RECOMMENDED_MODEL, models_dir)
    }

    pub fn new(model_id: impl Into<String>, models_dir: impl Into<PathBuf>) -> Self {
        Self {
            model_id: model_id.into(),
            models_dir: models_dir.into(),
            threads: 4,
            partial_interval_ms: 400,
            partial_window_secs: 20.0,
            vad_trim: false,
            chunk_secs: 30.0,
        }
    }
}

/// One recognized word with timing in seconds, relative to the start of the audio passed in.
#[derive(Clone, Debug, PartialEq)]
pub struct Word {
    pub text: String,
    pub start: f32,
    pub end: f32,
}

/// A decode result.
#[derive(Clone, Debug, Default)]
pub struct Hypothesis {
    pub text: String,
    /// The words of `text` with their timings. Empty if the runtime gives none: the session
    /// then spreads the words evenly over the audio, which is rough but enough.
    pub words: Vec<Word>,
}

impl Hypothesis {
    /// The words with absolute times: shifted by `offset` seconds, or spread over
    /// `offset..offset + secs` (the audio decoded) when the runtime gave no timings.
    pub(crate) fn timed_words(self, offset: f32, secs: f32) -> Vec<Word> {
        if self.words.is_empty() {
            return spread_words(&self.text, offset, offset + secs);
        }
        let shift = |w: Word| Word { start: w.start + offset, end: w.end + offset, ..w };
        self.words.into_iter().map(shift).collect()
    }
}

/// `text`'s words spread evenly over `start..end` (seconds), for runtimes without word timings.
pub(crate) fn spread_words(text: &str, start: f32, end: f32) -> Vec<Word> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let each = (end - start).max(0.0) / words.len().max(1) as f32;
    let at = |i: usize| start + i as f32 * each;
    words.iter().enumerate().map(|(i, w)| Word { text: w.to_string(), start: at(i), end: at(i + 1) }).collect()
}

/// Why a decode is happening; adapters may pick cheaper settings for partials.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeMode {
    Partial,
    Final,
}

/// The adapter interface every speech runtime implements: audio in, words with timings out.
/// Audio is always 16 kHz mono f32. The session does the rest (preview, chunking, joining) the
/// same way for every model; a runtime's quirks stay inside its adapter.
///
/// Natively streaming models (Parakeet Unified, Nemotron) are decoded offline like the rest.
/// Their native streaming could come back later as an optimisation, if the re-decoded preview
/// ever costs too much CPU or battery.
pub trait SpeechModel: Send + Sync + 'static {
    fn info(&self) -> &ModelInfo;
    /// Offline decode of a whole buffer, any length.
    fn transcribe(&self, pcm: &[f32], mode: DecodeMode) -> anyhow::Result<Hypothesis>;
    /// Whether `transcribe` times each word, rather than giving coarser timings (segments) or
    /// none. Chunks of a long dictation are decoded with context only then.
    fn word_timings(&self) -> bool {
        false
    }
}

/// Where an engine's model comes from.
enum Source {
    /// A catalog entry: loaded from disk, so it can be unloaded and loaded again.
    Catalog(&'static ModelInfo),
    /// A custom adapter ([`Engine::from_model`]), kept for the engine's lifetime.
    Custom(Arc<dyn SpeechModel>),
}

pub(crate) struct EngineInner {
    source: Source,
    pub(crate) config: EngineConfig,
    /// The loaded model, or `None` while unloaded. A session holds its own `Arc` from start to
    /// `Final`, so unloading never pulls the model out from under it.
    model: Mutex<Option<Arc<dyn SpeechModel>>>,
    /// Held while loading or unloading, so concurrent callers never load the model twice.
    load_lock: Mutex<()>,
    load_ms: AtomicU32,
    /// Serializes decodes so two sessions never oversubscribe the CPU/GPU.
    pub(crate) decode_lock: Mutex<()>,
    #[cfg(feature = "sherpa")]
    pub(crate) vad: Option<sherpa::VadTrimmer>,
}

/// A speech model and its settings. Send + Sync and cheap to clone (Arc inside).
///
/// The model can be freed while the app is idle ([`unload`](Self::unload)); the next session
/// or decode loads it again.
#[derive(Clone)]
pub struct Engine {
    pub(crate) inner: Arc<EngineInner>,
}

impl Engine {
    /// Blocking: loads the model. Call it off the UI thread.
    pub fn load(config: EngineConfig) -> anyhow::Result<Engine> {
        let info = models::find(&config.model_id)?;
        if !info.runtime.is_compiled() {
            anyhow::bail!(
                "model {} needs runtime {:?}, which this build of easyvox-core was compiled without",
                info.id,
                info.runtime
            );
        }
        let engine = Self::new(Source::Catalog(info), config)?;
        engine.preload()?;
        Ok(engine)
    }

    /// Wrap a custom [`SpeechModel`] adapter. It stays loaded: [`unload`](Self::unload) only
    /// frees catalog models.
    pub fn from_model(model: Arc<dyn SpeechModel>, config: EngineConfig) -> anyhow::Result<Engine> {
        let engine = Self::new(Source::Custom(model), config)?;
        engine.preload()?;
        Ok(engine)
    }

    fn new(source: Source, config: EngineConfig) -> anyhow::Result<Engine> {
        #[cfg(feature = "sherpa")]
        let vad = if config.vad_trim { Some(sherpa::VadTrimmer::load(&config)?) } else { None };
        #[cfg(not(feature = "sherpa"))]
        if config.vad_trim {
            anyhow::bail!("vad_trim needs the `sherpa` feature (Silero VAD comes from sherpa-onnx)");
        }
        Ok(Engine {
            inner: Arc::new(EngineInner {
                source,
                config,
                model: Mutex::new(None),
                load_lock: Mutex::new(()),
                load_ms: AtomicU32::new(0),
                decode_lock: Mutex::new(()),
                #[cfg(feature = "sherpa")]
                vad,
            }),
        })
    }

    /// Blocking: loads the model now if it isn't loaded.
    fn preload(&self) -> anyhow::Result<()> {
        self.inner.model().map(drop)
    }

    /// Frees the model; the next session or decode loads it again. Blocking while the memory
    /// is freed, so call it off the UI thread. A running session keeps the model until it
    /// ends. Returns whether a model was loaded (custom adapters are never unloaded).
    pub fn unload(&self) -> bool {
        if matches!(self.inner.source, Source::Custom(_)) {
            return false;
        }
        let _loading = self.inner.load_lock.lock().unwrap();
        let model = self.inner.model.lock().unwrap().take();
        model.is_some() // freed here, outside the lock
    }

    /// Whether the model is in memory now.
    pub fn is_loaded(&self) -> bool {
        self.inner.model.lock().unwrap().is_some()
    }

    /// Start a push-to-talk session. Events arrive on the returned receiver.
    ///
    /// Capture starts at once. If the model was unloaded, the session loads it while it
    /// records (about 0.15 s once the Metal shaders are cached), so no audio is lost: the
    /// preview starts once the model is ready, and `Final` covers the whole recording.
    pub fn start(&self, source: AudioSource) -> anyhow::Result<(Session, std::sync::mpsc::Receiver<TranscriptEvent>)> {
        session::start(self.inner.clone(), source)
    }

    /// One-shot decode of 16 kHz mono samples (benchmarks, re-transcribing history). Word
    /// times are relative to the start of `pcm`. Loads the model first if it was unloaded.
    pub fn transcribe_pcm(&self, pcm: &[f32]) -> anyhow::Result<Hypothesis> {
        let model = self.inner.model()?;
        let speech = self.inner.speech(pcm);
        let _g = self.inner.decode_lock.lock().unwrap();
        let mut h = model.transcribe(&pcm[speech.clone()], DecodeMode::Final)?;
        let offset = speech.start as f32 / SAMPLE_RATE as f32;
        h.words.iter_mut().for_each(|w| (w.start, w.end) = (w.start + offset, w.end + offset));
        Ok(h)
    }

    pub fn model_info(&self) -> &ModelInfo {
        match &self.inner.source {
            Source::Catalog(info) => info,
            Source::Custom(model) => model.info(),
        }
    }

    /// Wall time the latest load of the model took, in ms.
    pub fn load_ms(&self) -> u32 {
        self.inner.load_ms.load(Ordering::Relaxed)
    }

    pub fn config(&self) -> &EngineConfig {
        &self.inner.config
    }
}

impl EngineInner {
    /// The model, loaded first if it isn't in memory. Blocking while it loads.
    pub(crate) fn model(&self) -> anyhow::Result<Arc<dyn SpeechModel>> {
        if let Some(model) = self.model.lock().unwrap().clone() {
            return Ok(model);
        }
        let _loading = self.load_lock.lock().unwrap();
        if let Some(model) = self.model.lock().unwrap().clone() {
            return Ok(model); // another caller loaded it while we waited
        }
        let t0 = Instant::now();
        let model: Arc<dyn SpeechModel> = match &self.source {
            Source::Custom(model) => model.clone(),
            Source::Catalog(info) => match info.runtime {
                #[cfg(feature = "sherpa")]
                Runtime::SherpaOffline => Arc::new(sherpa::ParakeetModel::load(info, &self.config)?),
                #[cfg(feature = "sherpa")]
                Runtime::SherpaOnline => Arc::new(sherpa::StreamingModel::load(info, &self.config)?),
                #[cfg(feature = "transcribe-cpp")]
                Runtime::TranscribeCpp => Arc::new(tcpp::TcppModel::load(info, &self.config)?),
                #[allow(unreachable_patterns)]
                other => anyhow::bail!("runtime {other:?} isn't compiled into this build of easyvox-core"),
            },
        };
        self.load_ms.store(t0.elapsed().as_millis() as u32, Ordering::Relaxed);
        *self.model.lock().unwrap() = Some(model.clone());
        Ok(model)
    }

    /// The part of `pcm` a `Final` decode should see: all of it, or with `vad_trim` the span
    /// from the first to the last speech (empty if there's none).
    pub(crate) fn speech(&self, pcm: &[f32]) -> Range<usize> {
        #[cfg(feature = "sherpa")]
        if let Some(vad) = &self.vad {
            return vad.speech(pcm);
        }
        0..pcm.len()
    }
}

/// Sample rate every engine consumes.
pub const SAMPLE_RATE: u32 = 16_000;
