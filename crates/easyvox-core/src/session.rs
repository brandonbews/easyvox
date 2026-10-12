//! Push-to-talk session: a capture thread and a decode worker, the same for every model.
//!
//! Capture starts at once. The worker first makes sure the model is loaded (reloading it if the
//! engine was unloaded while idle; the audio meanwhile just buffers), then waits for speech
//! (`pause::SpeechDetector`): until the recording holds some, nothing is decoded, and a
//! recording that never does ends with an empty `Final` and no decode at all. The first tick
//! comes once `preview_first_ms` of audio has followed the start of speech, the next ones as
//! often until the preview shows words, then every `preview_interval_ms` ([`Tuning`], the
//! session's own copy, taken when it started) while the key is held. Each tick it polishes the
//! oldest chunk if enough audio waits (`chunks.rs`), then re-decodes the newest audio for the
//! preview, confirming words once two passes agree (`partials.rs`). That pass starts just before
//! the confirmed words (`preview_context_ms`), so it only decodes what may still change. A tick
//! skips the re-decode when nothing has been said since the last ones (`pause::SpeechGate`).
//! On release it polishes the rest and sends `Final`.
//!
//! A third thread runs the level meter while capture lasts (`level.rs`): it sends `Level` when
//! the input's loudness changes visibly, and `NoInput` when audio stops reaching the session.
//! It never waits on a decode, so the meter keeps moving while the model loads or decodes.

use crate::audio::{MicInfo, SharedAudio};
use crate::chunks::{self, Chunker, Polished};
use crate::level::LevelMeter;
use crate::partials::LocalAgreement;
use crate::pause::{SpeechDetector, SpeechGate};
use crate::tuning::{Tuning, ms_samples};
use crate::{AudioSource, EngineInner, SAMPLE_RATE, SpeechModel, TranscriptEvent, Word};
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
/// How often the worker looks for speech until it has heard some.
const SPEECH_POLL: Duration = Duration::from_millis(100);
/// The level meter never ticks faster than this, whatever `meter_interval_ms` says.
const MIN_METER_TICK_MS: u32 = 10;

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
    /// Audio the preview passes decoded, in seconds, summed over the passes (divide by
    /// `partial_passes` for the mean window).
    pub partial_audio_secs: f64,
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
    /// Where speech started, in seconds of audio (`pause::SpeechDetector`). `None`: the
    /// recording held none, so nothing was decoded and the final text is empty.
    pub speech_onset_secs: Option<f32>,
    /// Confirmed partial text at the moment the final pass started.
    pub confirmed_at_stop: String,
    /// `Level` events sent (the meter moved visibly), and `NoInput` events.
    pub level_events: u32,
    pub no_input_events: u32,
    /// When each `Level` was sent, for tests: whoever receives them may take two at once.
    #[cfg(test)]
    pub level_sent_at: Vec<Instant>,
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
    /// at most a model reload, a chunk decode and the final decode). Waiting matters: if the
    /// process exits while a decode thread is still running, ggml-metal's static destructors
    /// can crash at exit (seen once).
    fn drop(&mut self) {
        self.stop();
        self.join();
    }
}

pub(crate) fn start(
    engine: Arc<EngineInner>,
    source: AudioSource,
    tuning: Tuning,
) -> anyhow::Result<(Session, Receiver<TranscriptEvent>)> {
    let audio = Arc::new(SharedAudio::default());
    let stop_flag = Arc::new(AtomicBool::new(false));
    let ctl = Arc::new(Control { stop: stop_flag.clone(), stop_at: Mutex::new(None), stats: Mutex::default() });
    let (tx, rx) = mpsc::channel();

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

    let mut threads = vec![capture];
    if tuning.meter_interval_ms > 0 {
        threads.push(spawn_meter(audio.clone(), ctl.clone(), tuning, tx.clone())?);
    }
    let worker = {
        let (ctl, audio) = (ctl.clone(), audio.clone());
        std::thread::Builder::new().name("easyvox-decode".into()).spawn(move || {
            let w = Worker { engine, tuning, audio, ctl, tx };
            w.run();
        })?
    };
    threads.push(worker);

    Ok((Session { ctl, audio, threads: Mutex::new(threads) }, rx))
}

