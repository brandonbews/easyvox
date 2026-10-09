//! easyvox-core: the UI-agnostic speech core for EasyVox.
//!
//! It captures audio, runs a swappable speech model with live partials, does one final pass
//! on release, pastes the result and keeps a history. No UI code lives here, so the app shell
//! can change without touching it.
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

pub mod audio;
pub mod history;
pub mod metrics;
pub mod models;
pub mod partials;
pub mod paste;
pub mod paths;
mod session;

#[cfg(feature = "sherpa")]
mod sherpa;
#[cfg(feature = "transcribe-cpp")]
mod tcpp;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub use audio::{MicPermission, mic_permission_status, request_mic_permission};
pub use history::{History, HistoryEntry};
pub use models::{ModelInfo, Runtime, catalog};
pub use paste::{accessibility_trusted, copy_to_clipboard, paste_text};
pub use session::{Session, SessionStats};
#[cfg(feature = "transcribe-cpp")]
pub use tcpp::release_native_models;

/// Events a session sends to the UI.
#[derive(Clone, Debug, PartialEq)]
pub enum TranscriptEvent {
    /// Live preview. `confirmed` is stable (only ever grows); `provisional` may still change.
    Partial { confirmed: String, provisional: String },
    /// Final text, sent after `Session::stop()` (push-to-talk key released), or when a
    /// `WavReplay` file runs out. It comes from a separate pass and can differ from the
    /// partials, so the UI replaces the preview with it.
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
    /// Ignored by natively streaming models, which emit a partial per decoded chunk.
    pub partial_interval_ms: u32,
    /// Max audio window (seconds) re-decoded per partial pass. Older audio that has been
    /// confirmed is frozen and dropped from the window.
    pub partial_window_secs: f32,
    /// Trim leading/trailing silence with Silero VAD before the final pass
    /// (needs `models_dir/vad/silero_vad.onnx` and the `sherpa` feature).
    pub vad_trim: bool,
    /// Streaming models only: on stop, re-decode the whole recording offline instead of
    /// flushing the stream (slower, usually more accurate). Needs a model that supports
    /// both modes (Parakeet Unified).
    pub final_redecode: bool,
}

/// The default model (see `docs/spikes/core.md`): Parakeet Unified EN 0.6B on transcribe.cpp,
/// native streaming partials, one offline pass over the whole recording on release.
pub const RECOMMENDED_MODEL: &str = "parakeet-unified-en-0.6b-q5_k_m-gguf";

impl EngineConfig {
    /// [`RECOMMENDED_MODEL`] with `final_redecode = true` (needs the `transcribe-cpp` feature).
    pub fn recommended(models_dir: impl Into<PathBuf>) -> Self {
        let mut c = Self::new(RECOMMENDED_MODEL, models_dir);
        c.final_redecode = true;
        c
    }

    pub fn new(model_id: impl Into<String>, models_dir: impl Into<PathBuf>) -> Self {
        Self {
            model_id: model_id.into(),
            models_dir: models_dir.into(),
            threads: 4,
            partial_interval_ms: 400,
            partial_window_secs: 20.0,
            vad_trim: false,
            final_redecode: false,
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
    /// Word timings. May be empty if the runtime gives none.
    pub words: Vec<Word>,
}

/// Why a decode is happening; adapters may pick cheaper settings for partials.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeMode {
    Partial,
    Final,
}

/// The adapter interface every speech runtime implements. Audio is always 16 kHz mono f32.
pub trait SpeechModel: Send + Sync + 'static {
    fn info(&self) -> &ModelInfo;
    /// Offline decode of a whole buffer.
    fn transcribe(&self, pcm: &[f32], mode: DecodeMode) -> anyhow::Result<Hypothesis>;
    /// Native streaming, if the model supports it. `None` means the session re-decodes a
    /// sliding window for partials instead.
    fn new_stream(&self) -> Option<Box<dyn StreamDecoder>> {
        None
    }
}

/// Incremental decoder for natively streaming models.
pub trait StreamDecoder: Send {
    /// Feed new 16 kHz mono samples; decodes whatever chunks are ready.
    fn accept(&mut self, pcm: &[f32]);
    /// Current `(confirmed, provisional)` text.
    fn partial(&self) -> (String, String);
    /// Flush the tail and return the final text.
    fn finish(&mut self) -> String;
}

/// Splits append-only streaming output into (everything but the last word, last word).
pub fn split_last_word(text: &str) -> (String, String) {
    match text.rfind(' ') {
        Some(i) => (text[..i].to_string(), text[i + 1..].to_string()),
        None => (String::new(), text.to_string()),
    }
}

