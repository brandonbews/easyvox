//! Model catalog. Adding a model means adding an entry here, plus an adapter if it needs a new
//! runtime. `download.rs` downloads these entries in the app; `scripts/fetch-model.sh` mirrors
//! the GGUF entries' URLs and checksums for developers.
//!
//! Every entry says whether the app offers it ([`Offering`]). The app lists only the default and
//! the alternatives; the rest stay in the catalog for `easyvox-bench` and comparisons. Which
//! model sits where, and why, is in `DESIGN.md` ("Models") and `docs/model-bench.md`.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Runtime {
    /// transcribe.cpp (ggml; Metal on macOS). GGUF models. Feature `transcribe-cpp`.
    TranscribeCpp,
    /// sherpa-onnx offline recognizer (ONNX Runtime, CPU). Feature `sherpa`.
    SherpaOffline,
    /// sherpa-onnx online (natively streaming) recognizer, fed each buffer whole. Feature `sherpa`.
    SherpaOnline,
}

impl Runtime {
    /// The easyvox-core Cargo feature that compiles this runtime in.
    pub fn cargo_feature(self) -> &'static str {
        match self {
            Runtime::TranscribeCpp => "transcribe-cpp",
            Runtime::SherpaOffline | Runtime::SherpaOnline => "sherpa",
        }
    }

    /// Whether this build of easyvox-core can run models for this runtime.
    pub fn is_compiled(self) -> bool {
        match self {
            Runtime::TranscribeCpp => cfg!(feature = "transcribe-cpp"),
            Runtime::SherpaOffline | Runtime::SherpaOnline => cfg!(feature = "sherpa"),
        }
    }
}

/// Where a model shows up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Offering {
    /// The app's model: downloaded during onboarding, used unless the user picks another.
    /// Exactly one entry has it, and it is [`crate::RECOMMENDED_MODEL`].
    Default,
    /// Listed in Settings; the user downloads and activates it.
    Alternative,
    /// Not offered in the app. Kept for `easyvox-bench` and comparisons.
    BenchOnly,
}

impl Offering {
    /// Offered in the app (the default or an alternative).
    pub fn in_app(self) -> bool {
        self != Offering::BenchOnly
    }
}

/// A model's license, as its upstream model card states it. A GGUF repo's own license tag can
/// be wrong (the Parakeet Unified GGUF repo says CC-BY-4.0); upstream wins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct License {
    pub name: &'static str,
    /// The license text.
    pub url: &'static str,
}

pub const NVIDIA_OPEN_MODEL_LICENSE: License = License {
    name: "NVIDIA Open Model License",
    url: "https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/",
};
/// The credit line the NVIDIA Open Model License asks for, word for word.
pub const NVIDIA_OPEN_MODEL_NOTICE: &str = "Licensed by NVIDIA Corporation under the NVIDIA Open Model License";
pub const CC_BY_4_0: License = License { name: "CC-BY-4.0", url: "https://creativecommons.org/licenses/by/4.0/" };
pub const APACHE_2_0: License = License { name: "Apache-2.0", url: "https://www.apache.org/licenses/LICENSE-2.0" };

#[derive(Clone, Debug)]
pub struct ModelInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub runtime: Runtime,
    /// Offered in the app, and in which role.
    pub offering: Offering,
    /// File (GGUF) or directory (sherpa-onnx) relative to `EngineConfig::models_dir`.
    pub path: &'static str,
    /// Hugging Face downloads are pinned to a commit (`resolve/<commit>/<file>`), so a
    /// re-upload can't break the sha256 check.
    pub download_url: &'static str,
    /// SHA-256 of the downloaded file, when the download is the model file itself.
    pub sha256: Option<&'static str>,
    /// Bytes downloaded (archive size for .tar.bz2).
    pub download_bytes: u64,
    /// Bytes on disk after extraction (model files only).
    pub disk_bytes: u64,
    /// Outputs punctuation and capitalization.
    pub punctuation: bool,
    pub license: License,
    /// Credit line to show next to the model (who made it, under which license).
    pub attribution: &'static str,
    /// Upstream model card: the source for the license and the credit.
    pub card_url: &'static str,
}

impl ModelInfo {
    pub fn resolve(&self, models_dir: &Path) -> PathBuf {
        models_dir.join(self.path)
    }

