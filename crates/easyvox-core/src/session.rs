//! Push-to-talk session: a capture thread and a decode worker, the same for every model.
//!
//! Capture starts at once. The worker first makes sure the model is loaded (reloading it if the
//! engine was unloaded while idle; the audio meanwhile just buffers), then ticks every
//! `preview_interval_ms` ([`Tuning`](crate::Tuning)) while the key is held. Each tick it
//! polishes the oldest chunk in `Final` mode if enough audio waits (`chunks.rs`), then
//! re-decodes the recent audio in `Partial` mode for the preview, confirming words once two
//! passes agree (`partials.rs`). A tick skips the re-decode when nothing has been said since
//! the last ones (`pause::SpeechGate`). On release it polishes the rest and sends `Final`.

use crate::audio::{MicInfo, SharedAudio};
use crate::chunks::{self, Chunker, Polished};
use crate::partials::LocalAgreement;
use crate::pause::SpeechGate;
use crate::tuning::ms_samples;
use crate::{AudioSource, DecodeMode, EngineInner, SAMPLE_RATE, SpeechModel, TranscriptEvent, Word};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const SR: usize = SAMPLE_RATE as usize;

/// With the preview off (`preview_interval_ms` = 0, for comparisons), how often the loop still
/// ticks to polish chunks.
const POLISH_TICK_MS: u32 = 400;
/// The preview window never gets shorter than this, whatever `preview_window_ms` says.
const MIN_WINDOW_MS: u32 = 2_000;

/// Counters a session collects (for benchmarks and diagnostics).
#[derive(Clone, Debug, Default)]
pub struct SessionStats {
    /// How long the session waited for the model at the start: about 0 when it was loaded,
    /// the reload time when the engine had been unloaded.
    pub model_wait_ms: f64,
    /// Re-decode passes run for partials.
    pub partial_passes: u32,
    /// Ticks that skipped the re-decode because nothing had been said since the last passes.
    pub partial_skipped: u32,
    pub partial_decode_ms_total: f64,
    pub partial_decode_ms_max: f64,
    /// `Partial` events sent.
    pub partial_events: u32,
    /// Chunks polished while recording (the eager final pass), and their decode times.
    pub chunks: u32,
    pub chunk_decode_ms_total: f64,
    pub chunk_decode_ms_max: f64,
    /// Time from stop (or end of WAV) until the worker started the final decode (waiting for
    /// an in-flight partial pass or chunk, and the capture thread).
    pub final_wait_ms: f64,
    /// Final decode of what was left unpolished (incl. VAD trim).
    pub final_decode_ms: f64,
    /// Confirmed partial text at the moment the final pass started.
    pub confirmed_at_stop: String,
    pub audio_secs: f32,
    pub input_peak: f32,
    pub mic: Option<MicInfo>,
    pub finished: bool,
}

struct Control {
    /// Shared with the capture thread, which polls it.
    stop: Arc<AtomicBool>,
    stop_at: Mutex<Option<Instant>>,
    stats: Mutex<SessionStats>,
}

