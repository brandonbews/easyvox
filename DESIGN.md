# EasyVox design notes

Working notes. Decisions are recorded as they're made; what's still open is listed at the end.

## Requirements

- All processing happens on the Mac. No audio or text leaves the machine. The only network use is downloading models.
- English only for now. French is a stretch goal (see below).
- Lightweight and fast: low memory use when idle, and text pasted quickly after you release the key.
- Push-to-talk on a shortcut the user picks: hold to record, release to transcribe.
- Long dictations are normal use: holding the key for several minutes and rambling. The live preview has to keep up the whole time, and the final text has to arrive quickly with nothing dropped.
- Show text as it's recognized, in chunks, so you don't wait until the end.
- On release, paste the transcript at the cursor. The transcript stays on the clipboard until something else replaces it.
- History of past transcripts, each with a quick copy button.
- Subtle, quick animation for polish: a live indicator that reacts when it hears you, a smooth open and close, and new words easing in.
- Light and dark mode, following the macOS appearance setting and switching live when it changes. This covers the overlay, Settings and History; the menu bar icon is a template image, so macOS tints it automatically.
- Local storage for history, settings and downloaded models.
- A tight model list with one opinionated default. Models are easy to swap, since we'll add options over time.
- Use both "two model" approaches (see below) without giving up speed or lightness.
- macOS first. Linux possibly later.

## Core flow

1. Hold the shortcut. Recording starts.
2. A small floating window shows the text as it comes in.
3. Release. EasyVox finishes the transcript, puts it on the clipboard and pastes it at the cursor.
4. The transcript is saved to history.

## Keeping it light and fast

- The speech model drives speed and memory far more than the UI framework does. In the spike, the GPUI Kit shell idled at about 22 MB with no model loaded and about 636 MB with the default model loaded.
- Keep the speech model loaded so recording starts instantly, and unload it after a stretch of no use. Loading on key-down would add a noticeable delay.
- Use one speech model for both the live preview and the final text, so two models never sit in memory at once (see "two model" approach B).
- LLM cleanup is optional and off by default. The LLM loads only when cleanup is turned on.
- Set a latency target once we can measure, for example: a short sentence pasted within half a second of release, without cleanup. In the spike, the final text for 3–30 s of speech was ready 0.09–0.49 s after release (from recorded clips, not a real mic).

## Feature notes

### Push-to-talk shortcut

- Needs both key-down and key-up events.
- Regular combos (like ⌥Space) are straightforward. They go through the `global-hotkey` crate and need no permission. ⌥Space works on macOS 26, despite the Option-only restriction added in macOS 15.
- Holding a modifier on its own (right ⌥, Fn) needs a lower-level event tap and the Input Monitoring or Accessibility permission. macOS uses Fn/Globe for its own features by default. Creating the tap succeeds even without the permission, so check it with `CGPreflightListenEventAccess()`.
- The shortcut recorder in Settings must pause the live shortcut while it records, or the OS swallows the combo.
- Later: a toggle mode (tap to start, tap to stop) for long dictation.

### Live text while speaking

- Most speech models aren't built for streaming. EasyVox handles both kinds:
  - **Models that stream natively** (the default, Parakeet Unified) send a partial per decoded chunk. With 560 ms chunks the first words appear about 1.35 s after you start speaking. Smaller chunks show them sooner, at more CPU and GPU cost.
  - **Other models** re-run on the recent audio every 400 ms or so. Text shows in two states: confirmed (stable) and provisional (may still change). Text is confirmed once two passes in a row agree.
- Either way, on release one offline pass over the whole recording produces the final text, which replaces the preview.
- Live text goes in EasyVox's own floating window, not the target app. Typing provisional text into another app and then correcting it is fragile. The final text is pasted once, on release.

### Overlay animation

Subtle and quick. It should feel responsive, never slow you down, and never draw attention away from what you're typing into.

- **Hearing input.** A small level indicator (a few bars or a waveform) moves with your voice and rests flat in silence. It's the first sign the mic is working. If no audio arrives at all, show a clear "can't hear you" state instead of a flat line.
  - Needs the core to send audio levels about 30 times a second, smoothed, e.g. a new `TranscriptEvent::Level(f32)`. The current contract only sends text.
