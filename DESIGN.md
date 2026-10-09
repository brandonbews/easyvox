# EasyVox design notes

Working notes, not final decisions. Anything marked **Open** still needs a decision.

## Requirements

- All processing happens on the Mac. No audio or text leaves the machine. The only network use is downloading models.
- English only for now. French is a stretch goal (see below).
- Lightweight and fast: low memory use when idle, and text pasted quickly after you release the key.
- Push-to-talk on a shortcut the user picks: hold to record, release to transcribe.
- Show text as it's recognized, in chunks, so you don't wait until the end.
- On release, paste the transcript at the cursor. The transcript stays on the clipboard until something else replaces it.
- History of past transcripts, each with a quick copy button.
- Local storage for history, settings and downloaded models.
- Models are easy to swap, since we'll add options over time.
- Use both "two model" approaches (see below) without giving up speed or lightness.
- macOS first. Linux possibly later.

## Core flow

1. Hold the shortcut. Recording starts.
2. A small floating window shows the text as it comes in.
3. Release. EasyVox finishes the transcript, puts it on the clipboard and pastes it at the cursor.
4. The transcript is saved to history.

## Keeping it light and fast

- The speech model drives speed and memory far more than the UI framework does.
- Keep the speech model loaded so recording starts instantly, and unload it after a stretch of no use. Loading on key-down would add a noticeable delay.
- Use one speech model for both the live preview and the final text, so two models never sit in memory at once (see "two model" approach B).
- LLM cleanup is optional and off by default. The LLM loads only when cleanup is turned on.
- Set a latency target once we can measure, for example: a short sentence pasted within half a second of release, without cleanup.

## Feature notes

### Push-to-talk shortcut

- Needs both key-down and key-up events.
- Regular combos (like ⌥Space) are straightforward.
- Holding a modifier on its own (right ⌥, Fn) needs a lower-level event tap and the Input Monitoring or Accessibility permission. macOS uses Fn/Globe for its own features by default.
- Later: a toggle mode (tap to start, tap to stop) for long dictation.

### Live text while speaking

- Most speech models aren't built for streaming. Common workarounds:
  - Re-run the model on the recent audio every half second or so. Show text in two states: confirmed (stable) and provisional (may still change). Text is confirmed once two passes in a row agree.
  - Use a model that streams natively: streaming Parakeet variants, Kyutai STT, or Apple's SpeechAnalyzer (macOS 26+ only).
  - Use voice activity detection (Silero VAD) to cut the audio into chunks at pauses.
- Live text goes in EasyVox's own floating window, not the target app. Typing provisional text into another app and then correcting it is fragile. The final text is pasted once, on release.

### Paste at cursor

- Standard approach: put the text on the clipboard, then simulate ⌘V. This needs the Accessibility permission.
- The transcript stays on the clipboard on purpose, which means whatever was there before is lost. History covers this, since every transcript is saved. A "restore previous clipboard" setting could come later.
- If no text field has focus, the text is still on the clipboard and in history.
- Fallbacks for apps where ⌘V misbehaves: simulated typing (slow for long text) or inserting through the Accessibility API (not every app supports it).

### History

- Newest first, one-click copy per entry.
- Store text, timestamp, duration and which model was used. If cleanup was on, keep the raw text too.
- Later: search, delete, a retention limit.
- **Open:** keep the audio? It allows re-transcribing with a better model later, but it costs disk (about 2 MB per minute uncompressed, far less compressed) and raises privacy concerns. Suggest off by default.

### Local storage

- Everything lives in `~/Library/Application Support/EasyVox/` on macOS (`~/.local/share/easyvox/` on Linux): a SQLite database for history and a `models/` folder.

### Swappable models

- Download models on demand instead of bundling them. They range from about 50 MB to over 1.5 GB.
- The app talks to one engine interface: start a session, feed audio (16 kHz mono), receive provisional and final text, finish.
- Each runtime (whisper.cpp, ONNX, Core ML, Apple Speech) gets an adapter behind that interface.
- A model catalog file lists each model's id, name, runtime, size, download URL and checksum. Adding a model means adding an entry, plus an adapter if it needs a new runtime.
- Settings screen: pick a model, download or delete models, see disk usage.

## Models to explore

