//! End-to-end checks against the real recommended model. They need the model on disk
//! (`scripts/fetch-model.sh`), so they're ignored by default:
//!
//!   cargo test -p easyvox-core --release -- --ignored

#![cfg(feature = "transcribe-cpp")]

use easyvox_core::{AudioSource, Engine, EngineConfig, RECOMMENDED_MODEL, TranscriptEvent, models, paths};
use std::path::PathBuf;
use std::time::Duration;

fn clip(name: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata")).join(name)
}

fn engine() -> Engine {
    engine_with(|_| {})
}

fn engine_with(tweak: impl FnOnce(&mut EngineConfig)) -> Engine {
    let dir = paths::models_dir();
    let info = models::find(RECOMMENDED_MODEL).unwrap();
    assert!(info.is_available(&dir), "{} is missing; run scripts/fetch-model.sh", info.resolve(&dir).display());
    let mut config = EngineConfig::recommended(dir);
    tweak(&mut config);
    Engine::load(config).expect("load model")
}

/// Replays `name` to its end; returns the `Partial` events and the final text.
fn replay(engine: &Engine, name: &str) -> (Vec<(String, String)>, String, easyvox_core::SessionStats) {
    let (session, events) = engine.start(AudioSource::WavReplay(clip(name))).unwrap();
    let mut partials = vec![];
    let final_text = loop {
        match events.recv_timeout(Duration::from_secs(30)).expect("session stalled") {
            TranscriptEvent::Partial { confirmed, provisional } => partials.push((confirmed, provisional)),
            TranscriptEvent::Final { text, .. } => break text,
            TranscriptEvent::Error(e) => panic!("session error: {e}"),
        }
    };
    session.join();
    (partials, final_text, session.stats())
}

/// Word-level edit distance between `words()` of the two texts.
fn word_errors(reference: &str, hypothesis: &str) -> usize {
    let (r, h) = (words(reference), words(hypothesis));
    let mut row: Vec<usize> = (0..=h.len()).collect();
    for (i, rw) in r.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, hw) in h.iter().enumerate() {
            let substitution = diagonal + usize::from(rw != hw);
            diagonal = row[j + 1];
            row[j + 1] = substitution.min(row[j] + 1).min(row[j + 1] + 1);
        }
    }
    row[h.len()]
}

/// Lowercase words without punctuation, with number words up to twenty as digits, so the check
/// ignores formatting choices ("ten" vs "10", "thing:" vs "thing,") but not wrong words.
fn words(s: &str) -> Vec<String> {
    const NUMBERS: &str = "zero one two three four five six seven eight nine ten eleven twelve thirteen fourteen \
                           fifteen sixteen seventeen eighteen nineteen twenty";
    s.split_whitespace()
        .map(|w| w.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect::<String>())
        .filter(|w| !w.is_empty())
        .map(|w| NUMBERS.split(' ').position(|n| n == w).map_or(w, |i| i.to_string()))
        .collect()
}

#[test]
fn words_ignores_formatting_only() {
    assert_eq!(words("I'll be ten minutes late."), words("ill be 10 minutes, late"));
    assert_ne!(words("I'll be ten minutes late."), words("I'll be two minutes late."));
}

#[test]
#[ignore = "needs the recommended model (scripts/fetch-model.sh)"]
fn one_shot_transcription_matches_the_clip() {
    let engine = engine();
    let pcm = easyvox_core::audio::load_wav_16k_mono(&clip("cmd_samantha.wav")).unwrap();
    let text = engine.transcribe_pcm(&pcm).unwrap().text;
    assert_eq!(words(&text), words("Send a message to Sarah saying I'll be ten minutes late."), "got {text:?}");
}

#[test]
#[ignore = "needs the recommended model (scripts/fetch-model.sh)"]
fn replay_session_streams_partials_then_final() {
    let (partials, final_text, stats) = replay(&engine(), "punct_samantha.wav");
    assert!(partials.len() >= 3, "only {} partials", partials.len());
    assert_eq!(stats.chunks, 0, "an 11 s clip is one chunk, polished on release");
    // Without chunk swaps, confirmed text only grows.
    for w in partials.windows(2) {
        assert!(w[1].0.starts_with(&w[0].0), "confirmed text changed: {:?} -> {:?}", w[0].0, w[1].0);
    }
    let expected = "Well, here's the thing: if we ship on Friday, and nobody objects, we'll have the whole weekend \
                    to fix bugs. Otherwise, we wait until next Wednesday. Does that sound reasonable?";
    assert_eq!(words(&final_text), words(expected), "got {final_text:?}");
}

/// The eager final pass on a real model: 8 s chunks on a 30 s clip cut it at pauses while it
/// plays, and the joined text must read like the transcript (no lost or doubled words).
#[test]
#[ignore = "needs the recommended model (scripts/fetch-model.sh)"]
fn chunked_final_pass_joins_cleanly() {
    let engine = engine_with(|c| c.tuning.chunk_ms = 8_000);
    let (partials, final_text, stats) = replay(&engine, "long_daniel.wav");
    assert!(stats.chunks >= 2, "only {} chunks polished while playing", stats.chunks);
    let expected = "The old lighthouse stood at the edge of the cliff for nearly 200 years. Every night, the keeper \
                    climbed the narrow stairs, cleaned the great glass lens, and lit the lamp that guided ships away \
                    from the rocks below. When the storms came in from the west, he would stay awake until dawn, \
                    listening to the wind and watching the dark water for any sign of trouble. Most of the sailors \
                    who passed never knew his name, but many of them owed him their lives. Today the light is \
                    automatic, and the keeper's cottage has become a small museum.";
    let errors = word_errors(expected, &final_text);
    assert!(errors <= 2, "{errors} word errors: {final_text:?}");
    // The preview swaps polished text in at most once per chunk.
    let rewrites = partials.windows(2).filter(|w| !w[1].0.starts_with(&w[0].0)).count();
    assert!(rewrites <= stats.chunks as usize, "{rewrites} rewrites of confirmed text for {} chunks", stats.chunks);
}