/// The level meter's thread: a tick every `meter_interval_ms` while capture lasts, reading the
/// frame levels the capture thread measured (`level.rs`). When capture ends, the meter falls
/// flat and the thread exits, so nothing runs after the session.
fn spawn_meter(
    audio: Arc<SharedAudio>,
    ctl: Arc<Control>,
    tuning: Tuning,
    tx: Sender<TranscriptEvent>,
) -> std::io::Result<JoinHandle<()>> {
    let tick = Duration::from_millis(tuning.meter_interval_ms.max(MIN_METER_TICK_MS) as u64);
    std::thread::Builder::new().name("easyvox-level".into()).spawn(move || {
        let mut meter = LevelMeter::new(&tuning, Instant::now());
        let send = |ev: TranscriptEvent| {
            let mut s = ctl.stats.lock().unwrap();
            match ev {
                TranscriptEvent::Level(_) => {
                    s.level_events += 1;
                    #[cfg(test)]
                    s.level_sent_at.push(Instant::now());
                }
                _ => s.no_input_events += 1,
            }
            drop(s);
            tx.send(ev).is_ok()
        };
        loop {
            // A tick late (the machine was busy) isn't made up for: the next one is a full tick
            // later, so the overlay is never asked to redraw faster than the meter's rate.
            std::thread::sleep(tick);
            let done = audio.done.load(Ordering::SeqCst);
            let reading = {
                let levels = audio.levels.lock().unwrap();
                meter.tick(Instant::now(), levels.levels(), levels.samples())
            };
            if done {
                // Capture ended: rest flat (the overlay shows the final text next).
                if meter.sent() > 0.0 {
                    send(TranscriptEvent::Level(0.0));
                }
                return;
            }
            let sent = reading.no_input.is_none_or(|n| send(TranscriptEvent::NoInput(n)))
                && reading.level.is_none_or(|l| send(TranscriptEvent::Level(l)));
            if !sent {
                return; // nobody is listening any more
            }
        }
    })
}

struct Worker {
    engine: Arc<EngineInner>,
    tuning: Tuning,
    audio: Arc<SharedAudio>,
    ctl: Arc<Control>,
    tx: Sender<TranscriptEvent>,
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

