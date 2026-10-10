//! sherpa-onnx adapters: Parakeet TDT (offline), Nemotron streaming (online), Silero VAD.
//! The online model is decoded like the offline ones: each buffer is streamed through it.

use crate::models::{ModelInfo, SILERO_VAD_PATH};
use crate::{EngineConfig, Hypothesis, SAMPLE_RATE, SpeechModel, Word};
use anyhow::{Context, bail};
use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OnlineRecognizer, OnlineRecognizerConfig, OnlineStream, VadModelConfig,
    VoiceActivityDetector,
};
use std::ops::Range;
use std::path::Path;
use std::sync::Mutex;

/// ONNX Runtime execution provider: "cpu" (default) or e.g. "coreml" via EASYVOX_ORT_PROVIDER.
fn provider() -> String {
    std::env::var("EASYVOX_ORT_PROVIDER").unwrap_or_else(|_| "cpu".into())
}

fn file(dir: &Path, name: &str) -> anyhow::Result<String> {
    let p = dir.join(name);
    if !p.exists() {
        bail!("missing model file {}", p.display());
    }
    Ok(p.to_string_lossy().into_owned())
}

/// NeMo BPE tokens start a new word with a leading space or '▁'.
fn tokens_to_words(tokens: &[String], starts: Option<&[f32]>, durations: Option<&[f32]>) -> Vec<Word> {
    let Some(starts) = starts else { return Vec::new() };
    let mut words: Vec<Word> = Vec::new();
    for (i, tok) in tokens.iter().enumerate() {
        let start = starts.get(i).copied().unwrap_or(0.0);
        let dur = durations.and_then(|d| d.get(i).copied()).filter(|d| *d > 0.0).unwrap_or(0.08);
        let new_word = tok.starts_with(' ') || tok.starts_with('\u{2581}') || words.is_empty();
        let piece = tok.trim_start_matches([' ', '\u{2581}']);
        if new_word {
            words.push(Word { text: piece.to_string(), start, end: start + dur });
        } else if let Some(w) = words.last_mut() {
            w.text.push_str(piece);
            w.end = start + dur;
        }
    }
    words.retain(|w| !w.text.is_empty());
    words
}

pub struct ParakeetModel {
    info: &'static ModelInfo,
    rec: OfflineRecognizer,
}

impl ParakeetModel {
    pub fn load(info: &'static ModelInfo, cfg: &EngineConfig) -> anyhow::Result<Self> {
        let dir = info.resolve(&cfg.models_dir);
        let mut c = OfflineRecognizerConfig::default();
        c.model_config.transducer.encoder = Some(file(&dir, "encoder.int8.onnx")?);
        c.model_config.transducer.decoder = Some(file(&dir, "decoder.int8.onnx")?);
        c.model_config.transducer.joiner = Some(file(&dir, "joiner.int8.onnx")?);
        c.model_config.tokens = Some(file(&dir, "tokens.txt")?);
        c.model_config.model_type = Some("nemo_transducer".into());
        c.model_config.num_threads = cfg.threads as i32;
        c.model_config.provider = Some(provider());
        c.decoding_method = Some("greedy_search".into());
        let rec = OfflineRecognizer::create(&c)
            .with_context(|| format!("sherpa-onnx could not create recognizer from {}", dir.display()))?;
        Ok(Self { info, rec })
    }
}

impl SpeechModel for ParakeetModel {
    fn info(&self) -> &ModelInfo {
        self.info
    }

    fn transcribe(&self, pcm: &[f32]) -> anyhow::Result<Hypothesis> {
        if pcm.len() < (SAMPLE_RATE as usize) / 10 {
            return Ok(Hypothesis::default());
        }
        let stream = self.rec.create_stream();
        stream.accept_waveform(SAMPLE_RATE as i32, pcm);
        self.rec.decode(&stream);
        let r = stream.get_result().context("sherpa-onnx returned no result")?;
        let words = tokens_to_words(&r.tokens, r.timestamps.as_deref(), r.durations.as_deref());
        Ok(Hypothesis { text: r.text.trim().to_string(), words })
    }

    fn word_timings(&self) -> bool {
        true
    }
}

pub struct StreamingModel {
    info: &'static ModelInfo,
    rec: OnlineRecognizer,
}