English only, which opens up some faster English-specific models. My information runs to mid-2026 and this area moves fast, so re-check before choosing.

| Model | Size | Notes |
|---|---|---|
| NVIDIA Parakeet TDT 0.6B v2 | 600M params | English. Very fast and accurate, near the top of the Hugging Face Open ASR leaderboard. Leading candidate for both preview and final text. |
| Whisper large-v3-turbo | 809M | Multilingual, strong accuracy. Fallback default if Parakeet disappoints. |
| Whisper `.en` models (tiny / base / small / medium) | 39M–769M | English-tuned Whisper. The small ones could serve as preview models. |
| Distil-Whisper large-v3 | 756M | English. Faster than full large-v3. |
| Moonshine tiny / base | 27M / 61M | English. Built for low latency on small devices. |
| Kyutai STT 2.6B | 2.6B | English. Streams natively. Probably too heavy for "lightweight." |
| Apple SpeechAnalyzer | Built in | macOS 26+ only. Streams natively, no download, no memory cost to us. Possible Mac-only extra. Not available on Linux. |

Runtimes and libraries:

- **whisper.cpp**: C++, Metal GPU, optional Core ML for the Neural Engine. Has Rust bindings (`whisper-rs`). MIT.
- **sherpa-onnx / ONNX Runtime**: one runtime for many models, including Parakeet. Works on macOS and Linux. Has a C API with bindings for many languages.
- **WhisperKit** (Argmax, now in the `argmax-oss-swift` repo): Swift, Core ML, Neural Engine, streaming. MIT. Mac only.
- **FluidAudio**: Swift, Core ML. Parakeet, VAD, speaker separation. Apache-2.0. Mac only.
- **MLX**: Apple's ML framework. `parakeet-mlx` runs Parakeet on it. Mac only.

## The "two model" approach

People use this phrase for two different setups. EasyVox will do both, kept light.

**A. Speech model + language model cleanup.** The speech model produces raw text. A small local LLM then removes filler words, fixes punctuation, handles self-corrections ("Tuesday, no, Wednesday" becomes "Wednesday") and formats lists.

- Local LLM options: small open models like Qwen3 or Gemma 3 through llama.cpp (macOS and Linux). On Mac, Apple's Foundation Models framework (macOS 26+, Macs with Apple Intelligence) costs us no disk or memory.
- Costs: extra wait after release, more memory, and the LLM can change what you said.
- Plan: optional, off by default, loaded only when on. Keep the raw text in history.

**B. Fast preview + accurate final text.** Something fast streams text while you talk. On release, the whole recording gets one clean pass, and that result is what gets pasted.

- Plan: use the same loaded model for both, such as streaming chunks through Parakeet for the preview and then one full pass on release. You get the benefit of B without a second model in memory.

Combined flow: live preview → final transcript → optional cleanup.

## Stack: Rust?

Possible Linux support later tips this toward Rust. **Open:** which UI layer.

The UI is small: a menu bar icon, the live-text overlay, settings and history. The hard part is the overlay. It has to appear on top without taking focus from the app you're typing in, or the paste lands in the wrong place.

- **Rust + GPUI Kit.** GPUI is the GPU-rendered UI framework behind the Zed editor, and GPUI Kit (`longbridge/gpui-kit`, Apache-2.0) adds 75+ ready-made components on top. Everything is Rust: no HTML, CSS or JavaScript, and no web view. It runs on macOS, Windows and Linux.
  - Overlay: GPUI's `PopUp` window kind is already a non-activating panel on macOS, which is exactly what we need. On Wayland, its `LayerShell` window kind is the standard way to draw overlays.
  - No menu bar icon built in. Use the `tray-icon` crate (from the Tauri team, works without Tauri), but test that it works alongside GPUI first.
  - Risks: GPUI is pre-1.0 and built for Zed first, and GPUI Kit pins a pre-release copy of it, so upgrades may break things. Compiles are slow. Fewer people have built an app like this with it.
  - No built-in bundler, signing or updater. Use `cargo-packager` or a short script around Apple's `codesign` and `notarytool`.