    /// Downloaded into `models_dir` and runnable by this build. A single-file model (one with a
    /// sha256) must also be its full size, so a truncated copy counts as not downloaded. The
    /// sha256 itself is only checked when downloading: hashing 500 MB at every launch would cost
    /// energy for nothing.
    pub fn is_available(&self, models_dir: &Path) -> bool {
        let path = self.resolve(models_dir);
        self.runtime.is_compiled()
            && match self.sha256 {
                Some(_) => std::fs::metadata(&path).is_ok_and(|m| m.is_file() && m.len() == self.download_bytes),
                None => path.exists(),
            }
    }

    /// Where [`crate::download`] writes the file while it downloads: hidden, next to the model.
    pub fn partial_path(&self, models_dir: &Path) -> PathBuf {
        let path = self.resolve(models_dir);
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        path.with_file_name(format!(".{name}.part"))
    }
}

pub static CATALOG: &[ModelInfo] = &[
    ModelInfo {
        id: "parakeet-tdt-0.6b-v2-q5_k_m-gguf",
        name: "Parakeet TDT 0.6B v2 (recommended)",
        runtime: Runtime::TranscribeCpp,
        offering: Offering::Default,
        path: "gguf/parakeet-tdt-0.6b-v2-Q5_K_M.gguf",
        download_url: "https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v2-gguf/resolve/382796e405a9fe37145938dccbabfb25e93e4286/parakeet-tdt-0.6b-v2-Q5_K_M.gguf",
        sha256: Some("dfa904520b95451599683613fea47766c378c5fdf5d8cddf48151226b6eaec85"),
        download_bytes: 539_012_608,
        disk_bytes: 539_012_608,
        punctuation: true,
        license: CC_BY_4_0,
        attribution: "Parakeet TDT 0.6B v2 by NVIDIA, licensed under CC BY 4.0",
        card_url: "https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2",
    },
    ModelInfo {
        id: "parakeet-unified-en-0.6b-q5_k_m-gguf",
        name: "Parakeet Unified 0.6B (best public benchmark scores)",
        runtime: Runtime::TranscribeCpp,
        offering: Offering::Alternative,
        path: "gguf/parakeet-unified-en-0.6b-Q5_K_M.gguf",
        download_url: "https://huggingface.co/handy-computer/parakeet-unified-en-0.6b-gguf/resolve/d5249700b2382bf5c5024c2421d101b8db54a629/parakeet-unified-en-0.6b-Q5_K_M.gguf",
        sha256: Some("f9def6f9b4e83ab7d006df3e1b676dfa1f973a3b6da232a9c99fcaa66bcd2836"),
        download_bytes: 540_795_264,
        disk_bytes: 540_795_264,
        punctuation: true,
        license: NVIDIA_OPEN_MODEL_LICENSE,
        attribution: NVIDIA_OPEN_MODEL_NOTICE,
        card_url: "https://huggingface.co/nvidia/parakeet-unified-en-0.6b",
    },
    ModelInfo {
        id: "whisper-base.en-q8_0-gguf",
        name: "Whisper base.en (smallest)",
        runtime: Runtime::TranscribeCpp,
        offering: Offering::Alternative,
        path: "gguf/whisper-base.en-Q8_0.gguf",
        download_url: "https://huggingface.co/handy-computer/whisper-base.en-gguf/resolve/f83d8d187299d4fc06e86a6de5084375ceac34bf/whisper-base.en-Q8_0.gguf",
        sha256: Some("3b46ca40bccbf7609c68d88a36d96077a04ca7c87f2060ede06f129fac3e7652"),
        download_bytes: 84_886_208,
        disk_bytes: 84_886_208,
        punctuation: true,
        license: APACHE_2_0,
        attribution: "Whisper base.en by OpenAI, licensed under Apache 2.0",
        card_url: "https://huggingface.co/openai/whisper-base.en",
    },
    // Bench only: on transcribe-cpp 0.3.1 it writes numbers as words ("three hundred and forty
    // two dollars"), so its WER on our clips is 7 % against Whisper base.en's 1.4 %. See
    // docs/model-bench.md.
    ModelInfo {
        id: "canary-180m-flash-q5_k_m-gguf",
        name: "Canary 180M Flash",
        runtime: Runtime::TranscribeCpp,
        offering: Offering::BenchOnly,
        path: "gguf/canary-180m-flash-Q5_K_M.gguf",
        download_url: "https://huggingface.co/handy-computer/canary-180m-flash-gguf/resolve/46b86384d1db59108d6e060a0fc00924cb58499f/canary-180m-flash-Q5_K_M.gguf",
        sha256: Some("73c4861a43fe848cc7a64c179c08da04e985efac03f3e4684a3f5e94a1bb33fe"),
        download_bytes: 158_704_416,
        disk_bytes: 158_704_416,
        punctuation: true,
        license: CC_BY_4_0,
        attribution: "Canary 180M Flash by NVIDIA, licensed under CC BY 4.0",
        card_url: "https://huggingface.co/nvidia/canary-180m-flash",
    },
    // Bench only: v2's multilingual successor (25 European languages, picked per recording). On
    // the author's voice it was no more accurate than v2, wrote hesitations as words, and used
    // more CPU and memory. See docs/model-bench.md ("Parakeet TDT v3 against v2").
    ModelInfo {
        id: "parakeet-tdt-0.6b-v3-q5_k_m-gguf",
        name: "Parakeet TDT 0.6B v3 (multilingual)",
        runtime: Runtime::TranscribeCpp,
        offering: Offering::BenchOnly,
        path: "gguf/parakeet-tdt-0.6b-v3-Q5_K_M.gguf",
        download_url: "https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v3-gguf/resolve/90f082450fcbacdb54e5900c44ef697c9ea59622/parakeet-tdt-0.6b-v3-Q5_K_M.gguf",
        sha256: Some("cc722e76adc1a629fc0b2535de879d99b8160d07ad4c0215e2ca7d7ea0ae4b8f"),
        download_bytes: 548_946_272,
        disk_bytes: 548_946_272,
        punctuation: true,
        license: CC_BY_4_0,
        attribution: "Parakeet TDT 0.6B v3 by NVIDIA, licensed under CC BY 4.0",
        card_url: "https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3",
    },
    ModelInfo {
        id: "parakeet-tdt-0.6b-v2-int8",
        name: "Parakeet TDT 0.6B v2 (CPU, sherpa-onnx)",
        runtime: Runtime::SherpaOffline,
        offering: Offering::BenchOnly,
        path: "parakeet-tdt-0.6b-v2-int8",
        download_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8.tar.bz2",
        sha256: None,
        download_bytes: 482_468_385,
        disk_bytes: 661_190_513,
        punctuation: true,
        license: CC_BY_4_0,
        attribution: "Parakeet TDT 0.6B v2 by NVIDIA, licensed under CC BY 4.0",
        card_url: "https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2",
    },
    ModelInfo {
        id: "nemotron-streaming-en-0.6b-560ms-int8",
        name: "Nemotron Speech Streaming 0.6B (CPU, sherpa-onnx)",
        runtime: Runtime::SherpaOnline,
        offering: Offering::BenchOnly,
        path: "nemotron-streaming-en-0.6b-560ms-int8",
        download_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemotron-speech-streaming-en-0.6b-560ms-int8-2026-04-25.tar.bz2",
        sha256: None,
        download_bytes: 463_945_051,
        disk_bytes: 661_919_416,
        punctuation: true,
        license: NVIDIA_OPEN_MODEL_LICENSE,
        attribution: NVIDIA_OPEN_MODEL_NOTICE,
        card_url: "https://huggingface.co/nvidia/nemotron-speech-streaming-en-0.6b",
    },
];