impl StreamingModel {
    pub fn load(info: &'static ModelInfo, cfg: &EngineConfig) -> anyhow::Result<Self> {
        let dir = info.resolve(&cfg.models_dir);
        let mut c = OnlineRecognizerConfig::default();
        c.model_config.transducer.encoder = Some(file(&dir, "encoder.int8.onnx")?);
        c.model_config.transducer.decoder = Some(file(&dir, "decoder.int8.onnx")?);
        c.model_config.transducer.joiner = Some(file(&dir, "joiner.int8.onnx")?);
        c.model_config.tokens = Some(file(&dir, "tokens.txt")?);
        c.model_config.num_threads = cfg.threads as i32;
        c.model_config.provider = Some(provider());
        c.decoding_method = Some("greedy_search".into());
        c.enable_endpoint = false;
        let rec = OnlineRecognizer::create(&c)
            .with_context(|| format!("sherpa-onnx could not create online recognizer from {}", dir.display()))?;
        Ok(Self { info, rec })
    }

    fn drain(&self, stream: &OnlineStream) {
        while self.rec.is_ready(stream) {
            self.rec.decode(stream);
        }
    }
}

impl SpeechModel for StreamingModel {
    fn info(&self) -> &ModelInfo {
        self.info
    }

    /// Streams the whole buffer through. No word timings: the session spreads the words.
    fn transcribe(&self, pcm: &[f32]) -> anyhow::Result<Hypothesis> {
        let stream = self.rec.create_stream();
        // Without leading context the cache-aware encoder drops the first word when speech
        // starts at t=0 (seen on every TTS clip). 0.5 s of silence fixes it; the tail padding
        // gets the last chunk decoded.
        let sr = SAMPLE_RATE as i32;
        stream.accept_waveform(sr, &vec![0.0; SAMPLE_RATE as usize / 2]);
        stream.accept_waveform(sr, pcm);
        stream.accept_waveform(sr, &vec![0.0; (SAMPLE_RATE as usize) * 6 / 10]);
        stream.input_finished();
        self.drain(&stream);
        let text = self.rec.get_result(&stream).map(|r| r.text.trim().to_string()).unwrap_or_default();
        Ok(Hypothesis { text, words: Vec::new() })
    }
}

/// Silero VAD silence trimmer for the final pass.
pub struct VadTrimmer {
    vad: Mutex<VoiceActivityDetector>,
}

impl VadTrimmer {
    pub fn load(cfg: &EngineConfig) -> anyhow::Result<Self> {
        let path = cfg.models_dir.join(SILERO_VAD_PATH);
        if !path.exists() {
            bail!("vad_trim is on but {} is missing", path.display());
        }
        let mut c = VadModelConfig::default();
        c.silero_vad.model = Some(path.to_string_lossy().into_owned());
        c.silero_vad.threshold = 0.5;
        c.silero_vad.min_silence_duration = 0.3;
        c.silero_vad.min_speech_duration = 0.1;
        c.silero_vad.max_speech_duration = 60.0;
        c.silero_vad.window_size = 512;
        c.sample_rate = SAMPLE_RATE as i32;
        c.num_threads = 1;
        let vad = VoiceActivityDetector::create(&c, 120.0).context("could not create Silero VAD")?;
        Ok(Self { vad: Mutex::new(vad) })
    }

    /// The span from the first to the last detected speech (with padding), or an empty range
    /// if no speech was found.
    pub fn speech(&self, pcm: &[f32]) -> Range<usize> {
        let vad = self.vad.lock().unwrap();
        vad.reset();
        for chunk in pcm.chunks(512) {
            vad.accept_waveform(chunk);
        }
        vad.flush();
        let (mut first, mut last) = (usize::MAX, 0usize);
        while let Some((s, n)) = vad.front().map(|seg| (seg.start().max(0) as usize, seg.n().max(0) as usize)) {
            first = first.min(s);
            last = last.max(s + n);
            vad.pop();
        }
        if first == usize::MAX {
            return 0..0;
        }
        let pad_before = (SAMPLE_RATE as usize) / 4; // 250 ms
        let pad_after = (SAMPLE_RATE as usize) * 3 / 10; // 300 ms
        let a = first.saturating_sub(pad_before);
        let b = (last + pad_after).min(pcm.len());
        a..b
    }
}
