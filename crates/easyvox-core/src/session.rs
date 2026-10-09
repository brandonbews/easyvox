//! Push-to-talk session: capture thread + decode worker.

use crate::audio::{MicInfo, SharedAudio};
use crate::partials::LocalAgreement;
use crate::{AudioSource, DecodeMode, EngineInner, SAMPLE_RATE, TranscriptEvent, Word};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const SR: usize = SAMPLE_RATE as usize;

/// Counters a session collects (for benchmarks and diagnostics).
#[derive(Clone, Debug, Default)]
pub struct SessionStats {
    /// Re-decode passes run for partials (0 for natively streaming models).
    pub partial_passes: u32,
    pub partial_decode_ms_total: f64,
    pub partial_decode_ms_max: f64,
    /// `Partial` events sent.
    pub partial_events: u32,
    /// Streaming models: total ms spent in incremental decode.
    pub stream_decode_ms_total: f64,
    /// Time from stop (or end of WAV) until the worker started the final decode
    /// (waiting for an in-flight partial pass and the capture thread).
    pub final_wait_ms: f64,
    /// Final decode itself (incl. VAD trim).
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
    /// at most one final decode). Waiting matters: if the process exits while a decode thread
    /// is still running, ggml-metal's static destructors can crash at exit (seen once).
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
        let mut warned = false;
        if let Some(mut stream) = self.engine.model.new_stream() {
            // Natively streaming model: feed audio as it arrives.
            let mut fed = 0usize;
            let mut last = (String::new(), String::new());
            loop {
                self.wait(Instant::now() + Duration::from_millis(100), fed);
                self.mic_checks(started, &mut warned);
                let new: Vec<f32> = self.audio.samples.lock().unwrap()[fed..].to_vec();
                if !new.is_empty() {
                    let t = Instant::now();
                    stream.accept(&new);
                    fed += new.len();
                    self.ctl.stats.lock().unwrap().stream_decode_ms_total += t.elapsed().as_secs_f64() * 1e3;
                    let split = stream.partial();
                    if split != last && !(split.0.is_empty() && split.1.is_empty()) {
                        last = split.clone();
                        self.send(TranscriptEvent::Partial { confirmed: split.0, provisional: split.1 });
                    }
                }
                if self.done() && self.audio.len() == fed {
                    break;
                }
            }
            let end = self.end_instant();
            let t = Instant::now();
            let redecode = self.engine.config.final_redecode;
            // With a full re-decode coming, don't spend time flushing the stream.
            let mut text = if redecode {
                drop(stream);
                String::new()
            } else {
                stream.finish()
            };
            if redecode {
                let all = self.audio.samples.lock().unwrap().clone();
                let trimmed = self.engine.trim(&all);
                let _g = self.engine.decode_lock.lock().unwrap();
                match self.engine.model.transcribe(&trimmed, DecodeMode::Final) {
                    Ok(h) => text = h.text,
                    Err(e) => self.send(TranscriptEvent::Error(format!("final re-decode failed: {e:#}"))),
                }
            }
            self.finish(text, end, t, last.0);
        } else {
            self.run_redecode(started, &mut warned);
        }
    }

    fn run_redecode(&self, started: Instant, warned: &mut bool) {
        let cfg = &self.engine.config;
        let mut la = LocalAgreement::new();
        let interval = Duration::from_millis(cfg.partial_interval_ms.max(1) as u64);
        let window = (cfg.partial_window_secs.max(2.0) * SR as f32) as usize;
        let mut anchor = 0usize;
        let mut decoded_len = 0usize;
        let mut next = Instant::now() + interval;
        let mut last = (String::new(), String::new());
        loop {
            // Sleep until the next tick (new audio alone doesn't wake us early).
            loop {
                self.wait(next, usize::MAX);
                if self.done() || Instant::now() >= next {
                    break;
                }
            }
            self.mic_checks(started, warned);
            if self.done() {
                break;
            }
            let pass_start = Instant::now();
            next = pass_start + interval;
            if cfg.partial_interval_ms == 0 {
                continue;
            }
            let window_pcm: Vec<f32>;
            {
                let a = self.audio.samples.lock().unwrap();
                let total = a.len();
                if total < SR * 3 / 10 || total < decoded_len + SR / 10 {
                    continue;
                }
                if total - anchor > window {
                    let frozen = (la.confirmed_until() * SR as f32) as usize;
                    anchor = anchor.max(frozen.min(total));
                    if total - anchor > window {
                        anchor = total - window;
                    }
                }
                window_pcm = a[anchor..total].to_vec();
                decoded_len = total;
            }
            let t = Instant::now();
            let hyp = {
                let _g = self.engine.decode_lock.lock().unwrap();
                self.engine.model.transcribe(&window_pcm, DecodeMode::Partial)
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
                    let off = anchor as f32 / SR as f32;
                    let timed = if h.words.is_empty() && !h.text.is_empty() {
                        // Runtime gave no word timings: spread the words evenly over the window.
                        let ws: Vec<&str> = h.text.split_whitespace().collect();
                        let dur = window_pcm.len() as f32 / SR as f32 / ws.len().max(1) as f32;
                        ws.iter()
                            .enumerate()
                            .map(|(i, t)| Word {
                                text: t.to_string(),
                                start: i as f32 * dur,
                                end: (i + 1) as f32 * dur,
                            })
                            .collect()
                    } else {
                        h.words
                    };
                    let words: Vec<Word> = timed
                        .iter()
                        .map(|w| Word { text: w.text.clone(), start: w.start + off, end: w.end + off })
                        .collect();
                    let cur = la.update(&words);
                    if cur != last && !(cur.0.is_empty() && cur.1.is_empty()) {
                        last = cur.clone();
                        self.send(TranscriptEvent::Partial { confirmed: cur.0, provisional: cur.1 });
                    }
                }
                Err(e) => self.send(TranscriptEvent::Error(format!("partial decode failed: {e:#}"))),
            }
            if Instant::now() >= next {
                next = Instant::now() + Duration::from_millis(50);
            }
        }
        let end = self.end_instant();
        let t = Instant::now();
        let all = self.audio.samples.lock().unwrap().clone();
        let trimmed = self.engine.trim(&all);
        let text = {
            let _g = self.engine.decode_lock.lock().unwrap();
            match self.engine.model.transcribe(&trimmed, DecodeMode::Final) {
                Ok(h) => h.text,
                Err(e) => {
                    self.send(TranscriptEvent::Error(format!("final decode failed: {e:#}")));
                    String::new()
                }
            }
        };
        self.finish(text, end, t, la.confirmed_text());
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
