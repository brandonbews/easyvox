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
    /// As people write it, for the UI: "CC BY 4.0" is Creative Commons' own spelling.
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
pub const CC_BY_4_0: License = License { name: "CC BY 4.0", url: "https://creativecommons.org/licenses/by/4.0/" };
pub const APACHE_2_0: License = License { name: "Apache 2.0", url: "https://www.apache.org/licenses/LICENSE-2.0" };
pub const MIT: License = License { name: "MIT", url: "https://opensource.org/license/mit" };

/// The one-line warranty disclaimer the credits end with (CC BY 4.0 §3(a)(1)(A)(iv) asks for a
/// notice that refers to it; Apache-2.0 §7 has the same disclaimer).
pub const DISCLAIMER: &str = "Provided as is, without warranties of any kind.";

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
    /// One line for Settings, after "Pick this if": who this model is for. From `DESIGN.md`
    /// ("Models"). Empty for bench-only entries.
    pub pick_if: &'static str,
    pub license: License,
    /// Who made the model ("NVIDIA"), as the credit names them.
    pub creator: &'static str,
    /// Credit line to show next to the model: who made it, under which license. It names
    /// `creator` and `license.name`.
    pub attribution: &'static str,
    /// How the file we download differs from the upstream model, and who changed it, ending in
    /// "by <who>". CC BY 4.0 §3(a)(1)(B) and Apache-2.0 §4(b) ask to keep this note.
    pub modification: &'static str,
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
        self.sibling(models_dir, |name| format!(".{name}.part"))
    }

    /// The license notice [`crate::download`] writes next to the model: `<file>.LICENSE.txt`.
    pub fn license_notice_path(&self, models_dir: &Path) -> PathBuf {
        self.sibling(models_dir, |name| format!("{name}.LICENSE.txt"))
    }

    /// A file next to the model, named after it.
    fn sibling(&self, models_dir: &Path, name: impl FnOnce(&str) -> String) -> PathBuf {
        let path = self.resolve(models_dir);
        let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        path.with_file_name(name(&file))
    }

    /// The name without the note in brackets: "Parakeet TDT 0.6B v2".
    pub fn short_name(&self) -> &'static str {
        self.name.split_once(" (").map_or(self.name, |(name, _)| name)
    }

    /// Who made the changes in [`modification`](Self::modification): the words after its last
    /// " by ".
    pub fn modified_by(&self) -> &'static str {
        self.modification.rsplit_once(" by ").map_or("", |(_, who)| who)
    }

    /// Where the modified files come from: the Hugging Face repository the download URL points
    /// into, or the GitHub project of a release download.
    pub fn source_url(&self) -> &'static str {
        let url = self.download_url;
        ["/resolve/", "/releases/download/"].iter().find_map(|cut| url.split_once(cut)).map_or(url, |(repo, _)| repo)
    }

    /// The text of [`license_notice_path`](Self::license_notice_path): the license, the credit,
    /// the modification note and the disclaimer, so the terms travel with the file.
    pub fn license_notice(&self) -> String {
        let title = self.short_name();
        let file = Path::new(self.path).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        format!(
            "{title}\n{underline}\n\n\
             {attribution}.\n\
             License: {license_url}\n\
             Original model: {card}\n\
             \n\
             {modification}:\n\
             {source}\n\
             \n\
             {DISCLAIMER}\n\
             \n\
             EasyVox downloaded {file} from {source}.\n\
             The model isn't part of EasyVox and keeps its own license. EasyVox isn't affiliated\n\
             with or endorsed by {creator}.\n",
            underline = "=".repeat(title.chars().count()),
            attribution = self.attribution.trim_end_matches('.'),
            license_url = self.license.url,
            card = self.card_url,
            modification = self.modification,
            source = self.source_url(),
            creator = self.creator,
        )
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
        pick_if: "you want the best all-round model.",
        license: CC_BY_4_0,
        creator: "NVIDIA",
        attribution: "Parakeet TDT 0.6B v2 by NVIDIA, licensed under CC BY 4.0",
        modification: "Converted to GGUF and quantized to Q5_K_M by handy-computer",
        card_url: "https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2",
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
        pick_if: "you want the smallest model (an 8 GB Mac, or memory matters more than the last bit of \
                  accuracy).",
        // OpenAI's Hugging Face card and the GGUF's own metadata say Apache-2.0; its GitHub
        // release says MIT. We follow the Hugging Face release, which the GGUF is made from.
        license: APACHE_2_0,
        creator: "OpenAI",
        attribution: "Whisper base.en by OpenAI, licensed under Apache 2.0",
        modification: "Converted to GGUF and quantized to Q8_0 by handy-computer",
        card_url: "https://huggingface.co/openai/whisper-base.en",
    },
    // Bench only since 10 October 2026: on the author's voice it was a word behind v2 (2.10 %
    // against 1.68 % WER), as fast, used a little more CPU and often left out the last full
    // stop. Its license would also need an "I agree" step in the app. See DESIGN.md ("Bench
    // only").
    ModelInfo {
        id: "parakeet-unified-en-0.6b-q5_k_m-gguf",
        name: "Parakeet Unified EN 0.6B",
        runtime: Runtime::TranscribeCpp,
        offering: Offering::BenchOnly,
        path: "gguf/parakeet-unified-en-0.6b-Q5_K_M.gguf",
        download_url: "https://huggingface.co/handy-computer/parakeet-unified-en-0.6b-gguf/resolve/d5249700b2382bf5c5024c2421d101b8db54a629/parakeet-unified-en-0.6b-Q5_K_M.gguf",
        sha256: Some("f9def6f9b4e83ab7d006df3e1b676dfa1f973a3b6da232a9c99fcaa66bcd2836"),
        download_bytes: 540_795_264,
        disk_bytes: 540_795_264,
        punctuation: true,
        pick_if: "",
        license: NVIDIA_OPEN_MODEL_LICENSE,
        creator: "NVIDIA",
        attribution: NVIDIA_OPEN_MODEL_NOTICE,
        modification: "Converted to GGUF and quantized to Q5_K_M by handy-computer",
        card_url: "https://huggingface.co/nvidia/parakeet-unified-en-0.6b",
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
        pick_if: "",
        license: CC_BY_4_0,
        creator: "NVIDIA",
        attribution: "Canary 180M Flash by NVIDIA, licensed under CC BY 4.0",
        modification: "Converted to GGUF and quantized to Q5_K_M by handy-computer",
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
        pick_if: "",
        license: CC_BY_4_0,
        creator: "NVIDIA",
        attribution: "Parakeet TDT 0.6B v3 by NVIDIA, licensed under CC BY 4.0",
        modification: "Converted to GGUF and quantized to Q5_K_M by handy-computer",
        card_url: "https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3",
    },
    // Bench only, like the entries after it: the candidates of DESIGN.md's "Other models we
    // looked at", run through the app's pipeline on 11 October 2026 (docs/model-bench.md,
    // "Every candidate, one pipeline"). Q5_K_M where it's published, else the closest.
    ModelInfo {
        id: "whisper-large-v3-turbo-q5_k_m-gguf",
        name: "Whisper large-v3-turbo",
        runtime: Runtime::TranscribeCpp,
        offering: Offering::BenchOnly,
        path: "gguf/whisper-large-v3-turbo-Q5_K_M.gguf",
        download_url: "https://huggingface.co/handy-computer/whisper-large-v3-turbo-gguf/resolve/ceea6c8a94a21ab85be244d311e874a39344dbf5/whisper-large-v3-turbo-Q5_K_M.gguf",
        sha256: Some("977b5db4e004349dffd1ab9caa10ba5aaba3fc3edd3ba72cadb84328a3203e36"),
        download_bytes: 619_628_128,
        disk_bytes: 619_628_128,
        punctuation: true,
        pick_if: "",
        // OpenAI's model card says MIT; the GGUF repo's tag says Apache-2.0. Upstream wins.
        license: MIT,
        creator: "OpenAI",
        attribution: "Whisper large-v3-turbo by OpenAI, licensed under MIT",
        modification: "Converted to GGUF and quantized to Q5_K_M by handy-computer",
        card_url: "https://huggingface.co/openai/whisper-large-v3-turbo",
    },
    // No GGUF is published. transcribe.cpp also loads whisper.cpp's own .bin format, and the
    // authors publish this one, in F16 only.
    ModelInfo {
        id: "distil-whisper-large-v3.5-f16-ggml",
        name: "Distil-Whisper large-v3.5",
        runtime: Runtime::TranscribeCpp,
        offering: Offering::BenchOnly,
        path: "ggml/distil-large-v3.5/ggml-model.bin",
        download_url: "https://huggingface.co/distil-whisper/distil-large-v3.5-ggml/resolve/960ecb5c2ecfba3ebb9ebe485c1032ec266cf436/ggml-model.bin",
        sha256: Some("ec2498919b498c5f6b00041adb45650124b3cd9f26f545fffa8f5d11c28dcf26"),
        download_bytes: 1_519_521_155,
        disk_bytes: 1_519_521_155,
        punctuation: true,
        pick_if: "",
        license: MIT,
        creator: "Hugging Face",
        attribution: "Distil-Whisper large-v3.5 by Hugging Face, licensed under MIT",
        modification: "Converted to whisper.cpp's ggml format (F16) by distil-whisper",
        card_url: "https://huggingface.co/distil-whisper/distil-large-v3.5",
    },
    ModelInfo {
        id: "qwen3-asr-1.7b-q5_k_m-gguf",
        name: "Qwen3-ASR 1.7B",
        runtime: Runtime::TranscribeCpp,
        offering: Offering::BenchOnly,
        path: "gguf/Qwen3-ASR-1.7B-Q5_K_M.gguf",
        download_url: "https://huggingface.co/handy-computer/Qwen3-ASR-1.7B-gguf/resolve/3555bd238a8572bbace3ebf60d23b036dc0a5dbe/Qwen3-ASR-1.7B-Q5_K_M.gguf",
        sha256: Some("034c557fe92ff8fcd9a9c041cbdaad347be0a86a58d3a348f63cf3f0180879d0"),
        download_bytes: 1_517_290_464,
        disk_bytes: 1_517_290_464,
        punctuation: true,
        pick_if: "",
        license: APACHE_2_0,
        creator: "the Qwen team at Alibaba Cloud",
        attribution: "Qwen3-ASR 1.7B by the Qwen team at Alibaba Cloud, licensed under Apache 2.0",
        modification: "Converted to GGUF and quantized to Q5_K_M by handy-computer",
        card_url: "https://huggingface.co/Qwen/Qwen3-ASR-1.7B",
    },
    ModelInfo {
        id: "cohere-transcribe-03-2026-q5_k_m-gguf",
        name: "Cohere Transcribe (03-2026)",
        runtime: Runtime::TranscribeCpp,
        offering: Offering::BenchOnly,
        path: "gguf/cohere-transcribe-03-2026-Q5_K_M.gguf",
        download_url: "https://huggingface.co/handy-computer/cohere-transcribe-03-2026-gguf/resolve/ab667acedcb5d8d56837ef3ed3568c3ad50dde47/cohere-transcribe-03-2026-Q5_K_M.gguf",
        sha256: Some("14d02f1ad6dd77b3a60f82639879012c3adb4fe25c50a5a47a2c4c661daf1558"),
        download_bytes: 1_770_270_208,
        disk_bytes: 1_770_270_208,
        punctuation: true,
        pick_if: "",
        license: APACHE_2_0,
        creator: "Cohere",
        attribution: "Cohere Transcribe by Cohere, licensed under Apache 2.0",
        modification: "Converted to GGUF and quantized to Q5_K_M by handy-computer",
        card_url: "https://huggingface.co/CohereLabs/cohere-transcribe-03-2026",
    },
    ModelInfo {
        id: "canary-qwen-2.5b-q5_k_m-gguf",
        name: "Canary-Qwen 2.5B",
        runtime: Runtime::TranscribeCpp,
        offering: Offering::BenchOnly,
        path: "gguf/canary-qwen-2.5b-Q5_K_M.gguf",
        download_url: "https://huggingface.co/handy-computer/canary-qwen-2.5b-gguf/resolve/df576c1641eb59bb66bc3c396bbdd8b0b113825a/canary-qwen-2.5b-Q5_K_M.gguf",
        sha256: Some("d71b1d9ae442dc538665ed4df63a69520eb9be769ec0bc78c7d2f40040928306"),
        download_bytes: 1_983_729_024,
        disk_bytes: 1_983_729_024,
        punctuation: true,
        pick_if: "",
        license: CC_BY_4_0,
        creator: "NVIDIA",
        attribution: "Canary-Qwen 2.5B by NVIDIA, licensed under CC BY 4.0",
        modification: "Converted to GGUF and quantized to Q5_K_M by handy-computer",
        card_url: "https://huggingface.co/nvidia/canary-qwen-2.5b",
    },
    ModelInfo {
        id: "granite-speech-4.1-2b-q5_k_m-gguf",
        name: "Granite Speech 4.1 2B",
        runtime: Runtime::TranscribeCpp,
        offering: Offering::BenchOnly,
        path: "gguf/granite-speech-4.1-2b-Q5_K_M.gguf",
        download_url: "https://huggingface.co/handy-computer/granite-speech-4.1-2b-gguf/resolve/5fa89dfac74d52a186f63647553b9e1f7ae36e83/granite-speech-4.1-2b-Q5_K_M.gguf",
        sha256: Some("1ea15c2174b485b8de81337a1da3ae6703a7892a5330345b9a2212e9a6a0c365"),
        download_bytes: 1_829_704_640,
        disk_bytes: 1_829_704_640,
        punctuation: true,
        pick_if: "",
        license: APACHE_2_0,
        creator: "IBM",
        attribution: "Granite Speech 4.1 2B by IBM, licensed under Apache 2.0",
        modification: "Converted to GGUF and quantized to Q5_K_M by handy-computer",
        card_url: "https://huggingface.co/ibm-granite/granite-speech-4.1-2b",
    },
    ModelInfo {
        id: "granite-speech-5.0-470m-turboctc-q5_k_m-gguf",
        name: "Granite Speech 5.0 470M TurboCTC",
        runtime: Runtime::TranscribeCpp,
        offering: Offering::BenchOnly,
        path: "gguf/granite-speech-5.0-470m-turboctc-Q5_K_M.gguf",
        download_url: "https://huggingface.co/handy-computer/granite-speech-5.0-470m-turboctc-gguf/resolve/2b0b44d0d6c8b94a2e4f04a2cf3ec6a5c054463b/granite-speech-5.0-470m-turboctc-Q5_K_M.gguf",
        sha256: Some("a18c6882ff9dbcefbe674ccb8f8653578ed7dbc3fa9a955f036f3617b9627cce"),
        download_bytes: 335_737_184,
        disk_bytes: 335_737_184,
        punctuation: false,
        pick_if: "",
        license: APACHE_2_0,
        creator: "IBM",
        attribution: "Granite Speech 5.0 470M TurboCTC by IBM, licensed under Apache 2.0",
        modification: "Converted to GGUF and quantized to Q5_K_M by handy-computer",
        card_url: "https://huggingface.co/ibm-granite/granite-speech-5.0-470m-turboctc",
    },
    ModelInfo {
        id: "nemotron-speech-streaming-en-0.6b-q5_k_m-gguf",
        name: "Nemotron Speech Streaming EN 0.6B",
        runtime: Runtime::TranscribeCpp,
        offering: Offering::BenchOnly,
        path: "gguf/nemotron-speech-streaming-en-0.6b-Q5_K_M.gguf",
        download_url: "https://huggingface.co/handy-computer/nemotron-speech-streaming-en-0.6b-gguf/resolve/9789e0ebf77277911272f0d9a35e1646b5aa6004/nemotron-speech-streaming-en-0.6b-Q5_K_M.gguf",
        sha256: Some("d6b8b36f9aca6a751779d064a29f6583f3e82c0c15646904383c8436a25c1489"),
        download_bytes: 538_989_568,
        disk_bytes: 538_989_568,
        punctuation: true,
        pick_if: "",
        license: NVIDIA_OPEN_MODEL_LICENSE,
        creator: "NVIDIA",
        attribution: NVIDIA_OPEN_MODEL_NOTICE,
        modification: "Converted to GGUF and quantized to Q5_K_M by handy-computer",
        card_url: "https://huggingface.co/nvidia/nemotron-speech-streaming-en-0.6b",
    },
    // Q8_0 is the only quantization published.
    ModelInfo {
        id: "moonshine-streaming-medium-q8_0-gguf",
        name: "Moonshine Streaming Medium",
        runtime: Runtime::TranscribeCpp,
        offering: Offering::BenchOnly,
        path: "gguf/moonshine-streaming-medium-Q8_0.gguf",
        download_url: "https://huggingface.co/handy-computer/moonshine-streaming-medium-gguf/resolve/0f99e956a9e63d591ddd7f2a20dfead255e96c68/moonshine-streaming-medium-Q8_0.gguf",
        sha256: Some("f7c9564249b508f6012927ec4f9e536087da53a7047f858ca9975bea5f75299e"),
        download_bytes: 295_793_568,
        disk_bytes: 295_793_568,
        punctuation: true,
        pick_if: "",
        license: MIT,
        creator: "Moonshine AI",
        attribution: "Moonshine Streaming Medium by Moonshine AI, licensed under MIT",
        modification: "Converted to GGUF and quantized to Q8_0 by handy-computer",
        card_url: "https://huggingface.co/moonshine-ai/moonshine-streaming-medium",
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
        pick_if: "",
        license: CC_BY_4_0,
        creator: "NVIDIA",
        attribution: "Parakeet TDT 0.6B v2 by NVIDIA, licensed under CC BY 4.0",
        modification: "Converted to ONNX and quantized to int8 by k2-fsa/sherpa-onnx",
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
        pick_if: "",
        license: NVIDIA_OPEN_MODEL_LICENSE,
        creator: "NVIDIA",
        attribution: NVIDIA_OPEN_MODEL_NOTICE,
        modification: "Converted to ONNX and quantized to int8 by k2-fsa/sherpa-onnx",
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
        // Parakeet TDT v2, and Whisper base.en as the small option (DESIGN.md, "Models").
        let ids: Vec<_> = offered.iter().map(|m| m.id).collect();
        assert_eq!(ids, [crate::RECOMMENDED_MODEL, "whisper-base.en-q8_0-gguf"]);
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
            assert!(!m.creator.is_empty() && !m.modified_by().is_empty(), "{}", m.id);
            if m.license == NVIDIA_OPEN_MODEL_LICENSE {
                assert_eq!(m.attribution, NVIDIA_OPEN_MODEL_NOTICE, "{}", m.id);
            }
        }
    }

    /// What the app shows next to each model it offers, in Settings and in the notice it writes
    /// next to a download: the license (name and link), the credit, and the note that the file
    /// was converted and by whom (CC BY 4.0 §3(a)(1)(B), Apache-2.0 §4(b)).
    #[test]
    fn offered_models_carry_their_credits() {
        for m in offered() {
            assert!(!m.license.name.is_empty(), "{}: license name", m.id);
            assert!(m.license.url.starts_with("https://"), "{}: license URL", m.id);
            assert!(!m.attribution.is_empty(), "{}: attribution", m.id);
            assert!(!m.modification.is_empty(), "{}: modification note", m.id);
            // Settings links these words, so they have to be there.
            assert!(m.attribution.contains(m.license.name), "{}: the credit names the license", m.id);
            assert!(m.attribution.contains(m.creator), "{}: the credit names the creator", m.id);
            assert_eq!(m.modified_by(), "handy-computer", "{}", m.id);
            assert!(m.source_url().starts_with("https://huggingface.co/handy-computer/"), "{}", m.source_url());
            assert!(!m.pick_if.is_empty() && m.pick_if.ends_with('.'), "{}: pick this if…", m.id);
        }
    }

    /// The NVIDIA Open Model License binds whoever runs the model, so offering one would need an
    /// "I agree" step the app doesn't have. That's why Parakeet Unified is bench only.
    #[test]
    fn no_offered_model_needs_an_agreement() {
        for m in offered() {
            assert_ne!(m.license, NVIDIA_OPEN_MODEL_LICENSE, "{}", m.id);
        }
        assert!(!find("parakeet-unified-en-0.6b-q5_k_m-gguf").unwrap().offering.in_app());
    }

    #[test]
    fn the_license_notice_has_the_credits_and_the_disclaimer() {
        let m = find(crate::RECOMMENDED_MODEL).unwrap();
        let notice = m.license_notice();
        assert!(notice.starts_with("Parakeet TDT 0.6B v2\n====================\n\n"), "{notice}");
        for line in [
            "Parakeet TDT 0.6B v2 by NVIDIA, licensed under CC BY 4.0.\n",
            "License: https://creativecommons.org/licenses/by/4.0/\n",
            "Original model: https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2\n",
            "Converted to GGUF and quantized to Q5_K_M by handy-computer:\n\
             https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v2-gguf\n",
            "Provided as is, without warranties of any kind.\n",
            "isn't affiliated\nwith or endorsed by NVIDIA.\n",
        ] {
            assert!(notice.contains(line), "missing {line:?} in:\n{notice}");
        }
        let dir = Path::new("/m");
        assert_eq!(m.license_notice_path(dir), dir.join("gguf/parakeet-tdt-0.6b-v2-Q5_K_M.gguf.LICENSE.txt"));
        assert_eq!(m.short_name(), "Parakeet TDT 0.6B v2");
    }
}