/// Silero VAD (used for optional silence trimming), relative to `models_dir`.
pub const SILERO_VAD_PATH: &str = "vad/silero_vad.onnx";
pub const SILERO_VAD_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx";

pub fn catalog() -> &'static [ModelInfo] {
    CATALOG
}

/// The models the app offers: the default first, then the alternatives in catalog order.
pub fn offered() -> Vec<&'static ModelInfo> {
    let mut v: Vec<_> = CATALOG.iter().filter(|m| m.offering.in_app()).collect();
    v.sort_by_key(|m| m.offering); // stable: keeps catalog order within a role
    v
}

pub fn find(id: &str) -> anyhow::Result<&'static ModelInfo> {
    CATALOG.iter().find(|m| m.id == id).ok_or_else(|| {
        let ids: Vec<_> = CATALOG.iter().map(|m| m.id).collect();
        anyhow::anyhow!("unknown model id {id:?}; known: {}", ids.join(", "))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_recommended_model_is_listed() {
        let mut ids: Vec<_> = CATALOG.iter().map(|m| m.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), CATALOG.len());
        assert_eq!(find(crate::RECOMMENDED_MODEL).unwrap().runtime, Runtime::TranscribeCpp);
    }

    #[test]
    fn exactly_one_default_and_it_is_the_recommended_model() {
        let defaults: Vec<_> = CATALOG.iter().filter(|m| m.offering == Offering::Default).map(|m| m.id).collect();
        assert_eq!(defaults, [crate::RECOMMENDED_MODEL]);
        let offered = offered();
        assert_eq!(offered[0].id, crate::RECOMMENDED_MODEL, "the default comes first");
        assert!(offered[1..].iter().all(|m| m.offering == Offering::Alternative));
    }

    /// The app compiles in only the runtimes it lists in its manifest, so an offered model on
    /// another runtime would never show up in Settings.
    #[test]
    fn offered_models_run_on_a_runtime_the_app_compiles_in() {
        let manifest = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../easyvox-app/Cargo.toml"));
        let core_dep = manifest
            .lines()
            .find(|l| l.trim_start().starts_with("easyvox-core"))
            .expect("easyvox-app depends on easyvox-core");
        for m in offered() {
            let feature = format!("\"{}\"", m.runtime.cargo_feature());
            assert!(core_dep.contains(&feature), "{}: the app doesn't enable {feature}: {core_dep}", m.id);
        }
    }

    #[test]
    fn checksums_are_sha256_hex() {
        for m in CATALOG {
            if let Some(h) = m.sha256 {
                assert!(
                    h.len() == 64 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
                    "{}: not a lowercase sha256: {h}",
                    m.id
                );
            }
        }
        for m in offered() {
            assert!(m.sha256.is_some(), "{}: offered models need a checksum", m.id);
        }
    }

    /// Hugging Face URLs must be `https://huggingface.co/<org>/<repo>/resolve/<40-hex commit>/<file>`,
    /// with `<file>` the catalog path's file name.
    #[test]
    fn hugging_face_urls_are_pinned_to_a_commit() {
        for m in CATALOG {
            let Some(rest) = m.download_url.strip_prefix("https://huggingface.co/") else {
                assert!(!m.offering.in_app(), "{}: offered models download from Hugging Face", m.id);
                continue;
            };
            let parts: Vec<&str> = rest.split('/').collect();
            assert!(parts.len() == 5 && parts[2] == "resolve", "{}: unexpected URL shape {}", m.id, m.download_url);
            let commit = parts[3];
            assert!(
                commit.len() == 40 && commit.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
                "{}: URL isn't pinned to a commit: {}",
                m.id,
                m.download_url
            );
            assert_eq!(Some(parts[4]), Path::new(m.path).file_name().and_then(|f| f.to_str()), "{}", m.id);
        }
    }

    /// A truncated copy doesn't count as downloaded; one of the right size does. Sparse files,
    /// so no real model is needed.
    #[test]
    fn is_available_checks_the_size() {
        let dir = std::env::temp_dir().join(format!("easyvox-models-test-{}", std::process::id()));
        let m = find(crate::RECOMMENDED_MODEL).unwrap();
        let path = m.resolve(&dir);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        assert!(!m.is_available(&dir), "missing");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(m.download_bytes - 1).unwrap();
        assert!(!m.is_available(&dir), "one byte short");
        file.set_len(m.download_bytes).unwrap();
        assert!(m.is_available(&dir), "full size");
        assert_eq!(m.partial_path(&dir), dir.join("gguf/.parakeet-tdt-0.6b-v2-Q5_K_M.gguf.part"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn every_model_names_its_license() {
        for m in CATALOG {
            assert!(!m.license.name.is_empty() && m.license.url.starts_with("https://"), "{}", m.id);
            assert!(!m.attribution.is_empty() && m.card_url.starts_with("https://huggingface.co/"), "{}", m.id);
            if m.license == NVIDIA_OPEN_MODEL_LICENSE {
                assert_eq!(m.attribution, NVIDIA_OPEN_MODEL_NOTICE, "{}", m.id);
            }
        }
    }
}