- **Open and close.** A fast fade plus a slight scale or slide, roughly 150 ms in and 120 ms out. The pill opens on key press without waiting for the model, and closes right after the paste.
- **New words and chunks.** New words fade in over about 100–150 ms. Provisional text eases to full strength when it's confirmed. Avoid layout jumps: the pill's width grows smoothly, and long text scrolls smoothly to keep the newest words in view.
- **Reduce motion.** Respect the macOS "Reduce motion" setting: swap the movement for simple fades.
- **Cost.** Animate only while the overlay is visible, with nothing running at idle. The level indicator redraws far more often than the text updates, so measure its CPU with the screen unlocked; the spike only measured text updates, with no on-screen drawing.

### Paste at cursor

- Standard approach: put the text on the clipboard, then simulate ⌘V. This needs the Accessibility permission, and without it macOS drops the keystroke silently, so EasyVox checks first and shows an error.
- ⌘V is sent as key code 9 (where V sits on a US keyboard) with only Command held. Non-QWERTY layouts may need a layout lookup.
- The transcript stays on the clipboard on purpose, which means whatever was there before is lost. History covers this, since every transcript is saved. A "restore previous clipboard" setting could come later.
- EasyVox only ever writes the clipboard, never reads it. On macOS 26, reading the clipboard can trigger a privacy prompt.
- If no text field has focus, the text is still on the clipboard and in history.
- Fallbacks for apps where ⌘V misbehaves: simulated typing (slow for long text) or inserting through the Accessibility API (not every app supports it).

### History

- Newest first, one-click copy per entry.
- Store text, timestamp, duration and which model was used. If cleanup was on, keep the raw text too.
- Later: search, delete, a retention limit.
- **Text only; no audio is stored.** Keeping audio would allow re-transcribing with a better model later, but it costs disk (about 2 MB a minute uncompressed) and raises privacy concerns.

### Local storage

