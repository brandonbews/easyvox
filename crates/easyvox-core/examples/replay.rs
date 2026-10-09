//! Minimal push-to-talk loop with a WAV file standing in for the microphone: the same calls
//! the app makes.
//!
//!   cargo run --release -p easyvox-core --example replay -- [model-id] [clip.wav]

use easyvox_core::{AudioSource, Engine, EngineConfig, TranscriptEvent, paths};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let model = args.next().unwrap_or_else(|| easyvox_core::RECOMMENDED_MODEL.into());
    let wav = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/punct_samantha.wav")));

    let mut config = EngineConfig::new(model, paths::models_dir());
    config.final_redecode = true; // streaming models: one offline pass over the whole recording on release
    let engine = Engine::load(config)?; // blocking: off the UI thread in an app
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
        }
    }
    Ok(())
}