    fn run(self) {
        let started = Instant::now();
        // Reloads the model if the engine was unloaded. Capture already runs, so whatever is
        // said meanwhile buffers and gets decoded once the model is ready.
        let model = self.engine.model();
        self.ctl.stats.lock().unwrap().model_wait_ms = started.elapsed().as_secs_f64() * 1e3;
        match model {
            Ok(model) => self.record(&*model),
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

    fn record(&self, model: &dyn SpeechModel) {
        let t = &self.tuning;
        let chunker = Chunker::new(t);
        let mut polished = Polished::new(model.word_timings(), t);
        let mut la = LocalAgreement::new(t);
        let mut gate = SpeechGate::new(t);
        let preview_on = t.preview_interval_ms > 0;
        let interval = Duration::from_millis(if preview_on { t.preview_interval_ms } else { POLISH_TICK_MS } as u64);
        // Until the preview shows words, passes come as often as the first one (some models need
        // more than `preview_first_ms` of speech before they write anything).
        let until_words = Duration::from_millis(t.preview_first_ms as u64).max(SPEECH_POLL);
        let window = ms_samples(t.preview_window_ms.max(MIN_WINDOW_MS));
        let context = ms_samples(t.preview_context_ms);
        let min_audio = ms_samples(t.preview_min_audio_ms);
        let min_gap = Duration::from_millis(t.preview_min_gap_ms as u64);
        let mut anchor = 0usize;
        let mut decoded_len = 0usize;
        let speech = SpeechDetector::new(t);
        // Where speech starts, once the first tick has come. Nothing is decoded before.
        let mut onset = None;
        let mut next = Instant::now();
        let mut last = (String::new(), String::new());
        loop {
            if onset.is_none() {
                // The first tick: once the recording holds speech (and, with the preview on,
                // enough of it for the first pass).
                onset = self.wait_for_speech(&speech);
                if onset.is_none() {
                    break;
                }
            } else {
                // Sleep until the next tick (new audio alone doesn't wake us early).
                loop {
                    self.wait(next, usize::MAX);
                    if self.done() || Instant::now() >= next {
                        break;
                    }
                }
            }
            if self.done() {
                break;
            }
            let shown = !(last.0.is_empty() && last.1.is_empty());
            next = Instant::now() + if preview_on && !shown { until_words } else { interval };
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
                // Words two passes agreed on are settled: the window starts just before them
                // (never before the last cut), so a pass only re-decodes what may still change.
                let settled = (la.confirmed_until() * SR as f32) as usize;
                anchor = anchor.max(settled.saturating_sub(context).min(total));
                if total - anchor > window {
                    anchor = total - window;
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
                model.transcribe(&window_pcm)
            };
            let ms = t.elapsed().as_secs_f64() * 1e3;
            {
                let mut s = self.ctl.stats.lock().unwrap();
                s.partial_passes += 1;
                s.partial_decode_ms_total += ms;
                s.partial_decode_ms_max = s.partial_decode_ms_max.max(ms);
                s.partial_audio_secs += window_pcm.len() as f64 / SR as f64;
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
        // Released: polish the rest, if the recording held any speech. Once the first tick has
        // come it did, whatever the room sounds like by now.
        let end = self.end_instant();
        let t = Instant::now();
        let confirmed = preview(&polished, &la).0;
        let onset = onset.or_else(|| speech.onset(self.audio.levels.lock().unwrap().levels()));
        self.ctl.stats.lock().unwrap().speech_onset_secs = onset.map(|at| at as f32 / SR as f32);
        if onset.is_some() {
            let (pcm, span) = {
                let a = self.audio.samples.lock().unwrap();
                let span = polished.span(a.len(), a.len());
                (a[span.clone()].to_vec(), span)
            };
            polished.add(&self.decode_final(model, &pcm, span.start), span, None);
        }
        self.finish(polished.text(), end, t, confirmed);
    }

    /// Waits for the first tick: until the recording holds speech and, with the preview on,
    /// `preview_first_ms` of audio has followed its start (and `preview_min_audio_ms` in all).
    /// Returns where speech starts, or `None` if the input ended first.
    fn wait_for_speech(&self, speech: &SpeechDetector) -> Option<usize> {
        let t = &self.tuning;
        let (after, min_audio) = match t.preview_interval_ms {
            0 => (0, 0),
            _ => (ms_samples(t.preview_first_ms), ms_samples(t.preview_min_audio_ms)),
        };
        let mut due = usize::MAX;
        loop {
            // Look again every 100 ms, or as soon as the audio reaches the first tick.
            self.wait(Instant::now() + SPEECH_POLL, due.saturating_sub(1));
            if self.done() {
                return None;
            }
            let levels = self.audio.levels.lock().unwrap();
            let onset = speech.onset(levels.levels());
            due = onset.map_or(usize::MAX, |at| (at + after).max(min_audio));
            if levels.samples() >= due {
                return onset;
            }
        }
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

    /// The final decode of `pcm`, which starts `offset` samples into the recording. Returns
    /// the words with absolute times; failures become `Error` events.
    fn decode_final(&self, model: &dyn SpeechModel, pcm: &[f32], offset: usize) -> Vec<Word> {
        let speech = self.engine.speech(pcm);
        let hyp = {
            let _g = self.engine.decode_lock.lock().unwrap();
            model.transcribe(&pcm[speech.clone()])
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pause::tests::audio;
    use crate::{Engine, EngineConfig, Hypothesis, ModelInfo, models};
    use std::sync::atomic::AtomicU32;

    /// A model that hears "hello" in anything, and counts its decodes.
    #[derive(Default)]
    struct Counting {
        decodes: AtomicU32,
    }

    impl SpeechModel for Counting {
        fn info(&self) -> &ModelInfo {
            &models::catalog()[0]
        }

        fn transcribe(&self, _pcm: &[f32]) -> anyhow::Result<Hypothesis> {
            self.decodes.fetch_add(1, Ordering::SeqCst);
            Ok(Hypothesis { text: "hello".into(), words: vec![] })
        }
    }

    /// Writes `pcm` to a WAV file in a temporary folder.
    fn write_wav(name: &str, pcm: &[f32]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("easyvox-session-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join(format!("{name}.wav"));
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(&wav, spec).unwrap();
        pcm.iter().for_each(|x| writer.write_sample(*x).unwrap());
        writer.finalize().unwrap();
        wav
    }

    /// Replays `pcm` in real time through a session on a [`Counting`] model. Returns the final
    /// text, the number of decodes and the session's counters.
    fn replay(name: &str, pcm: &[f32]) -> (String, u32, SessionStats) {
        let wav = write_wav(name, pcm);
        let model = Arc::new(Counting::default());
        let engine = Engine::from_model(model.clone(), EngineConfig::new("counting", wav.parent().unwrap())).unwrap();
        let (session, events) = engine.start(AudioSource::WavReplay(wav.clone())).unwrap();
        let text = loop {
            if let TranscriptEvent::Final { text, .. } = events.recv_timeout(Duration::from_secs(10)).unwrap() {
                break text;
            }
        };
        session.join();
        let _ = std::fs::remove_file(&wav);
        (text, model.decodes.load(Ordering::SeqCst), session.stats())
    }

    #[test]
    fn silence_and_room_noise_are_never_decoded() {
        // Digital silence, and room noise at -46 dBFS after a Bluetooth mic's first zeros.
        for (name, pcm) in [("silence", audio(&[(1.5, 0.0)])), ("room", audio(&[(0.3, 0.0), (1.5, 0.007)]))] {
            let (text, decodes, stats) = replay(name, &pcm);
            assert_eq!((text.as_str(), decodes, stats.speech_onset_secs), ("", 0, None), "{name}");
            assert!(stats.finished, "{name}");
        }
    }

    /// Replays a WAV file through a session on a [`Counting`] model with `tuning`, and returns
    /// every event with the time it arrived (seconds after the start), and the session's counters.
    fn replay_events(wav: &std::path::Path, tuning: Tuning) -> (Vec<(f32, TranscriptEvent)>, SessionStats) {
        let engine =
            Engine::from_model(Arc::new(Counting::default()), EngineConfig::new("counting", wav.parent().unwrap()))
                .unwrap();
        let start = Instant::now();
        let (session, events) = engine.start_with(AudioSource::WavReplay(wav.into()), tuning).unwrap();
        let mut out = vec![];
        for event in events.iter() {
            out.push((start.elapsed().as_secs_f32(), event));
        }
        session.join();
        (out, session.stats())
    }

    /// [`replay_events`] of a clip in `testdata/`.
    fn replay_clip(clip: &str, tuning: Tuning) -> Vec<(f32, TranscriptEvent)> {
        let wav = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata")).join(clip);
        replay_events(&wav, tuning).0
    }

    fn levels(events: &[(f32, TranscriptEvent)]) -> Vec<(f32, f32)> {
        events
            .iter()
            .filter_map(|(at, e)| if let TranscriptEvent::Level(l) = e { Some((*at, *l)) } else { None })
            .collect()
    }

    #[test]
    fn speech_moves_the_level_meter_at_up_to_its_rate() {
        // 3 s of a voice whose loudness swings by 20 dB four times a second, as syllables do
        // (-13 to -33 dBFS), after 0.5 s of a quiet room.
        let mut pcm = audio(&[(0.5, 0.003)]);
        pcm.extend(crate::level::tests::syllables(3.0));
        let wav = write_wav("syllables", &pcm);
        for interval in [Tuning::default().meter_interval_ms, 33] {
            let (events, stats) = replay_events(&wav, Tuning { meter_interval_ms: interval, ..Tuning::default() });
            let sent = levels(&events);
            let peak = sent.iter().map(|(_, l)| *l).fold(0.0, f32::max);
            assert!(peak > 0.5, "{interval} ms: {sent:?}");
            // At most one level a tick, but no fewest: a busy or throttled machine can sleep the
            // meter's thread longer than a syllable, and then each tick sees a whole one and the
            // meter barely moves. How many ticks send is checked in `level.rs`, on exact times.
            let ticks = 3.5e3 / interval as f32;
            assert!(sent.len() as f32 <= ticks + 2.0, "{interval} ms: {sent:?}");
            // Spaced by when the meter sent them: a busy test thread can receive two at once.
            let fastest = stats.level_sent_at.windows(2).map(|w| w[1] - w[0]).fold(Duration::MAX, Duration::min);
            let tick = Duration::from_millis(interval as u64);
            assert!(fastest > tick / 2, "{interval} ms: two levels sent {fastest:?} apart");
            assert_eq!(sent.last().map(|(_, l)| *l), Some(0.0), "{interval} ms: it rests flat at the end");
        }
        let _ = std::fs::remove_file(&wav);
    }

    #[test]
    fn a_spoken_clip_lifts_the_meter_and_it_rests_after() {
        // A text-to-speech voice from its first frame: steadier than a person, so the meter
        // mostly holds near the top while it speaks.
        let Some(wav) = crate::testdata::clip("cmd_samantha.wav") else { return };
        let (events, _) = replay_events(&wav, Tuning::default());
        let sent = levels(&events);
        assert!(sent.iter().any(|(at, l)| *at < 0.4 && *l > 0.5), "{sent:?}");
        assert_eq!(sent.last().map(|(_, l)| *l), Some(0.0), "{sent:?}");
        assert!(!events.iter().any(|(_, e)| matches!(e, TranscriptEvent::NoInput(_))), "{events:?}");
    }

    #[test]
    fn digital_silence_keeps_the_meter_flat_and_says_it_cant_hear() {
        // 2 s of exact zeros, as a muted microphone gives.
        let events = replay_clip("silence_2s.wav", Tuning::default());
        assert!(levels(&events).iter().all(|(_, l)| *l < 0.05), "{events:?}");
        let said: Vec<_> =
            events.iter().filter_map(|(at, e)| matches!(e, TranscriptEvent::NoInput(true)).then_some(*at)).collect();
        assert!(said.len() == 1 && (1.5..1.8).contains(&said[0]), "{said:?}");
        // With the meter off, neither.
        let off = replay_clip("silence_2s.wav", Tuning { meter_interval_ms: 0, ..Tuning::default() });
        assert!(off.iter().all(|(_, e)| matches!(e, TranscriptEvent::Final { .. })), "{off:?}");
    }

    #[test]
    fn a_short_word_is_previewed_soon_after_it_starts_and_decoded() {
        // A 0.3 s word after 0.4 s of room noise, then 0.6 s more. The first preview pass comes
        // 0.5 s after the word starts, long before the 1.6 s interval.
        let pcm = audio(&[(0.4, 0.007), (0.3, 0.3), (0.6, 0.007)]);
        let (text, decodes, stats) = replay("word", &pcm);
        assert_eq!(text, "hello");
        let onset = stats.speech_onset_secs.unwrap();
        assert!((0.37..=0.4).contains(&onset), "{onset}");
        assert_eq!(stats.partial_passes, 1);
        assert_eq!(decodes, 2, "one preview pass and the final pass");
    }
}