- **Rust + Tauri.** One codebase for macOS and later Linux. Tauri uses the system's web view instead of bundling a browser, so it stays far lighter than Electron. whisper.cpp, ONNX Runtime and llama.cpp all have Rust bindings. Handy is a working example of this exact stack.
  - Also includes a menu bar icon, an updater, and signing and notarization in its bundler. The `tauri-nspanel` plugin handles the non-activating overlay on macOS.
  - Cost: the UI is HTML/CSS/JavaScript talking to Rust. The web view adds some memory, though far less than the speech model uses.
  - Applies to any Rust option: Mac-only frameworks (SpeechAnalyzer, Foundation Models, Core ML) need bridging to Swift. We can skip them at first, since Parakeet and llama.cpp cover the core features on both platforms.
  - Linux caveat: on Wayland (the default on most current Linux desktops), global shortcuts and simulated paste are restricted for security reasons. Linux support will take real work, not just a rebuild. Look at how Handy handles it.
- **Swift + SwiftUI.** The most native and lightest option on Mac, with direct access to every Apple framework. Rules out Linux without a rewrite.
- **Rust core + SwiftUI app.** The engine, models and history live in Rust and are shared, with a native UI per platform. The most native result, but two UIs to build eventually.

## Platform and distribution

- Apple silicon only. Intel Macs are too slow for the bigger models, and macOS 26 is the last release that supports them.
- **Open:** minimum macOS version. It only matters if we use the macOS 26 frameworks. Without them, macOS 14 or 15 is fine.
- Permissions: Microphone, Accessibility (paste), possibly Input Monitoring (modifier-only shortcuts).
- Three ways to install:
  - a DMG on GitHub Releases, with auto-updates;
  - a Homebrew cask pointing at that same DMG;
  - build-from-source steps in the README.
- No App Store. Its sandbox gets in the way of simulated paste and global key monitoring, so most apps like this skip it.

### Signing and notarization (macOS)

- A downloaded app opens without a warning only if it's signed with a Developer ID certificate and notarized by Apple. Both require the paid Apple Developer Program, and there's no free route.
- Without them, macOS blocks the app on first launch. Since macOS 15, the user has to go to System Settings → Privacy & Security → Open Anyway; the old right-click → Open trick no longer works.
- Homebrew's main cask repo has also been phasing out apps that aren't signed and notarized.
- Apps you build yourself from source don't get flagged, so contributors don't need a paid account.
- Sign development builds with your Apple Development certificate. macOS ties Microphone and Accessibility permissions to the app's signature, so unsigned rebuilds keep losing those permissions.
- Tauri's bundler can sign and notarize. Automate it in GitHub Actions with the certificate and an App Store Connect API key stored as secrets.
- Linux has no equivalent gate. Ship AppImage, .deb or Flatpak.

## Prior art

- **Handy** (`cjpais/Handy`): MIT, Rust + Tauri, Windows/macOS/Linux. Hold-to-talk, paste at cursor, Silero VAD, Whisper and Parakeet v3. Its README describes transcribing after release, with no live preview. Very close to this plan, and the best reference if we go with Rust.
- **VoiceInk** (`Beingpax/VoiceInk`): GPL-3.0-based, Swift, macOS. Local Whisper with optional AI cleanup. Because of the GPL, copying its code would make EasyVox GPL too.
- **Superwhisper, Wispr Flow, MacWhisper**: commercial. Useful for UX ideas.

## Stretch goals

To address after the rest of the app is done.

- **French.** Two versions:
  - **French dictation:** needs a multilingual model (Parakeet TDT v3 or Whisper). The current English Parakeet models can't do it.
  - **Speak French, get English:** Whisper has a built-in translate-to-English mode, or the optional cleanup LLM could translate. A live English preview while speaking French is the hard part, because translation needs more context than transcription, so the preview would lag or change more.

## Open questions

1. UI layer: GPUI Kit or Tauri. The spike results are in `docs/ui-stack-spike.md`; hands-on checks are pending.
2. Minimum macOS version.
3. Keep audio in history?
4. License: MIT (permissive) or GPL-3.0 (copyleft).
5. Should agents be able to set it up? That means a plain settings file (format and location to decide, e.g. `~/.config/easyvox/settings.toml`) that the app reads at launch and reloads when it changes, with the Settings window writing the same file. Agents still can't grant Microphone or Accessibility; a person has to click those.
