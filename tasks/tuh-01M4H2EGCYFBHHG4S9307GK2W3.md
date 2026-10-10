# Show each model's license and attribution in the app (NVIDIA notice for Parakeet Unified)

`tuh-01M4H2EGCYFBHHG4S9307GK2W3`

- **Status:** done
- **Priority:** 2
- **Labels:** `models` `app`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: DESIGN.md "Licenses" and "Next steps" 3. The catalog has a license (name and URL), an attribution line and the model card for every model. Parakeet Unified's NVIDIA Open Model License requires the app to show "Licensed by NVIDIA Corporation under the NVIDIA Open Model License" with a link to the agreement.

The ask: show the license, attribution and model-card link next to each model in Settings, links opening in the browser, plus an About or Acknowledgements spot that carries the active model's notice.

Acceptance: the exact NVIDIA sentence with a working link appears for Parakeet Unified; CC-BY-4.0 models credit their creator (NVIDIA; Moondream for Parakeet Ultra later) with a license link; a test asserts every Default and Alternative catalog entry has a non-empty license name, license URL and attribution.

Pointers: crates/easyvox-core/src/models.rs; crates/easyvox-app/src/settings.rs.


Decided by Brandon 2026-10-10: Parakeet Unified leaves the app (bench-only; see 'Drop Parakeet Unified from the app'), so no NVIDIA agreement line or notice is needed in the app. No upstream issue to handy-computer about its Unified license tag. Defaults taken (orchestrator, per the plan's recommendations): an About/Acknowledgements view lists every offered model; write a LICENSE.txt next to each downloaded model; show Whisper base.en as Apache-2.0 (its Hugging Face card and GGUF metadata). v2 needs the modification note (converted to GGUF and quantized to Q5_K_M by handy-computer, with the repo link) per CC BY 4.0 s3(a)(1)(B). Bundled code: generate third-party notices with cargo-about into the .app (covers ggml, miniz, Lucide icons; option-ext is MPL-2.0: list with a source link).

---
LICENSE PLAN (2026-10-10, read-only research agent; not legal advice; Brandon's answers get recorded above this line)

# EasyVox: model and third-party license plan

Research for ledger task `tuh-01M4H2EGCYFBHHG4S9307GK2W3`, "Show each model's license and attribution in the app".
Checked on 2026-10-10 against the live model cards, the pinned GGUF repos, the GGUF files' own metadata, and the
crate graph in `Cargo.lock`. I'm not a lawyer: this is a practical reading of the license texts, not legal advice.

## Summary

- **Nothing blocks shipping.** Every license involved allows a free MIT app to use these models and libraries.
- **The models are never in the DMG**, so EasyVox doesn't redistribute them; `handy-computer` does, from Hugging Face.
  The formal "when you redistribute" duties (a copy of the license, NVIDIA's Notice file) fall on that host, not on
  EasyVox. But the host's repos don't meet them: no Notice file, no copy of the agreement, and the Unified repo is tagged
  with the wrong license. So the app should carry the credits itself. The cost is small.
- **What `models.rs` lacks: the modification note.** Every offered file is a GGUF conversion and quantization made by
  `handy-computer`. CC BY 4.0 §3(a)(1)(B) asks you to "retain an indication of any previous modifications", and Apache-2.0
  §4(b) asks for a note that the files were changed. The catalog has the creator, the license and the model card, but
  nothing about the conversion or where the file comes from.
- **Bundled code:** the app links about 420 crates plus vendored C/C++ (transcribe.cpp, ggml, miniz) and Lucide icons.
  There's no GPL, LGPL or AGPL in the app's graph. One MPL-2.0 crate (`option-ext`) is fine for an MIT app: name it and say
  where its source is. The app must ship a third-party notices file **inside the .app**, because the Homebrew cask keeps
  only the .app. Generate it with `cargo-about`.

## 1. Models

### 1.1 Verified licenses

| Model (file) | Upstream license (authoritative) | GGUF repo card tag | License in the GGUF's own metadata | Copyright notice supplied upstream |
|---|---|---|---|---|
| Parakeet TDT 0.6B v2, Q5_K_M (default) | **CC BY 4.0** ([card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2): "GOVERNING TERMS: Use of this model is governed by the CC-BY-4.0 license") | cc-by-4.0 ([repo](https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v2-gguf)) | `cc-by-4.0`, author `NVIDIA` | None |
| Parakeet Unified EN 0.6B, Q5_K_M | **NVIDIA Open Model License** ([card](https://huggingface.co/nvidia/parakeet-unified-en-0.6b), `license_name: nvidia-open-model-license`) | **cc-by-4.0: wrong.** Its own card says "Inherited from the base model: CC-BY-4.0", yet it quotes NVIDIA's governing-terms line naming the Open Model License ([repo](https://huggingface.co/handy-computer/parakeet-unified-en-0.6b-gguf)) | `other` / `nvidia-open-model-license` (correct) | None |
| Whisper base.en, Q8_0 | **Apache-2.0** on OpenAI's own Hugging Face card ([card](https://huggingface.co/openai/whisper-base.en)). OpenAI's GitHub says "Whisper's code and model weights are released under the MIT License" ([repo](https://github.com/openai/whisper#license), `Copyright (c) 2022 OpenAI`) | apache-2.0 ([repo](https://huggingface.co/handy-computer/whisper-base.en-gguf)) | `apache-2.0`, author `OpenAI` | Only on GitHub (MIT) |

- The pinned commits in `models.rs` are still `main` on all three GGUF repos (382796e…, d524970…, f83d8d1…).
- Each GGUF repo holds only `README.md`, `.gitattributes` and the `.gguf` files: no LICENSE, NOTICE or copy of the
  agreement.
- The GGUF metadata says `base_model_relation: quantized`, which makes each file a modified version (CC "Adapted
  Material", NVIDIA "Derivative Model", Apache "Derivative Works").

### 1.2 What each license asks for

**CC BY 4.0** (Parakeet TDT v2; [legal code](https://creativecommons.org/licenses/by/4.0/legalcode.en)). §3(a) applies "if You
Share the Licensed Material (including in modified form)":
- **Creator:** retain who made it ("NVIDIA").
- **Copyright notice:** retain it if one is supplied. None is.
- **License:** give a notice that refers to the license, plus its text or a URI.
- **Disclaimer:** give a notice that refers to the disclaimer of warranties. Strictly, this applies only if one is
  supplied. It's one sentence, so add it anyway.
- **Link:** give a URI to the licensed material "to the extent reasonably practicable". That's the model card.
- **Modifications:** indicate any you made, and **retain an indication of any previous modifications**. Here, that's
  `handy-computer`'s GGUF conversion and quantization.
- **Medium:** §3(a)(2) says "any reasonable manner based on the medium". A credit line with links is fine.
- **No endorsement (§2(a)(6)):** don't imply NVIDIA endorses EasyVox.
- **No added restrictions (§2(a)(5)(B)):** EasyVox's own terms must not restrict the model further.
- **Use restrictions:** none.

**NVIDIA Open Model License** (Parakeet Unified; [agreement](https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/),
"Last Modified: October 24, 2025"):
- **On redistribution (§3a):** "If you distribute the Model, You must give any other recipients of the Model a copy of this
  Agreement and include the following attribution notice within a 'Notice' text file with such copies: 'Licensed by
  NVIDIA Corporation under the NVIDIA Open Model License'".
  - `NVIDIA_OPEN_MODEL_NOTICE` in `models.rs` matches the sentence word for word.
  - Note that the license asks for a *text file*. Showing the line in the UI is a sensible extra, not the literal duty.
- **No modification notice** is required. (§3c lets a modifier add their own terms for their changes; `handy-computer`
  added none.)
- **These bind every user,** whether or not they redistribute. That includes Brandon and each EasyVox user:
  - **Litigation:** suing anyone over the model ends your license.
  - **"Guardrail" clause:** circumventing a safety guardrail ends your license. Speech-to-text has none, and quantizing
    isn't circumventing one.
  - **Trustworthy AI terms** ([terms](https://www.nvidia.com/en-us/agreements/trustworthy-ai/terms/), June 27, 2024): no use in
    violation of law, "including … illegal surveillance, illegal collection or processing of biometric information without
    the consent of the subject … illegal harassment …". The rest only "encourages" developers to inform users of the
    product's nature and limitations.
  - **Indemnity:** you indemnify NVIDIA against claims from your use or distribution.
  - **Export compliance.**
  - **Updates:** NVIDIA may update the agreement, and you comply with the new version or stop. Link the live URL.
- **Trademarks (§5):** NVIDIA's name may be used only to describe the origin and to reproduce the Notice.
- **Cosmos clause:** n/a. Neither card marks the model as "Special-Purpose".

**Apache-2.0** (Whisper base.en; [license](https://www.apache.org/licenses/LICENSE-2.0)). §4 applies on redistribution:
- **(a)** give a copy of the license.
- **(b)** modified files carry a notice that they were changed.
- **(c)** keep any copyright notices. None are on the Hugging Face release.
- **(d)** include the NOTICE file, if there is one. There's none.
- **Use restrictions:** none. If you follow the GitHub MIT release instead, add "Copyright (c) 2022 OpenAI" and the MIT
  permission notice.

### 1.3 Does "downloaded at runtime, not bundled" change anything?

- **Yes for the redistribution duties.** EasyVox never hosts or ships the weights: the user's Mac fetches them from
  `huggingface.co/handy-computer/...`. The copy of the license, NVIDIA's Notice file, Apache §4 and CC's §3(a) all attach
  to whoever "distributes" or "Shares" the material. CC's definition of "Share" covers making material available, not
  pointing at another host's copy.
  - **Uncertain:** EasyVox picks, pins and triggers the download. A strict reader could call that "making available".
    Covering that reading costs a few strings and one text file, so the plan below does.
- **No for the use terms.** Whoever runs Parakeet Unified is a licensee under NVIDIA's "By using … you agree to be
  bound". That's each user, and Brandon.
- **No for the bundled code.** The DMG really does redistribute the Rust crates and C/C++ code (section 2).
- **The source host doesn't carry the notices.** It ships no Notice file and no agreement copy, and mislabels Unified. So
  the only place a user sees the correct terms is EasyVox's UI and the files EasyVox writes. That's the practical reason
  to show them, beyond the strict letter.

### 1.4 Strings to show, and where

Write links as `[text](url)`. Show the strings exactly; the NVIDIA sentence has no trailing period in NVIDIA's quote.

Shared URLs:
- CC BY 4.0: `https://creativecommons.org/licenses/by/4.0/`. Already `CC_BY_4_0.url`; the deed links to the legal code.
- NVIDIA OML: `https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/`
- NVIDIA Trustworthy AI terms: `https://www.nvidia.com/en-us/agreements/trustworthy-ai/terms/`
- Apache-2.0: `https://www.apache.org/licenses/LICENSE-2.0`

**A. Settings, model picker.** One block per offered model: under the name and size, two small lines plus links. Show it
on every listed row, downloaded or not, and whenever a download button is offered.

| Model | Line 1 (credit + license) | Line 2 (modification + links) |
|---|---|---|
| Parakeet TDT 0.6B v2 | `Parakeet TDT 0.6B v2 by NVIDIA, licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).` | `Converted to GGUF and quantized to Q5_K_M by [handy-computer](https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v2-gguf). [Model card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2)` |
| Parakeet Unified EN 0.6B | `Licensed by NVIDIA Corporation under the [NVIDIA Open Model License](https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/)` | `Converted to GGUF and quantized to Q5_K_M by [handy-computer](https://huggingface.co/handy-computer/parakeet-unified-en-0.6b-gguf). [Model card](https://huggingface.co/nvidia/parakeet-unified-en-0.6b)` |
| Whisper base.en | `Whisper base.en by OpenAI, licensed under [Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0).` | `Converted to GGUF and quantized to Q8_0 by [handy-computer](https://huggingface.co/handy-computer/whisper-base.en-gguf). [Model card](https://huggingface.co/openai/whisper-base.en)` |

Next to Parakeet Unified's download button, before the download starts (recommended; see Questions):
`By downloading, you agree to the [NVIDIA Open Model License](https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/) and NVIDIA's [Trustworthy AI terms](https://www.nvidia.com/en-us/agreements/trustworthy-ai/terms/).`

Note: "CC BY 4.0" is CC's own spelling for display. Keep the SPDX id `CC-BY-4.0` in `License.name` only if something
machine-reads it; otherwise show "CC BY 4.0".

**B. Onboarding download screen.** It downloads only the default model. One line under the progress bar:
`Downloading Parakeet TDT 0.6B v2 by NVIDIA from Hugging Face. Licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/); converted to GGUF by handy-computer. [Model card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2)`

This is recommended rather than strictly required: Settings and About carry the full credit. If onboarding ever offers
Unified, use its Settings line plus the "By downloading…" line.

**C. About / Acknowledgements.** I suggest an "About" page in Settings, which is already GPUI and already opens links with
`cx.open_url`. List **all offered models**, not only the active one; see Questions. Content:

```
EasyVox <version>
© 2026 Brandon Bews. Released under the MIT License. [License]

Speech models
EasyVox downloads these models from Hugging Face when you choose them. They are not part of EasyVox and keep their
own licenses. EasyVox is not affiliated with or endorsed by NVIDIA or OpenAI.

Parakeet TDT 0.6B v2
  Parakeet TDT 0.6B v2 by NVIDIA, licensed under CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/).
  Original model: https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2
  Modified: converted to GGUF and quantized to Q5_K_M by handy-computer:
  https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v2-gguf
  Provided as is, without warranties of any kind.

Parakeet Unified EN 0.6B
  Licensed by NVIDIA Corporation under the NVIDIA Open Model License
  https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/
  Use is also subject to NVIDIA's Trustworthy AI terms: https://www.nvidia.com/en-us/agreements/trustworthy-ai/terms/
  Original model: https://huggingface.co/nvidia/parakeet-unified-en-0.6b
  Converted to GGUF and quantized to Q5_K_M by handy-computer:
  https://huggingface.co/handy-computer/parakeet-unified-en-0.6b-gguf

Whisper base.en
  Whisper base.en by OpenAI, licensed under the Apache License 2.0 (https://www.apache.org/licenses/LICENSE-2.0).
  Original model: https://huggingface.co/openai/whisper-base.en
  Modified: converted to GGUF and quantized to Q8_0 by handy-computer:
  https://huggingface.co/handy-computer/whisper-base.en-gguf

Open-source software
EasyVox is built on open-source software, including GPUI (Zed Industries), GPUI Kit (Longbridge), transcribe.cpp
and ggml, SQLite, Lucide icons and many Rust crates. [Third-party notices]   ← opens Contents/Resources/THIRD_PARTY_NOTICES.html
```

Optional: a tray "About EasyVox" item via `tray_icon::menu::PredefinedMenuItem::about(None, Some(AboutMetadata { credits: None, .. }))`.
That opens AppKit's standard About panel. When muda 0.21.2 passes no credits (checked:
`platform_impl/macos/mod.rs:934` sets `NSAboutPanelOptionCredits` only when `credits` is `Some`), AppKit loads
`Credits.rtf` (or `Credits.html`) from `Contents/Resources`. Muda's own `credits` field is plain text with no live links.

**D. A Notice file next to each downloaded model** (recommended, belt and braces). After a download passes its sha256,
write `<models_dir>/gguf/<file>.LICENSE.txt` with the block from C for that model. For Parakeet Unified, follow it with
the full agreement text, bundled in the app as `Contents/Resources/licenses/NVIDIA-Open-Model-License.txt` (~11 KB, text
as of the Oct 24, 2025 version). This meets the strictest reading of NVIDIA §3a ("a copy of this Agreement" plus a
"Notice" text file "with such copies"), which `handy-computer` doesn't. `is_available` checks only the model file's size,
so a sibling file doesn't affect it.

### 1.5 Does `models.rs` need more for v2's CC BY?

Mostly no. It has the creator ("by NVIDIA"), the license name and link, and the model card (the URI to the licensed
material). No copyright notice is supplied upstream, so none is missing. What it lacks:

1. **The modification note**, required by §3(a)(1)(B): "Converted to GGUF and quantized to Q5_K_M by handy-computer",
   plus the GGUF repo link.
2. **Display.** The license link and the card link must actually appear. Today they're only data; `settings.rs` doesn't
   show them yet.
3. Optional: the one-line warranty disclaimer, and "not endorsed by NVIDIA".

Whisper (Apache §4(b)) needs the same modification note. Unified doesn't strictly need it, but it's accurate and cheap.

Proposed catalog change (sketch):

```rust
pub struct ModelInfo {
    // ...existing fields...
    /// How the downloaded file differs from the upstream model, and who changed it. CC BY 4.0 §3(a)(1)(B)
    /// and Apache-2.0 §4(b) ask to keep this note.
    pub modification: &'static str, // "Converted to GGUF and quantized to Q5_K_M by handy-computer"
}
impl ModelInfo {
    /// The repo the file is downloaded from: `download_url` up to `/resolve/`.
    pub fn source_repo_url(&self) -> Option<&'static str> { /* split download_url at "/resolve/" */ }
}
```

### 1.6 Bench-only and later models (brief)

- **Not shipped, no in-app duty:** Canary 180M Flash (CC BY 4.0, NVIDIA), sherpa-onnx Parakeet v2 int8 (CC BY 4.0),
  Nemotron Speech Streaming (NVIDIA OML), Silero VAD (MIT), and the sherpa-onnx crate (Apache-2.0). Their catalog
  credits are already right.
- **Parakeet Ultra (later):** its card says CC BY 4.0 and "A post-trained version of parakeet-tdt-0.6b-v3", which is
  NVIDIA's, also CC BY 4.0. Credit **both** creators, not Moondream alone:
  `Parakeet Ultra by Moondream, based on Parakeet TDT 0.6B v3 by NVIDIA, licensed under CC BY 4.0.` Add the GGUF note.
  - The ledger task's acceptance line says only "Moondream for Parakeet Ultra"; update it when Ultra lands.

## 2. Bundled code (the DMG)

### 2.1 What ships

`cargo tree -p easyvox-app -e normal --target aarch64-apple-darwin` lists 456 crates. 33 are proc-macros, which run at
build time, so about 420 are linked in. License expressions:

| Count | License |
|---|---|
| 195 / 48 / 22 / 9 | MIT OR Apache-2.0 (various spellings) |
| 78 | MIT |
| 26 | Apache-2.0 only: all `gpui-pre-*` (Zed), `gpui-kit`, `gpui-base`, `gpui-component`, `gpui-kit-assets` (Longbridge), plus `cpal`, `hound`, `unicode-linebreak` |
| 26 + 2 | Zlib OR MIT OR Apache-2.0 (mostly `objc2-*`), plus `foldhash` and `slotmap` (Zlib only) |
| 18 | Unicode-3.0 (`icu_*`, `zerovec`, `yoke`, …) |
| 8 | Unlicense OR MIT |
| 6 | BSD-3-Clause: `exr`, `instant`, `lebe`, `sha1_smol`, `tiny-skia`, `tiny-skia-path` |
| 1 each | BSD-2-Clause (`arrayref`), 0BSD (`enum-iterator`), CC0-1.0 (`notify`), Apache-2.0 OR BSL-1.0 (`ryu`), bzip2-1.0.6 (`libbz2-rs-sys`), **MPL-2.0 (`option-ext`)** |

Not covered by a crate's `license` field, so the notices must name them explicitly:

- **`transcribe-cpp-sys` 0.3.1 vendored C/C++:**
  - transcribe.cpp: MIT, "The transcribe.cpp authors" (`LICENSE`).
  - **ggml:** MIT, "The ggml authors" (`ggml/LICENSE`). Inside it are MIT snippets credited in file headers: Mozilla
    Foundation (`ggml-cpu/llamafile/sgemm.cpp`) and Jeffrey Quesnelle & Bowen Peng (YaRN, in `ops.cpp` and `rope.metal`).
  - **miniz** 3.1.1: MIT, RAD Game Tools / Valve / Rich Geldreich (`src/third_party/miniz/LICENSE`).
  - The build cache shows `GGML_CPU_KLEIDIAI=OFF`, so no KleidiAI is fetched. BLAS is off; Accelerate is Apple's.
- **`gpui-kit-assets` 0.7.1** declares Apache-2.0 but embeds **Lucide icons (ISC)**. Some are Feather-derived (**MIT,
  Cole Bemis**). See `LICENSE-LUCIDE`, which has no Apache file of its own.
- **`libsqlite3-sys` (bundled SQLite 3.53.2):** public domain. Nothing is required; a credit line is nice.
- **Rust standard library:** statically linked, MIT OR Apache-2.0. Crediting it is customary, not required.
- **NOTICE files:** none of the app's crates ships an Apache `NOTICE` file, checked in each crate's root and one level
  down. So Apache §4(d) adds nothing today. Re-check on upgrades; cargo-about handles it.

### 2.2 What the notices file must contain

- **MIT, BSD-2/3, ISC, Zlib, bzip2-1.0.6, Unicode-3.0, 0BSD:** the copyright lines and license texts.
  - MIT and BSD say "in all copies"; BSD and Unicode say "in the documentation … provided with the distribution".
  - Zlib and 0BSD don't strictly require it in binaries; include them anyway.
- **Apache-2.0:** one copy of the license text, a list of the crates under it, and any NOTICE files (none today).
- **MPL-2.0 (`option-ext` 0.2.0):** the MPL text, plus where to get the source. MPL §3.2(a) asks you to "inform
  recipients of the Executable Form how they can obtain a copy of such Source Code Form". Its crates.io or repository
  URL does that, so the template should print each crate's repository URL.
- **The non-crate items in 2.1.**
- **EasyVox's own MIT `LICENSE`.**
- **Where it goes:** `EasyVox.app/Contents/Resources/`. The DMG "holds only the app" (DESIGN.md), and the Homebrew cask
  copies only the .app.

### 2.3 Tool: `cargo-about` (0.9.2, Aug 2026)

| Tool | Verdict |
|---|---|
| **cargo-about** 0.9.2 | **Use this.** A Handlebars template gives a readable HTML or text notices file. It has `targets` (drops Windows and Linux crates such as `dwrote` and `cosmic-text`), `ignore-build-dependencies`, `ignore-dev-dependencies`, `private = { ignore = true }` for the unpublished workspace crates, per-crate `clarify` for nested license files, `--fail` for unaccepted licenses, and `--locked --offline`. |
| cargo-bundle-licenses 4.2.0 | JSON/YAML/TOML output, not a readable notices file. Its CI check mode is nice, but it's the wrong output for an About screen. |
| cargo-deny 0.20.2 | Complementary. `cargo deny check licenses` in CI blocks a GPL crate from sneaking in. It doesn't write notices. |
| cargo-packager 0.11.8 (planned DMG tool) | Its `resources` setting copies the generated file into `Contents/Resources`. |

Starting `about.toml` (at the workspace root):

```toml
accepted = [
  "MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause", "ISC",
  "Zlib", "0BSD", "Unicode-3.0", "Unlicense", "CC0-1.0", "BSL-1.0", "MPL-2.0", "bzip2-1.0.6",
]
targets = ["aarch64-apple-darwin"]   # add x86_64-apple-darwin if Intel builds ship
ignore-build-dependencies = true
ignore-dev-dependencies = true
private = { ignore = true }

# The crate's `license` field says MIT only. Make sure ggml's and miniz's own licenses are listed too.
[transcribe-cpp-sys.clarify]
license = "MIT"
[[transcribe-cpp-sys.clarify.files]]
path = "LICENSE"
checksum = "86a53633b56f6b029d3cb42158bcc7aac0cdff898aceb13e83b93e368bbc4ac6"
[[transcribe-cpp-sys.clarify.files]]
path = "ggml/LICENSE"
checksum = "94f29bbed6a22c35b992c5c6ebf0e7c92f13b836b90f36f461c9cf2f0f1d010d"
[[transcribe-cpp-sys.clarify.files]]
path = "src/third_party/miniz/LICENSE"
checksum = "6f20fa7672b00e2e975c291df737cf227addf3ad32e36fef3fe0f416e4664d3d"

# Declared Apache-2.0, but the icons it embeds are ISC (Lucide) and MIT (Feather).
[gpui-kit-assets.clarify]
license = "Apache-2.0 AND ISC AND MIT"
[[gpui-kit-assets.clarify.files]]
path = "LICENSE-LUCIDE"
license = "ISC AND MIT"
checksum = "b495047bd93a9b06913511076f504daba17d5bbeb3e0650f3bb53a4220329c57"
```

Run it as:

```
cargo about generate --locked --offline -m crates/easyvox-app/Cargo.toml -o packaging/THIRD_PARTY_NOTICES.html packaging/about.hbs
```

Start the template from `cargo about init`, add `{{crate.repository}}` per crate, and add a hand-written section for
SQLite, the Rust std, and the YaRN and Mozilla snippets.

- **Uncertain:** whether the `clarify` syntax is exactly right for multi-file entries, and whether cargo-about would
  pick up `ggml/LICENSE` on its own. Check the generated file for "The ggml authors", "Rich Geldreich" and "Lucide",
  and adjust.

### 2.4 Problem licenses

- **In the app binary:** no GPL, LGPL, AGPL, SSPL or EPL.
- **`option-ext` 0.2.0 (MPL-2.0):** pulled in by `zed-font-kit` → `dirs` → `dirs-sys`. Fine for an MIT app: it's
  unmodified and file-level copyleft. Duty: list it, with its license text and source URL.
- **Elsewhere in `Cargo.lock`, none in the app binary:**
  - `cbindgen` (MPL-2.0): a build tool for `gpui-pre-apple`. Its output isn't covered.
  - `dwrote` (MPL-2.0): Windows only.
  - `self_cell` (Apache-2.0 OR GPL-2.0-only): via `cosmic-text`, Linux only. Take Apache.
  - `r-efi` (MIT OR Apache-2.0 OR LGPL-2.1+): UEFI only.
- **GPUI:** the `gpui-pre-*` snapshot crates match upstream Zed at the pinned commit `279fe07`. `crates/gpui`, `util`,
  `zlog`, `ztracing`, `scheduler`, `http_client`, `collections`, `sum_tree` and `refineable` are all Apache-2.0 there,
  so the repackaging didn't relabel anything GPL. (Zed the editor is GPL; GPUI isn't.)

## 3. Implementation checklist (for a coding agent)

1. **`models.rs`:**
   - Add `modification: &'static str` to `ModelInfo` and fill it for every entry (Q5_K_M or Q8_0, "by handy-computer";
     the sherpa entries say "converted to ONNX int8 by k2-fsa/sherpa-onnx").
   - Add `source_repo_url()`, derived from `download_url`.
   - Optionally add `CC_BY_4_0` display text "CC BY 4.0".
   - Keep `NVIDIA_OPEN_MODEL_NOTICE` exactly as it is.
2. **Tests in `models.rs`:** extend `every_model_names_its_license`. For every **offered** entry, assert non-empty
   `license.name`, `license.url` (https), `attribution`, `card_url`, `modification` and `source_repo_url()`, and that
   NVIDIA-licensed entries use the exact notice. That covers the ledger task's acceptance test.
3. **`settings.rs`:**
   - Under the Model field, show the two lines from §1.4 A for the selected model. Show them per row in any model list
     or download UI.
   - Each link calls `cx.open_url(url)`.
   - Show the Unified "By downloading…" line if the user agrees (Question 1).
4. **Settings "About" page:**
   - Version, copyright and MIT; all offered models' blocks (§1.4 C).
   - A "Third-party notices" button that opens the bundled HTML from `Contents/Resources` in the browser.
   - Optional: a tray "About EasyVox" item with the standard panel and `Credits.rtf`.
5. **Onboarding** (when built): the one line from §1.4 B.
6. **After a verified download** (`easyvox_core::download`, or the controller), write `<model>.LICENSE.txt` (§1.4 D).
   For NVIDIA-licensed models, append the bundled agreement text. Put the text in `easyvox-core` so the
   `fetch-model.sh` users get it too, or leave the script alone.
7. **Notices:** add `about.toml` and `packaging/about.hbs` (§2.3), plus `scripts/notices.sh`.
   - Make `scripts/bundle.sh` (and cargo-packager later) copy `THIRD_PARTY_NOTICES.html`, EasyVox's `LICENSE` and
     `licenses/NVIDIA-Open-Model-License.txt` into `Contents/Resources`.
   - Commit the generated file, or generate it in CI with `--fail`.
   - Check that it lists ggml, miniz and Lucide.
8. **CI (optional):** a `deny.toml` with the same `accepted` list, and `cargo deny check licenses`.
9. **Info.plist:** add `NSHumanReadableCopyright` = `© 2026 Brandon Bews. MIT License.`
10. **DESIGN.md "Licenses":**
    - Add the modification note and the runtime-download reasoning.
    - Note Whisper's dual statement (Apache on Hugging Face, MIT on GitHub).
    - Correct "Moondream for Ultra" to "Moondream, based on NVIDIA's v3".

## 4. Uncertain

- **Whether EasyVox "distributes" or "Shares" the models** by pinning and triggering downloads from a third-party host.
  The plan covers the strict reading anyway, with the notices in the UI and the Notice file.
- **Whisper's license:** OpenAI states Apache-2.0 on Hugging Face and MIT on GitHub, for the same weights. Both are
  permissive. Following the Hugging Face release, which is the GGUF's `base_model` and what its metadata says, is
  consistent. Mentioning MIT as well costs one line.
- **Whether a Notice in the UI satisfies NVIDIA's "Notice text file".** Hence item 6, the sibling text file.
- **The cargo-about `clarify` syntax and auto-detection** for nested files. Verify against the generated output.
- **Whether AppKit's standard About panel loads `Credits.html` as well as `Credits.rtf`.** `Credits.rtf` is the safe choice.
- **Weights' copyrightability** varies by jurisdiction. Treat the licenses as binding either way.

## 5. Questions for Brandon (with recommendations)

1. **Show "By downloading, you agree to the NVIDIA Open Model License…" before the Parakeet Unified download?**
   Recommended: **yes.** The user becomes the licensee, and the source repo doesn't tell them the right license.
2. **Should About list all offered models, or only the active one** (as the ledger says)? Recommended: **all offered
   models.** It's static, simpler, and covers a model that's downloaded but not active.
3. **Write a `<model>.LICENSE.txt` next to downloaded models, and bundle the NVIDIA agreement text?** Recommended: **yes.**
   It's about 20 lines of code and closes the strictest reading of NVIDIA §3a.
4. **Ask `handy-computer` to fix the Parakeet Unified GGUF card** (tagged CC-BY-4.0; should be the NVIDIA OML) and add a
   Notice file? Recommended: **yes, open an issue.** This is external communication, so it's your call.
5. **Whisper: keep Apache-2.0** (the Hugging Face card, and the GGUF's metadata) **or show MIT?** Recommended: **keep
   Apache-2.0.** Add "OpenAI also releases the weights under the MIT License" in About only if you want both covered.

## Sources

- Model cards:
  - [nvidia/parakeet-tdt-0.6b-v2](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2) (commit ae9ad07)
  - [nvidia/parakeet-unified-en-0.6b](https://huggingface.co/nvidia/parakeet-unified-en-0.6b) (fe53cd8)
  - [openai/whisper-base.en](https://huggingface.co/openai/whisper-base.en) (911407f)
- GGUF repos:
  - [handy-computer/parakeet-tdt-0.6b-v2-gguf](https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v2-gguf)
  - [handy-computer/parakeet-unified-en-0.6b-gguf](https://huggingface.co/handy-computer/parakeet-unified-en-0.6b-gguf)
  - [handy-computer/whisper-base.en-gguf](https://huggingface.co/handy-computer/whisper-base.en-gguf)
- GGUF `general.*` metadata, read from the local copies' headers.
- Licenses:
  - [NVIDIA Open Model License](https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/) (Oct 24, 2025)
  - [NVIDIA Trustworthy AI terms](https://www.nvidia.com/en-us/agreements/trustworthy-ai/terms/) (June 27, 2024)
  - [CC BY 4.0 legal code](https://creativecommons.org/licenses/by/4.0/legalcode.en)
  - [Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0)
  - [openai/whisper LICENSE and README](https://github.com/openai/whisper#license)
  - [moondream/parakeet-ultra](https://huggingface.co/moondream/parakeet-ultra)
- Tools:
  - [cargo-about docs](https://embarkstudios.github.io/cargo-about/) and [config](https://embarkstudios.github.io/cargo-about/cli/generate/config.html)
  - crates.io API (versions of cargo-about, cargo-bundle-licenses, cargo-deny, cargo-packager)
- Local:
  - `cargo tree` / `cargo metadata --offline --locked`
  - `~/.cargo/registry/src/...` for `transcribe-cpp-sys-0.3.1`, `gpui-kit-assets-0.7.1`, `gpui-pre-0.3.8` and `muda-0.21.2`
  - Zed `crates/*/Cargo.toml` at commit `279fe07`

Done in 695c6d1: license, creator, model card and GGUF modification note per model; Whisper as Apache 2.0; <file>.LICENSE.txt written next to each downloaded model; About page (Settings and tray) with credits, THIRD_PARTY_NOTICES.html and LICENSE; scripts/notices.sh (cargo-about 0.9, --features cli) generates packaging/THIRD_PARTY_NOTICES.html; tests/notices.rs catches Cargo.lock drift without CI installing cargo-about. Models already on disk before this (downloaded by fetch-model.sh) have no LICENSE.txt.

## History

### 2026-10-10 12:20 UTC — edit by `brandon`

description edited

### 2026-10-10 14:56 UTC — edit by `brandon`

description edited

### 2026-10-10 17:51 UTC — edit by `brandon`

description edited · status open→done