pub(crate) struct EngineInner {
    pub(crate) model: Arc<dyn SpeechModel>,
    pub(crate) config: EngineConfig,
    pub(crate) load_ms: u32,
    /// Serializes decodes so two sessions never oversubscribe the CPU/GPU.
    pub(crate) decode_lock: Mutex<()>,
    #[cfg(feature = "sherpa")]
    pub(crate) vad: Option<sherpa::VadTrimmer>,
}

/// A loaded speech model. Send + Sync and cheap to clone (Arc inside).
#[derive(Clone)]
pub struct Engine {
    pub(crate) inner: Arc<EngineInner>,
}

impl Engine {
    /// Blocking: loads the model. Call it off the UI thread.
    pub fn load(config: EngineConfig) -> anyhow::Result<Engine> {
        let t0 = Instant::now();
        let info = models::find(&config.model_id)?;
        let model: Arc<dyn SpeechModel> = match info.runtime {
            #[cfg(feature = "sherpa")]
            Runtime::SherpaOffline => Arc::new(sherpa::ParakeetModel::load(info, &config)?),
            #[cfg(feature = "sherpa")]
            Runtime::SherpaOnline => Arc::new(sherpa::StreamingModel::load(info, &config)?),
            #[cfg(feature = "transcribe-cpp")]
            Runtime::TranscribeCpp => Arc::new(tcpp::TcppModel::load(info, &config)?),
            #[allow(unreachable_patterns)]
            other => anyhow::bail!(
                "model {} needs runtime {other:?}, which this build of easyvox-core was compiled without",
                info.id
            ),
        };
        Self::from_model_timed(model, config, t0)
    }

    /// Wrap a custom [`SpeechModel`] adapter.
    pub fn from_model(model: Arc<dyn SpeechModel>, config: EngineConfig) -> anyhow::Result<Engine> {
        Self::from_model_timed(model, config, Instant::now())
    }

    fn from_model_timed(model: Arc<dyn SpeechModel>, config: EngineConfig, t0: Instant) -> anyhow::Result<Engine> {
        #[cfg(feature = "sherpa")]
        let vad = if config.vad_trim { Some(sherpa::VadTrimmer::load(&config)?) } else { None };
        #[cfg(not(feature = "sherpa"))]
        if config.vad_trim {
            anyhow::bail!("vad_trim needs the `sherpa` feature (Silero VAD comes from sherpa-onnx)");
        }
        let load_ms = t0.elapsed().as_millis() as u32;
        Ok(Engine {
            inner: Arc::new(EngineInner {
                model,
                config,
                load_ms,
                decode_lock: Mutex::new(()),
                #[cfg(feature = "sherpa")]
                vad,
            }),
        })
    }

    /// Start a push-to-talk session. Events arrive on the returned receiver.
    pub fn start(&self, source: AudioSource) -> anyhow::Result<(Session, std::sync::mpsc::Receiver<TranscriptEvent>)> {
        session::start(self.inner.clone(), source)
    }

    /// One-shot decode of 16 kHz mono samples (benchmarks, re-transcribing history).
    pub fn transcribe_pcm(&self, pcm: &[f32]) -> anyhow::Result<Hypothesis> {
        let pcm = self.inner.trim(pcm);
        let _g = self.inner.decode_lock.lock().unwrap();
        self.inner.model.transcribe(&pcm, DecodeMode::Final)
    }

    pub fn model_info(&self) -> &ModelInfo {
        self.inner.model.info()
    }

    /// A fresh incremental decoder, for natively streaming models (`None` otherwise).
    pub fn stream_decoder(&self) -> Option<Box<dyn StreamDecoder>> {
        self.inner.model.new_stream()
    }

    /// Wall time `load()` took, in ms.
    pub fn load_ms(&self) -> u32 {
        self.inner.load_ms
    }

    pub fn config(&self) -> &EngineConfig {
        &self.inner.config
    }
}

impl EngineInner {
    pub(crate) fn trim<'a>(&self, pcm: &'a [f32]) -> std::borrow::Cow<'a, [f32]> {
        #[cfg(feature = "sherpa")]
        if let Some(vad) = &self.vad {
            return std::borrow::Cow::Owned(vad.trim(pcm));
        }
        std::borrow::Cow::Borrowed(pcm)
    }
}

/// Sample rate every engine consumes.
pub const SAMPLE_RATE: u32 = 16_000;
