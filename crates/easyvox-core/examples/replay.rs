//! Minimal push-to-talk loop with a WAV file standing in for the microphone: the same calls
//! the app makes.
//!
//!   cargo run --release -p easyvox-core --example replay -- [model-id] [clip.wav]

use easyvox_core::{AudioSource, Engine, EngineConfig, TranscriptEvent, paths};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    for (key, value) in easyvox_core::process_env() {
        // SAFETY: the first thing `main` does, before any thread exists.
        unsafe { std::env::set_var(key, value) };
    }
    let mut args = std::env::args().skip(1);
    let model = args.next().unwrap_or_else(|| easyvox_core::RECOMMENDED_MODEL.into());
    let wav = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/punct_samantha.wav")));

    let engine = Engine::load(EngineConfig::new(model, paths::models_dir()))?; // blocking: off the UI thread in an app
    eprintln!("loaded {} in {} ms", engine.model_info().id, engine.load_ms());

    let (_session, events) = engine.start(AudioSource::WavReplay(wav))?;
    for ev in events {
        match ev {
            TranscriptEvent::Partial { confirmed, provisional } => eprintln!("partial: {confirmed} [{provisional}]"),
            TranscriptEvent::Final { text, audio_secs, finalize_ms } => {
                println!("{text}");
                eprintln!("final after {finalize_ms} ms ({audio_secs:.1} s of audio)");
                break;
            }
            TranscriptEvent::Error(e) => eprintln!("error: {e}"),
            TranscriptEvent::NoInput(true) => eprintln!("can't hear the input"),
            TranscriptEvent::Level(_) | TranscriptEvent::NoInput(false) => {}
        }
    }
    Ok(())
}