/// A running push-to-talk session. Dropping it stops capture and blocks until the worker has
/// sent `Final` (drop it off the UI thread, or keep it until `Final` arrives).
pub struct Session {
    ctl: Arc<Control>,
    audio: Arc<SharedAudio>,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl Session {
    /// Key released: stop capture. A `Final` event follows on the channel.
    pub fn stop(&self) {
        let mut at = self.ctl.stop_at.lock().unwrap();
        if at.is_none() {
            *at = Some(Instant::now());
        }
        self.ctl.stop.store(true, Ordering::SeqCst);
        self.audio.cv.notify_all();
    }

    /// Snapshot of the session counters. Complete once `Final` has been received.
    pub fn stats(&self) -> SessionStats {
        self.ctl.stats.lock().unwrap().clone()
    }

    /// Block until the capture and worker threads exit (after `Final`).
    pub fn join(&self) {
        for h in self.threads.lock().unwrap().drain(..) {
            let _ = h.join();
        }
    }
}

impl Drop for Session {
    /// Stops capture and waits for the worker to finish (it still sends `Final`, which takes
    /// at most a model reload, a chunk decode and the final decode). Waiting matters: if the process exits
    /// while a decode thread is still running, ggml-metal's static destructors can crash at
    /// exit (seen once).
    fn drop(&mut self) {
        self.stop();
        self.join();
    }
}

pub(crate) fn start(
    engine: Arc<EngineInner>,
    source: AudioSource,
) -> anyhow::Result<(Session, Receiver<TranscriptEvent>)> {
    let audio = Arc::new(SharedAudio::default());
    let stop_flag = Arc::new(AtomicBool::new(false));
    let ctl = Arc::new(Control { stop: stop_flag.clone(), stop_at: Mutex::new(None), stats: Mutex::default() });
    let (tx, rx) = mpsc::channel();

    let is_mic = matches!(source, AudioSource::Microphone);
    let capture = match source {
        AudioSource::WavReplay(path) => crate::audio::spawn_wav_replay(&path, audio.clone(), stop_flag.clone())?,
        AudioSource::Microphone => {
            #[cfg(feature = "mic")]
            {
                let (h, info) = crate::audio::spawn_mic(audio.clone(), stop_flag.clone())?;
                ctl.stats.lock().unwrap().mic = Some(info);
                h
            }
            #[cfg(not(feature = "mic"))]
            anyhow::bail!("easyvox-core was built without the `mic` feature")
        }
    };

    let worker = {
        let (ctl, audio) = (ctl.clone(), audio.clone());
        std::thread::Builder::new().name("easyvox-decode".into()).spawn(move || {
            let w = Worker { engine, audio, ctl, tx, is_mic };
            w.run();
        })?
    };

    Ok((Session { ctl, audio, threads: Mutex::new(vec![capture, worker]) }, rx))
}

struct Worker {
    engine: Arc<EngineInner>,
    audio: Arc<SharedAudio>,
    ctl: Arc<Control>,
    tx: Sender<TranscriptEvent>,
    is_mic: bool,
}

impl Worker {
    fn send(&self, ev: TranscriptEvent) {
        if matches!(ev, TranscriptEvent::Partial { .. }) {
            self.ctl.stats.lock().unwrap().partial_events += 1;
        }
        let _ = self.tx.send(ev);
    }

    fn done(&self) -> bool {
        self.audio.done.load(Ordering::SeqCst)
    }

    /// Instant the user released the key, or the WAV ended.
    fn end_instant(&self) -> Instant {
        let stop = *self.ctl.stop_at.lock().unwrap();
        let done = *self.audio.done_at.lock().unwrap();
        match (stop, done) {
            (Some(a), Some(b)) => a.min(b),
            (a, b) => a.or(b).unwrap_or_else(Instant::now),
        }
    }

    /// Wait until `deadline`, new audio beyond `have` samples, or end of input.
    fn wait(&self, deadline: Instant, have: usize) {
        let mut g = self.audio.samples.lock().unwrap();
        while !self.done() && Instant::now() < deadline && g.len() <= have {
            let left = deadline.saturating_duration_since(Instant::now());
            g = self.audio.cv.wait_timeout(g, left.min(Duration::from_millis(20))).unwrap().0;
        }
    }

    fn mic_checks(&self, started: Instant, warned: &mut bool) {
        if !self.is_mic || *warned || started.elapsed() < Duration::from_millis(1500) {
            return;
        }
        let n = self.audio.len();
        if n == 0 {
            *warned = true;
            self.send(TranscriptEvent::Error("No audio arrived from the microphone in 1.5 s.".into()));
        } else if self.audio.peak() == 0.0 {
            *warned = true;
            self.send(TranscriptEvent::Error(
                "The microphone is delivering digital silence. On macOS this usually means the app has not been granted Microphone permission.".into(),
            ));
        }
    }

