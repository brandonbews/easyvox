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
    let dir = paths::models_dir();
    let info = models::find(RECOMMENDED_MODEL).unwrap();
    assert!(info.is_available(&dir), "{} is missing; run scripts/fetch-model.sh", info.resolve(&dir).display());
    Engine::load(EngineConfig::recommended(dir)).expect("load model")
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
    let engine = engine();
    let (session, events) = engine.start(AudioSource::WavReplay(clip("punct_samantha.wav"))).unwrap();
    let mut partials = 0;
    let mut last_confirmed = String::new();
    let final_text = loop {
        match events.recv_timeout(Duration::from_secs(30)).expect("session stalled") {
            TranscriptEvent::Partial { confirmed, .. } => {
                assert!(
                    confirmed.starts_with(&last_confirmed),
                    "confirmed text shrank: {last_confirmed:?} -> {confirmed:?}"
                );
                last_confirmed = confirmed;
                partials += 1;
            }
            TranscriptEvent::Final { text, .. } => break text,
            TranscriptEvent::Error(e) => panic!("session error: {e}"),
        }
    };
    session.join();
    assert!(partials >= 3, "only {partials} partials");
    let expected = "Well, here's the thing: if we ship on Friday, and nobody objects, we'll have the whole weekend \
                    to fix bugs. Otherwise, we wait until next Wednesday. Does that sound reasonable?";
    assert_eq!(words(&final_text), words(expected), "got {final_text:?}");
}
