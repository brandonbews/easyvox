//! transcribe.cpp adapter (ggml, Metal by default): Parakeet TDT/Unified and Whisper GGUF models.
//! Offline decode via `Session::run`, native streaming via `Session::stream` for models
//! that support it (Parakeet Unified, Nemotron streaming).
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
use crate::{DecodeMode, EngineConfig, Hypothesis, SpeechModel, StreamDecoder, Word};
use anyhow::{Context, bail};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use transcribe_cpp::{
    Model, ParakeetBufferedStreamOptions, RunOptions, Session, SessionOptions, StreamExtension, StreamOptions,
    TimestampKind,
};

/// Parakeet Unified buffered-streaming window. The model default (L 5600 / C 1040 / R 1040 ms)
/// shows the first words ~2.2 s after speech starts (streamed WER 0.55% on our clips);
/// chunk 560 / right 160 shows them after ~1.4 s (WER 1.4%), 160/160 after ~0.85 s (1.6%,
/// 2x the compute). Override with
/// EASYVOX_PK_CHUNK_MS / EASYVOX_PK_RIGHT_MS (multiples of 80 from the model's menu:
/// chunk 80/160/560/1040, right 0/80/160/240/320/560/1040; -1 = model default).
fn buffered_stream_options() -> StreamOptions {
    let env = |k: &str, d: i32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    StreamOptions {
        family: Some(StreamExtension::ParakeetBuffered(ParakeetBufferedStreamOptions {
            left_ms: None,
            chunk_ms: Some(env("EASYVOX_PK_CHUNK_MS", 560)).filter(|v| *v >= 0),
            right_ms: Some(env("EASYVOX_PK_RIGHT_MS", 160)).filter(|v| *v >= 0),
        })),
        ..Default::default()
    }
}

pub struct TcppModel {
    info: &'static ModelInfo,
    native: Arc<Native>,
    threads: i32,
    streaming: bool,
    /// Word timings for partials when the family has them (Parakeet yes, Whisper no).
    word_timestamps: bool,
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
        let threads = cfg.threads as i32;
        let session = model
            .session_with(&SessionOptions { n_threads: threads, ..Default::default() })
            .map_err(|e| anyhow::anyhow!("transcribe.cpp session: {e}"))?;
        let caps = model.capabilities();
        let streaming = info.streaming && caps.supports_streaming;
        let word_timestamps = matches!(caps.max_timestamp_kind, TimestampKind::Word | TimestampKind::Token);
        let native = Arc::new(Native { model: Mutex::new(Some(model)), session: Mutex::new(Some(session)) });
        {
            let mut live = LIVE.lock().unwrap();
            live.retain(|w| w.strong_count() > 0);
            live.push(Arc::downgrade(&native));
        }
        ATEXIT.call_once(|| unsafe {
            libc::atexit(release_at_exit);
        });
        Ok(Self { info, native, threads, streaming, word_timestamps })
    }
}

impl SpeechModel for TcppModel {
    fn info(&self) -> &ModelInfo {
        self.info
    }

    fn transcribe(&self, pcm: &[f32], mode: DecodeMode) -> anyhow::Result<Hypothesis> {
        if pcm.len() < 1600 {
            return Ok(Hypothesis::default());
        }
        let opts = RunOptions {
            timestamps: if mode == DecodeMode::Partial && self.word_timestamps {
                TimestampKind::Word
            } else {
                TimestampKind::None
            },
            ..Default::default()
        };
        let mut guard = self.native.session.lock().unwrap();
        let session = guard.as_mut().context("transcribe.cpp model was released (process exiting)")?;
        let t = session.run(pcm, &opts).map_err(|e| anyhow::anyhow!("transcribe.cpp run: {e}"))?;
        let words = t
            .words
            .iter()
            .map(|w| Word { text: w.text.trim().to_string(), start: w.t0_ms as f32 / 1e3, end: w.t1_ms as f32 / 1e3 })
            .filter(|w| !w.text.is_empty())
            .collect();
        Ok(Hypothesis { text: t.text.trim().to_string(), words })
    }

    fn new_stream(&self) -> Option<Box<dyn StreamDecoder>> {
        if !self.streaming {
            return None;
        }
        let model = self.native.model.lock().unwrap().clone()?;
        TcppStream::spawn(model, self.threads).ok().map(|s| Box::new(s) as Box<dyn StreamDecoder>)
    }
}

enum Msg {
    Feed(Vec<f32>),
    Finish,
}

/// `transcribe_cpp::Stream` borrows its `Session`, so the pair lives on its own thread.
/// The library refuses `run()` while a stream is alive on the model, so `finish()` and
/// `Drop` both wait until the thread has dropped the stream.
struct TcppStream {
    tx: Option<Sender<Msg>>,
    ack: Receiver<String>,
    text: Arc<Mutex<(String, String)>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl TcppStream {
    fn spawn(model: Model, threads: i32) -> anyhow::Result<Self> {
        let (tx, rx) = channel::<Msg>();
        let (ack_tx, ack) = channel::<String>();
        let text = Arc::new(Mutex::new((String::new(), String::new())));
        let shared = text.clone();
        let (ready_tx, ready_rx) = channel::<Result<(), String>>();
        let thread = std::thread::Builder::new().name("easyvox-tcpp-stream".into()).spawn(move || {
            let mut session = match model.session_with(&SessionOptions { n_threads: threads, ..Default::default() }) {
                Ok(s) => s,
                Err(e) => {
                    let _ = ready_tx.send(Err(e.to_string()));
                    return;
                }
            };
            let opts = if model.arch().contains("parakeet") && model.variant().contains("unified") {
                buffered_stream_options()
            } else {
                StreamOptions::default()
            };
            log::debug!("tcpp stream: arch={} variant={} opts={opts:?}", model.arch(), model.variant());
            let final_text = {
                let mut stream = match session.stream(&RunOptions::default(), &opts) {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e.to_string()));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(()));
                let mut final_text = None;
                while let Ok(msg) = rx.recv() {
                    match msg {
                        Msg::Feed(pcm) => {
                            let r = stream.feed(&pcm);
                            log::trace!("tcpp feed {} -> {r:?}", pcm.len());
                            let t = stream.text();
                            *shared.lock().unwrap() = (t.committed.trim().to_string(), t.tentative.trim().to_string());
                            let _ = ack_tx.send(String::new());
                        }
                        Msg::Finish => {
                            let _ = stream.finalize();
                            final_text = Some(stream.text().full.trim().to_string());
                            break;
                        }
                    }
                }
                final_text
            }; // stream dropped here: the model accepts run() again
            if let Some(t) = final_text {
                let _ = ack_tx.send(t);
            }
        })?;
        ready_rx.recv().context("stream thread died")?.map_err(|e| anyhow::anyhow!("transcribe.cpp stream: {e}"))?;
        Ok(Self { tx: Some(tx), ack, text, thread: Some(thread) })
    }
}

impl StreamDecoder for TcppStream {
    fn accept(&mut self, pcm: &[f32]) {
        if self.tx.as_ref().is_some_and(|tx| tx.send(Msg::Feed(pcm.to_vec())).is_ok()) {
            let _ = self.ack.recv();
        }
    }
    fn partial(&self) -> (String, String) {
        self.text.lock().unwrap().clone()
    }
    fn finish(&mut self) -> String {
        if !self.tx.as_ref().is_some_and(|tx| tx.send(Msg::Finish).is_ok()) {
            return String::new();
        }
        self.ack.recv().unwrap_or_default()
    }
}

impl Drop for TcppStream {
    fn drop(&mut self) {
        self.tx.take(); // closes the channel; the thread drops the stream and exits
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