    fn run(self) {
        let started = Instant::now();
        // Reloads the model if the engine was unloaded. Capture already runs, so whatever is
        // said meanwhile buffers and gets decoded once the model is ready.
        let model = self.engine.model();
        self.ctl.stats.lock().unwrap().model_wait_ms = started.elapsed().as_secs_f64() * 1e3;
        match model {
            Ok(model) => self.record(&*model, started),
            Err(e) => {
                self.send(TranscriptEvent::Error(format!("the speech model couldn't load: {e:#}")));
                while !self.done() {
                    self.wait(Instant::now() + Duration::from_secs(1), usize::MAX);
                }
                let end = self.end_instant();
                self.finish(String::new(), end, Instant::now(), String::new());
            }
        }
    }

    fn record(&self, model: &dyn SpeechModel, started: Instant) {
        let mut warned = false;
        let t = &self.engine.config.tuning;
        let chunker = Chunker::new(t);
        let mut polished = Polished::new(model.word_timings(), t);
        let mut la = LocalAgreement::new(t);
        let mut gate = SpeechGate::new(t);
        let preview_on = t.preview_interval_ms > 0;
        let interval = Duration::from_millis(if preview_on { t.preview_interval_ms } else { POLISH_TICK_MS } as u64);
        let window = ms_samples(t.preview_window_ms.max(MIN_WINDOW_MS));
        let min_audio = ms_samples(t.preview_min_audio_ms);
        let min_gap = Duration::from_millis(t.preview_min_gap_ms as u64);
        let mut anchor = 0usize;
        let mut decoded_len = 0usize;
        // The first tick comes one interval after the start, or at once if loading the model
        // took longer than that.
        let mut next = (started + interval).max(Instant::now());
        let mut last = (String::new(), String::new());
        loop {
            // Sleep until the next tick (new audio alone doesn't wake us early).
            loop {
                self.wait(next, usize::MAX);
                if self.done() || Instant::now() >= next {
                    break;
                }
            }
            self.mic_checks(started, &mut warned);
            if self.done() {
                break;
            }
            next = Instant::now() + interval;
            if self.polish_chunk(model, &chunker, &mut polished) {
                // The preview keeps only what follows the polished words, and its window moves
                // up to the cut (less the chunks' context).
                la.skip_to(polished.until());
                anchor = anchor.max(polished.start().saturating_sub(polished.context()));
                if self.done() {
                    break;
                }
            }
            if !preview_on {
                continue;
            }
            let window_pcm = {
                let a = self.audio.samples.lock().unwrap();
                let total = a.len();
                // Too little audio yet, or less than 0.1 s new since the last pass.
                if total < min_audio || total < decoded_len + SR / 10 {
                    continue;
                }
                if total - anchor > window {
                    let frozen = (la.confirmed_until() * SR as f32) as usize;
                    anchor = anchor.max(frozen.saturating_sub(polished.context()).min(total));
                    if total - anchor > window {
                        anchor = total - window;
                    }
                }
                gate.should_decode(&a, anchor).then(|| {
                    decoded_len = total;
                    a[anchor..total].to_vec()
                })
            };
            let Some(window_pcm) = window_pcm else {
                // Nothing said since the last passes: a re-decode would only repeat them. A
                // chunk polished during the pause still updates the preview.
                self.ctl.stats.lock().unwrap().partial_skipped += 1;
                let cur = preview(&polished, &la);
                if cur != last && !(cur.0.is_empty() && cur.1.is_empty()) {
                    last = cur.clone();
                    self.send(TranscriptEvent::Partial { confirmed: cur.0, provisional: cur.1 });
                }
                continue;
            };
            let t = Instant::now();
            let hyp = {
                let _g = self.engine.decode_lock.lock().unwrap();
                model.transcribe(&window_pcm, DecodeMode::Partial)
            };
            let ms = t.elapsed().as_secs_f64() * 1e3;
            {
                let mut s = self.ctl.stats.lock().unwrap();
                s.partial_passes += 1;
                s.partial_decode_ms_total += ms;
                s.partial_decode_ms_max = s.partial_decode_ms_max.max(ms);
            }
            match hyp {
                Ok(h) => {
                    la.update(&h.timed_words(anchor as f32 / SR as f32, window_pcm.len() as f32 / SR as f32));
                    let cur = preview(&polished, &la);
                    if cur != last && !(cur.0.is_empty() && cur.1.is_empty()) {
                        last = cur.clone();
                        self.send(TranscriptEvent::Partial { confirmed: cur.0, provisional: cur.1 });
                    }
                }
                Err(e) => self.send(TranscriptEvent::Error(format!("partial decode failed: {e:#}"))),
            }
            if Instant::now() >= next {
                next = Instant::now() + min_gap;
            }
        }
        // Released: polish the rest.
        let end = self.end_instant();
        let t = Instant::now();
        let confirmed = preview(&polished, &la).0;
        let (pcm, span) = {
            let a = self.audio.samples.lock().unwrap();
            let span = polished.span(a.len(), a.len());
            (a[span.clone()].to_vec(), span)
        };
        polished.add(&self.decode_final(model, &pcm, span.start), span, None);
        self.finish(polished.text(), end, t, confirmed);
    }