- **Data:** models and history live in `~/Library/Application Support/EasyVox/` on macOS (`~/.local/share/easyvox/` on Linux): a `models/` folder and `history.sqlite3`.
- **Settings:** a TOML file at `~/.config/easyvox/config.toml` (under `$XDG_CONFIG_HOME` when that's set). The app watches it and applies changes live, and the Settings window writes the same file, so agents and dotfiles can configure EasyVox. Agents still can't grant Microphone or Accessibility; a person has to click those.
- For development and tests, `EASYVOX_DATA_DIR` moves the data folder, `EASYVOX_MODELS_DIR` moves just the models, and `EASYVOX_CONFIG` points at a different settings file.

### Swappable models

- The app talks to one engine interface: start a session, feed audio (16 kHz mono), receive provisional and final text, finish.
- Each runtime gets an adapter behind that interface. Today: transcribe.cpp (in the app) and sherpa-onnx (bench only). Core ML or Apple Speech could get adapters later.
- The model catalog (`crates/easyvox-core/src/models.rs`) lists each model's id, name, runtime, offering (default, alternative or bench only), size, download URL (pinned to a Hugging Face commit), sha256 checksum, license and attribution. Adding a model means adding an entry, plus an adapter if it needs a new runtime.
- Settings screen: pick a model, download or delete models, see disk usage. Which models are offered is covered in "Models" below.

## Models

The list is tight on purpose: one opinionated default and two alternatives, each with a one-line "pick this if". Every model on it runs on transcribe.cpp, the one speech runtime built into the app. No model ships inside the app. Each one goes through the same flow: download, sha256 check, then activate. Download URLs are pinned to a Hugging Face commit (`resolve/<commit>/<file>`), so a re-upload can't break the sha256 check.

| | Model (GGUF) | Download | Pick this if… | License |
|---|---|---|---|---|
| **Default** | Parakeet Unified EN 0.6B, Q5_K_M | 541 MB | You want the best all-round model. | NVIDIA Open Model License |
| Alternative | Parakeet TDT 0.6B v2, Q5_K_M | 539 MB | You want words to appear sooner while you talk (about 0.5 s instead of 1.35 s), and don't mind more GPU and CPU use while speaking. | CC-BY-4.0 |
| Alternative | Whisper base.en, Q8_0 | 85 MB | You want the smallest model (an 8 GB Mac, or memory matters more than the last bit of accuracy). | Apache-2.0 |

Choices as of 9 October 2026, settled by a local bench the same day ([`docs/model-bench.md`](docs/model-bench.md)). This area moves fast, so re-check before changing the list. Accuracy numbers are average word error rates (WER, lower is better) and only compare within one source.

In the code, each catalog entry in `crates/easyvox-core/src/models.rs` has an `offering`: `Default` (exactly one), `Alternative`, or `BenchOnly`. Settings lists the default first, then the alternatives, and only the ones that are downloaded and runnable in this build.

### Default: Parakeet Unified EN 0.6B

**Downloaded during onboarding, not bundled.** This keeps the DMG small, app updates don't re-ship 541 MB, and onboarding already has to ask for the Microphone permission and wait out the ~14 s first-launch shader compile, so the download fits there. The catch: the first run needs a network connection.

Why it's the default:

- **Accuracy.** In [transcribe.cpp's own runs](https://github.com/handy-computer/transcribe.cpp/tree/main/docs/models) of each GGUF, it has the lowest FLEURS English error of any model under 1B parameters (3.99 %). NVIDIA's same-pipeline comparison on its [model card](https://huggingface.co/nvidia/parakeet-unified-en-0.6b) puts it ahead of TDT v2 (5.91 vs 6.04). It isn't on the Open ASR Leaderboard itself, which is the main gap in the evidence.
- **One model, both jobs.** It's the only model that gives both the live preview (buffered streaming) and the full-accuracy final pass from one set of weights.
- **Speed and size.** In the October bench: final text 0.09–0.46 s after release for 3–30 s of speech, 7–9 % of one core while you talk, about 690 MB of memory. If 1.35 s to first words feels slow, try a smaller streaming chunk first (0.85 s, at about twice the compute) before switching models.

### Alternatives

- **Parakeet TDT 0.6B v2 (Q5_K_M): faster first words.** It's offline-only, so EasyVox re-decodes the recent audio for the preview. First words appear after about 0.46 s, but it uses 15–37 % of one core while you speak, against Unified's 7–9 %. It's one of the best models under 1B parameters on the [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) (6.43). **Quantization: Q5_K_M, not Q8_0.** In the bench, Q5_K_M wrote exactly the same text as Q8_0 on every clip, showed first words just as fast, and finished within 10–20 ms of it (its decodes are about 5 % slower). In exchange it's 191 MB smaller to download and uses about 190 MB less memory (690 vs 880 MB). The Q8_0 entry is gone from the catalog.
- **Whisper base.en (Q8_0): smallest.** 85 MB, about 205 MB of memory. On our clips it made as few errors as Canary 180M Flash on ordinary sentences (1.5 %) and wrote numbers as digits. Its known flaw: it makes up words on silence (2 s of silence came out as "you"), and it once added a stray "Mhm." at the end of a long clip. The app should skip the final pass when the recording holds no speech (VAD or a level check), whichever model is active.
- **Canary 180M Flash: benched, not offered.** It was the candidate for the small slot: more accurate than Whisper base.en on the leaderboard (7.48 vs 11.10 on the June 2026 version), and the empty-transcript issue ([transcribe.cpp #140](https://github.com/handy-computer/transcribe.cpp/issues/140)) never showed up (0 empty results in 84 decodes, nothing on silence). But on transcribe-cpp 0.3.1 it writes every number as words ("three hundred and forty two dollars", "five five five zero one four two"): the runtime has no number-formatting (ITN) switch for Canary. That puts it at 7.1 % WER on our clips against Whisper's 1.4 %, which fails the "no worse than Whisper base.en" bar, and digits matter for dictation. Worth re-checking if a later transcribe.cpp adds ITN for Canary.

### Licenses

The catalog has a `license` (name and URL), an `attribution` line and the upstream model card for every model. The app shows the license and attribution next to each model in Settings.

- **NVIDIA Open Model License** ([agreement](https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/)): Parakeet Unified. Free use, including commercial use and redistribution. The app must show "Licensed by NVIDIA Corporation under the NVIDIA Open Model License" with a link to the agreement. The `handy-computer` GGUF repo labels it CC-BY-4.0, but NVIDIA's [model card](https://huggingface.co/nvidia/parakeet-unified-en-0.6b) says the Open Model License, and upstream wins.
- **CC-BY-4.0** ([license](https://creativecommons.org/licenses/by/4.0/)): Parakeet TDT v2 ([card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2)), and later Parakeet Ultra ([card](https://huggingface.co/moondream/parakeet-ultra)). Free for any use; credit the creator (NVIDIA, or Moondream for Ultra) and link the license.
- **Apache-2.0** ([license](https://www.apache.org/licenses/LICENSE-2.0)): Whisper base.en ([card](https://huggingface.co/openai/whisper-base.en)). Permissive.

### Bench only

These stay in the catalog (`offering: BenchOnly`) for `easyvox-bench` and comparisons, but the app doesn't offer them:

- Canary 180M Flash (Q5_K_M, CC-BY-4.0), for the reason above;
- the two sherpa-onnx models (Parakeet TDT 0.6B v2 int8, CC-BY-4.0, and Nemotron Speech Streaming 0.6B, NVIDIA Open Model License) and the Silero VAD model. The app doesn't compile that runtime in (it would add 18.5 MB), so they could never show up there anyway.

### Later: French

Parakeet Ultra (Q5_K_M, 550 MB), Moondream's further-trained Parakeet v3, would take v2's slot. Its English is about v2's, and it has the best French of any fast model in transcribe.cpp's runs (4.66 % on FLEURS French). It needs transcribe-cpp 0.4, because its support landed after the pinned 0.3.1 ([PR #192](https://github.com/handy-computer/transcribe.cpp/pull/192)). Parakeet TDT v3 runs on 0.3.1 today as a stopgap.

### Other models we looked at

| Model | Why not (for now) |
|---|---|
| Canary 180M Flash | Benched: writes numbers as words on transcribe-cpp 0.3.1, so 7.1 % WER on our clips against Whisper base.en's 1.4 %. Otherwise clean. See "Alternatives". |
| Qwen3-ASR 1.7B | The best open model on the leaderboard (5.75), but 1.5 GB and about 4× slower than Parakeet. The strongest candidate for a future "maximum accuracy" option. |
| Cohere Transcribe, Canary-Qwen 2.5B, Granite Speech 4.x | Accurate, but 1.6–2 GB and 2.5–4× slower, for a small gain. Cohere also silently cuts transcripts short past ~50 s ([#128](https://github.com/handy-computer/transcribe.cpp/issues/128)). |
| Granite Speech 5.0 TurboCTC | The fastest here, but by all signs it outputs lowercase text with no punctuation. |
| Nemotron Speech Streaming EN | Streams natively, but writes numbers as words (6.6 % WER on our clips) and trails v2 on the leaderboard. |
| Whisper large-v3-turbo, Distil-Whisper | Slower and less accurate than Parakeet. Turbo took 1–2.7 s to finish in the spike and makes up text on silence. |
| Moonshine Streaming | Small and streams natively, but no more accurate than Canary 180M and slower on long audio. |
| Phonon-2, Parakeet Redux | About 170 MB with near-v3 accuracy (2-bit and ternary Parakeet v3), but they don't run on transcribe.cpp yet ([#189](https://github.com/handy-computer/transcribe.cpp/issues/189)). If they ever do, they'd be the best candidates for the small slot. |
| Kyutai STT | Doesn't run on transcribe.cpp; it would need a second runtime. |
| Apple SpeechAnalyzer | Built into macOS 26: no download, no memory cost to us. Less accurate than Parakeet in one independent test, and needs Swift bridging. A possible extra. |

Runtimes:

- **transcribe.cpp** (`handy-computer/transcribe.cpp`, from the team behind Handy): ggml with Metal on the GPU. Runs Parakeet, Canary and Whisper GGUF models, among others. In the spike it ran Parakeet 4–5× faster than ONNX Runtime did on the CPU. The app's runtime.
- **sherpa-onnx / ONNX Runtime**: CPU, many models, macOS and Linux. The fallback runtime, behind the core's optional `sherpa` feature.
- **whisper.cpp / `whisper-rs`**: not used. It vendors its own copy of ggml, which can't be linked next to transcribe.cpp's; Whisper GGUF models run on transcribe.cpp instead.
- **WhisperKit, FluidAudio, MLX**: Swift or Mac-only. Would need bridging; not planned.

## The "two model" approach

People use this phrase for two different setups. EasyVox will do both, kept light.

**A. Speech model + language model cleanup.** The speech model produces raw text. A small local LLM then removes filler words, fixes punctuation, handles self-corrections ("Tuesday, no, Wednesday" becomes "Wednesday") and formats lists.

- Local LLM options: small open models like Qwen3 or Gemma 3 through llama.cpp (macOS and Linux). On Mac, Apple's Foundation Models framework (part of macOS 26, on Macs with Apple Intelligence) costs us no disk or memory.
- Catch: llama.cpp bindings vendor their own ggml, and only one ggml can be linked into a binary next to transcribe.cpp. Cleanup will need a way around that, such as a separate helper process, or a llama.cpp build that shares transcribe.cpp's ggml.
- Costs: extra wait after release, more memory, and the LLM can change what you said.
- Plan: optional, off by default, loaded only when on. Keep the raw text in history.

**B. Fast preview + accurate final text.** Something fast streams text while you talk. On release, the whole recording gets one clean pass, and that result is what gets pasted.

- Done: the default model, Parakeet Unified, streams the preview and then does one offline pass over the whole recording on release. You get the benefit of B without a second model in memory.

Combined flow: live preview → final transcript → optional cleanup.

## Stack: Rust + GPUI Kit

**Decided** (October 2026), after a spike that built EasyVox twice, once on Tauri and once on GPUI Kit, around the same Rust core. Both worked. GPUI Kit was lighter with no model loaded (one process instead of five, about half the idle memory, much cheaper overlay updates), it's all Rust, and its pop-up window is already the kind of panel the overlay needs. The hands-on checks (real key presses, focus, paste, mic) are still pending and will run on the real app. Details and numbers: `docs/ui-stack-spike.md`, with the full reports in `docs/spikes/`.

The choice is cheap to reverse: everything except the thin shell lives in the core, so a Tauri shell could replace it without touching the rest.

### Workspace layout

A Cargo workspace:

- `crates/easyvox-core`: the UI-agnostic core: audio capture, the speech engines, live partials, paste and history. No UI code, so the shell can change without touching it.
- `crates/easyvox-app`: a thin GPUI Kit shell (binary `easyvox`): menu bar icon, shortcut, overlay, Settings, History, and a controller that drives the core. macOS-specific code lives under `src/platform/`, so a Linux port can add its own module there later.
- `crates/easyvox-bench`: a developer CLI that measures speed, memory and word errors on the test clips. Not shipped.
- `testdata/`: short test clips spoken by macOS voices (`say`), with their transcripts.
- `scripts/`: model download and app bundling.
- `packaging/`: `Info.plist` and other bundle files.
- `docs/`: the stack decision and the spike reports.

### Engine

- **Parakeet Unified on transcribe.cpp** (ggml, Metal on the GPU). One loaded model streams the live preview while you talk, then runs one offline pass over the whole recording on release (`EngineConfig::recommended`). See "Models" for the list.
- **sherpa-onnx** is an optional fallback runtime (ONNX Runtime on the CPU, the core's `sherpa` feature). It isn't compiled into the app.
- **One ggml and one ONNX Runtime per binary.** Libraries that vendor their own copy (`whisper-rs`, llama.cpp bindings, `ort`) can't sit next to ours. The linker quietly keeps one copy, so this fails at runtime, not at build time.

### GPUI Kit

GPUI is the GPU-rendered UI framework behind the Zed editor, and GPUI Kit (`longbridge/gpui-kit`, Apache-2.0) adds ready-made components on top. Everything is Rust: no HTML, CSS or JavaScript, and no web view. It runs on macOS, Windows and Linux.

- **Exact version pins.** GPUI Kit ships weekly releases with breaking changes, and pins its own pre-release copy of GPUI. The workspace pins it exactly (`gpui-kit = "=0.7.1"`, which pins `gpui-pre =0.3.8`). Upgrade on purpose, and expect some work each time.
- **Overlay.** GPUI's `PopUp` window kind is already a non-activating panel on macOS, above other windows and on every Space. A small native shim adds click-through and a borderless style, plus a key guard so the overlay can never take keyboard focus (and a paste can never land in it). On Wayland, GPUI's `LayerShell` window kind is the standard way to draw overlays.
- **Menu bar icon and shortcut.** GPUI has neither built in. The `tray-icon` and `global-hotkey` crates (from the Tauri team, used without Tauri) work alongside it.
- **Light and dark.** GPUI Kit starts in light mode. The app has to follow the system appearance itself and switch when it changes.
- **Rough edges.** Some mistakes are runtime panics rather than compile errors, and a panic inside an AppKit callback aborts the whole app instead of unwinding. Opening a window uses a deprecated AppKit call to bring it to the front.
- **Builds.** Nothing compiles Metal shaders at build time, so building doesn't need Apple's Metal toolchain (`xcrun metal`). GPUI Kit turns on GPUI's `runtime_shaders` feature, and ggml embeds its shader source (`GGML_METAL_EMBED_LIBRARY`); both compile their shaders when the app first runs. Revisit if a GPUI Kit upgrade drops `runtime_shaders`. Compiles are slow-ish: about 2 minutes for a clean release build on an M4.
- **No bundler, signing or updater.** See "Signing and notarization".

### Engine and macOS gotchas

Learned the hard way in the spike. The code handles each one; they're here so nobody undoes the fix.

- **Microphone hang.** If the Microphone permission hasn't been asked yet, opening the mic hangs with no error. Onboarding asks first, and the core refuses to open the mic until then.
- **First model load is slow once.** The first load under a new app identity (bundle id) compiles ggml's Metal shaders: about 14 s, then about 0.2 s on every later load. Warm it up during onboarding, or ship precompiled shaders.
- **Idle wake-ups.** ggml's Metal code wakes the process about 210 times a second for as long as its GPU device exists, even after the model is unloaded. `GGML_METAL_NO_RESIDENCY=1` stops it (3.5 wake-ups a second), but its effect on the first transcription after a long idle is unmeasured. A small upstream fix could make the loop sleep when idle.
- **Quitting with a model loaded** aborts inside ggml. A hook in the core frees the model at exit.
- **Ending a session blocks** until its worker thread finishes, so the app drops sessions off the UI thread.
- **Menu bar icons on macOS 26** are drawn by Control Center, so tools that look for the app's own windows don't find them.

### Alternatives considered

- **Rust + Tauri.** The more mature path, with a tray, bundler, signing and an updater built in, and Handy as a working example. Dropped: five processes and twice the idle memory, a UI in HTML/CSS/JavaScript, and an overlay that leans on a third-party plugin calling a private AppKit API. Its spike report is kept in `docs/spikes/tauri.md`.
- **Swift + SwiftUI.** The most native and lightest option on Mac, with direct access to every Apple framework. Rules out Linux without a rewrite.
- **Rust core + SwiftUI app.** The most native result, but two UIs to build eventually.
- Applies to any Rust option: Mac-only frameworks (SpeechAnalyzer, Foundation Models, Core ML) need bridging to Swift. We can skip them at first, since Parakeet and llama.cpp cover the core features on both platforms.

## Platform and distribution

- Apple silicon only. Intel Macs are too slow for the bigger models, and macOS 26 is the last release that supports them.
- **macOS 26.0 or later.** Forward-looking on purpose: macOS 27 is close, so supporting older releases isn't worth it. A bonus: the macOS 26 frameworks (SpeechAnalyzer, Foundation Models) need no fallbacks.
- **Bundle id: `dev.easyvox.EasyVox`.** Don't change it after release: macOS ties permission grants and the Metal shader cache to it.
- Permissions: Microphone, Accessibility (paste), possibly Input Monitoring (modifier-only shortcuts).
- Three ways to install:
  - a DMG on GitHub Releases, with auto-updates (updater not chosen yet);
  - a Homebrew cask pointing at that same DMG;
  - build-from-source steps in the README.
- The DMG holds only the app, about 25 MB with the engine (10 MB zipped). Models are downloaded in the app (see "Models").
- No App Store. Its sandbox gets in the way of simulated paste and global key monitoring, so most apps like this skip it.
- **Linux later.** On Wayland (the default on most current Linux desktops), global shortcuts and simulated paste are restricted for security reasons, and GNOME has no layer-shell for overlays. Linux support will take real per-desktop work, not just a rebuild. Handy documents how it handles this. X11 is workable today.

### Signing and notarization (macOS)

- A downloaded app opens without a warning only if it's signed with a Developer ID certificate and notarized by Apple. Both require the paid Apple Developer Program, and there's no free route.
- Without them, macOS blocks the app on first launch. Since macOS 15, the user has to go to System Settings → Privacy & Security → Open Anyway; the old right-click → Open trick no longer works.
- Homebrew's main cask repo has also been phasing out apps that aren't signed and notarized.
- Apps you build yourself from source don't get flagged, so contributors don't need a paid account.
- **Development builds** are signed ad hoc (`codesign -s -`) by `scripts/bundle.sh`, which needs no certificate. The catch: macOS ties the Microphone and Accessibility permissions to the app's signature, and an ad-hoc signature changes on every rebuild, so each rebuild resets those permissions.
- **Shipping builds** need a Developer ID Application certificate. Only the Apple account holder can create one; the current Apple Distribution certificate can't be used for this.
- `cargo-packager` is the planned tool for Developer ID signing, notarization and the DMG. Automate it in GitHub Actions with the certificate and an App Store Connect API key stored as secrets.
- Linux has no equivalent gate. Ship AppImage, .deb or Flatpak.

## Prior art

- **Handy** (`cjpais/Handy`): MIT, Rust + Tauri, Windows/macOS/Linux. Hold-to-talk, paste at cursor, Silero VAD, Whisper and Parakeet v3. Its README describes transcribing after release, with no live preview. Very close to this plan. Its team also makes transcribe.cpp and the GGUF builds of the models EasyVox uses.
- **VoiceInk** (`Beingpax/VoiceInk`): GPL-3.0-based, Swift, macOS. Local Whisper with optional AI cleanup. EasyVox is MIT, so we can't copy its code.
- **Superwhisper, Wispr Flow, MacWhisper**: commercial. Useful for UX ideas.

## Stretch goals

To address after the rest of the app is done.

- **French.** Two versions:
  - **French dictation:** needs a multilingual model. The current English Parakeet models can't do it. Parakeet Ultra is the pick (see "Later: French").
  - **Speak French, get English:** Whisper has a built-in translate-to-English mode, or the optional cleanup LLM could translate. A live English preview while speaking French is the hard part, because translation needs more context than transcription, so the preview would lag or change more.

## Backlog and open questions

The backlog lives on the tuhdoo ledger, not in this file: run `tuhdoo backlog` in the repo (see `CLAUDE.md`). Open questions are held tasks labelled `decision`, and checks that need a person at an unlocked screen are labelled `hands-on`.

Resolved in October 2026, each recorded in its section: the UI layer (Rust + GPUI Kit), the license (MIT, see `LICENSE`), the minimum macOS (26.0), the bundle id (`dev.easyvox.EasyVox`), the model list (Parakeet Unified default; Parakeet TDT v2 and Whisper base.en as downloads), audio in history (no) and the settings file (TOML, applied live).
