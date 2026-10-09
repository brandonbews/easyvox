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

/// The default model: Parakeet Unified EN 0.6B on transcribe.cpp (see `DESIGN.md`, "Models").
pub const RECOMMENDED_MODEL: &str = "parakeet-unified-en-0.6b-q5_k_m-gguf";

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

    /// One-shot decode of 16 kHz mono samples (benchmarks, re-transcribing history). Word
    /// times are relative to the start of `pcm`.
    pub fn transcribe_pcm(&self, pcm: &[f32]) -> anyhow::Result<Hypothesis> {
        let speech = self.inner.speech(pcm);
        let _g = self.inner.decode_lock.lock().unwrap();
        let mut h = self.inner.model.transcribe(&pcm[speech.clone()], DecodeMode::Final)?;
        let offset = speech.start as f32 / SAMPLE_RATE as f32;
        h.words.iter_mut().for_each(|w| (w.start, w.end) = (w.start + offset, w.end + offset));
        Ok(h)
    }

    pub fn model_info(&self) -> &ModelInfo {
        self.inner.model.info()
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