    /// The eager final pass: if enough audio waits unpolished, decode the oldest chunk in
    /// `Final` mode (see `chunks.rs`). True if it did.
    fn polish_chunk(&self, model: &dyn SpeechModel, chunker: &Chunker, polished: &mut Polished) -> bool {
        let (pcm, span, end) = {
            let a = self.audio.samples.lock().unwrap();
            let Some(cut) = chunker.cut(&a[polished.start()..]) else { return false };
            let end = polished.start() + cut;
            let span = polished.span(end, a.len());
            (a[span.clone()].to_vec(), span, end)
        };
        let t = Instant::now();
        polished.add(&self.decode_final(model, &pcm, span.start), span, Some(end));
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let mut s = self.ctl.stats.lock().unwrap();
        s.chunks += 1;
        s.chunk_decode_ms_total += ms;
        s.chunk_decode_ms_max = s.chunk_decode_ms_max.max(ms);
        true
    }

    /// `Final`-mode decode of `pcm`, which starts `offset` samples into the recording. Returns
    /// the words with absolute times; failures become `Error` events.
    fn decode_final(&self, model: &dyn SpeechModel, pcm: &[f32], offset: usize) -> Vec<Word> {
        let speech = self.engine.speech(pcm);
        let hyp = {
            let _g = self.engine.decode_lock.lock().unwrap();
            model.transcribe(&pcm[speech.clone()], DecodeMode::Final)
        };
        match hyp {
            Ok(h) => h.timed_words((offset + speech.start) as f32 / SR as f32, speech.len() as f32 / SR as f32),
            Err(e) => {
                self.send(TranscriptEvent::Error(format!("final decode failed: {e:#}")));
                Vec::new()
            }
        }
    }

    fn finish(&self, text: String, end: Instant, decode_start: Instant, confirmed: String) {
        let now = Instant::now();
        let audio_secs = self.audio.len() as f32 / SR as f32;
        let finalize_ms = now.duration_since(end).as_millis() as u32;
        {
            let mut s = self.ctl.stats.lock().unwrap();
            s.final_wait_ms = decode_start.saturating_duration_since(end).as_secs_f64() * 1e3;
            s.final_decode_ms = now.duration_since(decode_start).as_secs_f64() * 1e3;
            s.confirmed_at_stop = confirmed;
            s.audio_secs = audio_secs;
            s.input_peak = self.audio.peak();
            s.finished = true;
        }
        self.send(TranscriptEvent::Final { text, audio_secs, finalize_ms });
    }
}

/// The preview: polished text, then the words confirmed after it, as `confirmed`; the words
/// still changing as `provisional`.
fn preview(polished: &Polished, la: &LocalAgreement) -> (String, String) {
    let confirmed = [polished.text(), chunks::join(&polished.without_repeat(la.confirmed_words().to_vec()))];
    let confirmed = confirmed.into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ");
    (confirmed, la.provisional_text().to_string())
}
