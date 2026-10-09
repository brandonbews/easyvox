# Core spike report (`easyvox-core`)

## Summary

The speech side of EasyVox works well on this M4 from Rust. The surprise is the runtime: **Parakeet on transcribe.cpp (ggml + Metal, from the team behind Handy) is 4–5× faster than the same model on ONNX Runtime CPU** (30 s of audio in ~0.4 s instead of 2.1 s), keeps the CPU mostly idle, and links statically. The recommended start is **Parakeet Unified EN 0.6B (Q5_K_M GGUF)**: one model does both native streaming partials (~10% of one core, update every 0.56 s) and an offline final pass on release (0.09 / 0.21 / 0.49 s for 3 / 11 / 30 s clips, 0.55% WER on our TTS set). Everything links statically (no `.dylib` in the `.app`); the two gotchas are that whisper-rs and transcribe.cpp can't share a binary (two vendored ggml copies) and that ggml-metal aborts at process exit if a model is still loaded (worked around in the crate). The microphone could not be verified here: with permission "not determined", CoreAudio **hangs** opening the stream (no error, no audio, no visible prompt), so the crate now refuses to open the mic until permission has been requested.

## Feature results

| Feature | Status | Hackiness (0–4) | How (API / crate / shim) | Evidence |
|---|---|---|---|---|
| Engine: Parakeet TDT 0.6B v2, sherpa-onnx (ORT CPU) | verified | 1 | official `sherpa-onnx` 1.13.8 crate, static ORT | `core-evidence/bench-parakeet-tdt-0.6b-v2-int8.json`, `sessions-…int8.json` |
| Engine: Parakeet TDT v2 + Unified, transcribe.cpp (Metal) | verified | 2 | `transcribe-cpp` 0.3.1 + atexit shim for a ggml-metal exit abort (`tcpp.rs:41-70`) | `bench-parakeet-*-gguf.json`, `sessions-parakeet-*-gguf*.json`, `stream-wer/` |
| Engine: Whisper (base.en, large-v3-turbo q5_0), whisper-rs (Metal) | verified | 1 | `whisper-rs` 0.16 (whisper.cpp 1.8.3) | `bench-whisper-*.json`, `sessions-whisper-*.json` |
| Engine: Nemotron streaming 0.6B (native streaming) | verified | 1 | sherpa-onnx `OnlineRecognizer` | `bench-nemotron-*.json`, `sessions-nemotron-*.json` |
| Swappable models (one trait, catalog, id string) | verified | 0 | own `SpeechModel`/`StreamDecoder` traits, 4 adapters, 8 catalog models | `lib.rs`, `models.rs`; same session code ran all 8 |
| Live partials, re-decode + local agreement | verified | 1 | own code (`partials.rs`, `session.rs:220+`), unit-tested; 0 confirmed-text regressions in all runs | `sessions-*.json` (`confirmed_violations: 0`) |
| Live partials, native streaming | verified | 1 | transcribe.cpp buffered streaming / sherpa online | `unified-stream-matrix/`, `sessions-nemotron-*.json` |
| Final pass on `stop()` | verified | 1 | full offline decode (optionally VAD-trimmed) | `sessions-*.json` (`finalize_ms`) |
| WAV replay at real time | verified | 1 | `hound` + 20 ms paced feeder thread | every `sessions-*.json` |
| Microphone capture (cpal → 16 kHz mono) | not achieved (blocked by TCC here); needs hands-on | 1 | `cpal` 0.18 + own resampler (unit-tested) | `mic-attempt-1.json`, `mic-attempt-2.json`, `mic-attempt-2-stack-sample.txt` |
| Mic permission status / request | status verified; request needs hands-on | 2 | `objc2-av-foundation` (~30 lines, `audio.rs:166-208`) | `mic-gated.json`, `easyvox-bench info` |
| Silero VAD silence trim (nice-to-have) | verified | 1 | sherpa-onnx `VoiceActivityDetector` | silence → `""` instead of "Thank you."/"you" (below) |
| Clipboard write (left on clipboard) | verified | 2 | `objc2-app-kit` NSPasteboard (~15 lines) | `paste-probe.json` (changeCount 390→391, read-back matches) |
| Paste (simulated Cmd+V) | needs hands-on (AX error path verified) | 2 | CGEvent via `objc2-core-graphics` + `AXIsProcessTrusted` extern (~20 lines) | `paste-probe.json` (`accessibility_trusted: false`, clear error) |
| History (SQLite) | verified | 1 | `rusqlite` 0.40 `bundled` | `cargo test` (2 tests), `easyvox-bench history` |

## Hackiness details

- **Paste + clipboard, `easyvox-core/src/paste.rs:17-66` (~50 lines, rating 2).** `NSPasteboard.generalPasteboard` `clearContents` + `setString:forType:`, `AXIsProcessTrusted()` from ApplicationServices via a 3-line `extern "C"`, and Cmd+V as two `CGEventCreateKeyboardEvent(source, kVK_ANSI_V=9, down/up)` with `MaskCommand`, posted to `kCGHIDEventTap`. Standard and well understood. Fragility: key code 9 is the ANSI "V" position, so non-QWERTY layouts may need a layout lookup; without Accessibility macOS drops the events silently, which is why the code checks first and errors.
- **Mic permission, `easyvox-core/src/audio.rs:166-208` (~30 lines, rating 2).** `AVCaptureDevice authorizationStatusForMediaType:` and `requestAccessForMediaType:completionHandler:` through `objc2-av-foundation` + `block2`. Small and documented. It exists because of the hang described under "Microphone" below.
- **transcribe.cpp exit shim, `easyvox-core/src/tcpp.rs:41-70` + the `atexit` registration in `TcppModel::load` (~30 lines, rating 2).** ggml-metal 0.25 runs `GGML_ASSERT([rsets->data count] == 0)` in a C++ static destructor; if any model is still loaded at `exit()` the process aborts (exit 134, which shows up as "quit unexpectedly"). Reproduced with `EASYVOX_LEAK_ENGINE=1 examples/replay` (4/4 aborts). Fix: keep native handles in `Option`s, register `libc::atexit` *after* the first model load (so it runs before ggml's destructor) and free every live model. After the fix: 5/5 clean exits. Moderately fragile (depends on atexit ordering; a decode still running at exit would skip the release via `try_lock`). `Session::drop` also joins its worker thread, which fixed a one-off abort at exit seen before.
- **ggml duplicate libraries (not a shim, a constraint).** `whisper-rs-sys` (ggml 0.9.5) and `transcribe-cpp-sys` (ggml 0.25.3) both emit `-lggml`, `-lggml-base`, …; the linker resolves both to one file. A combined build linked and both engines produced correct text, but `nm` showed the binary held mostly the 0.25.3 symbols, so whisper.cpp was running on a foreign ggml. `lib.rs:20-28` now refuses the combination with `compile_error!`.

## Metrics

All numbers on the M4 (10 cores, 16 GB), release builds, 4 threads, measured only when the 1-minute load average was below 2.5–3.0 (other agents compile here; the load is stored in each JSON). MB = 10⁶ bytes. Memory is the process **physical footprint** (Activity Monitor's "Memory"; includes Metal buffers on unified memory). Decode times are medians of 3 runs.

### Models: size, load, memory, speed, accuracy

| Model (runtime) | Download / on disk | Load cold / warm | Footprint after load / idle / peak | RSS idle / peak | Decode 3 s / 11 s / 30 s | RTF | WER (365 words) |
|---|---|---|---|---|---|---|---|
| **Parakeet Unified EN 0.6B Q5_K_M (tcpp, Metal)** | 541 / 541 MB | 275 / 145 ms | 692 / 673 / 743 MB | 726 / 741 MB | 66 / 159 / 427 ms | 0.015 | **0.55%** (2) |
| Parakeet TDT 0.6B v2 Q8_0 (tcpp, Metal) | 730 / 730 MB | 391 / 269 ms | 879 / 857 / 925 MB | 908 / 923 MB | 59 / 144 / 390 ms | 0.014 | **0.55%** (2) |
| Parakeet TDT 0.6B v2 int8 (sherpa-onnx, CPU) | 482 / 661 MB | 835 / 616 ms | 1120 / 1296 / 1425 MB | 1475 / 1585 MB | 205 / 718 / 2077 ms | 0.068 | 1.10% (4) |
| Nemotron streaming 0.6B 560 ms int8 (sherpa, CPU) | 464 / 662 MB | 976 / 766 ms | 1138 / 834 / 1164 MB | 1009 / 1182 MB | 730 / 2245 / 5876 ms¹ | 0.205¹ | 6.58% (24)² |
| Whisper base.en f16 (whisper-rs, Metal) | 148 / 148 MB | 118³ / 73 ms | 264 / 307 / 351 MB | 332 / 332 MB | 81 / 136 / 366 ms | 0.015 | 2.19% (8) |
| Whisper base.en Q8_0 (tcpp, Metal) | 85 / 85 MB | 72 / 70 ms | 206 / 213 / 259 MB | 254 / 256 MB | 86 / 127 / 306 ms | 0.014 | 1.37% (5) |
| Whisper large-v3-turbo q5_0 (whisper-rs, Metal) | 574 / 574 MB | 302³ / 170 ms | 725 / 780 / 826 MB | 807 / 808 MB | 1013 / 1094 / 2173 ms | 0.126 | 0.55% (2) |
| Silero VAD (sherpa) | 0.64 MB | – | – | – | ~5 ms per clip | – | – |

- **Cold** = first load in a fresh process from a fresh APFS clone of the files (new vnode, so not in the page cache; `purge` needs sudo). **Warm** = reload in the same process. After unloading, footprint drops back to 21–153 MB (`after_unload`), so "unload after idle" really frees the memory.
- ¹ Nemotron is a streaming model; the offline number is the stream fed as fast as possible. In live use that work is spread over the recording (see partials).
- ² 18 of Nemotron's 24 errors are numbers spelled out ("three hundred forty two dollars") — it has no inverse text normalization. Before I fed it 0.5 s of leading silence it also dropped the first word of every clip (9.3% WER).
- ³ whisper-rs compiles its embedded Metal library on the first load after each new build: 5.8 s and 14.6 s observed; later processes load in ~0.1 s. transcribe.cpp didn't show this.
- Silence (2 s of zeros): Parakeet and Nemotron output nothing; Whisper large-v3-turbo writes "Thank you.", base.en GGUF writes "you". With `vad_trim` both return `""` (VAD costs ~5 ms).
- WER is a simple normalized WER (lowercase, punctuation stripped, "$342" = "342 dollars", small number words = digits, "Dr." = "doctor"). **TTS speech is much easier than real dictation**: clean, evenly paced, no disfluencies, no room noise or accents beyond `say`'s voices. Treat these as best-case numbers useful for *ranking*; most remaining errors are rare names ("Siobhan", "Joaquin").

### Live partials and final pass (WavReplay sessions at real time)

finalize = time from end of audio (key release) to `Final`, including waiting for an in-flight partial pass. CPU % is of one core, averaged over the whole session (wall clock). GPU is ioreg "Device Utilization %" averaged over the session (relative to the GPU's current clock, so it overstates energy; idle ≈ 0–6%).

| Engine / strategy | finalize 3 s / 11 s / 30 s | finalize, partials off | First partial | Update cadence (median) | Provisional revisions⁴ | CPU (one core) | GPU util |
|---|---|---|---|---|---|---|---|
| **Unified, native stream 560/160 + `final_redecode`** | **90 / 207 / 487 ms** | – | 1.35 s | 0.56 s | 0–12% | 10–11% | 62% |
| Unified, native stream 560/160, streamed final | 86 / 82 / 98 ms | – | 1.36 s | 0.56 s | 0–12% | 9–11% | 62% |
| Unified, native stream 160/160 | 69 / 91 / 91 ms | – | 0.85 s | 0.17 s | 13–27% | 18–19% | 86% |
| Unified, model default 1040/1040 | 93 / 152 / 148 ms | – | 2.2 s | 1.04 s | 0–25% | 5–7% | – |
| TDT v2 Q8_0 (tcpp), re-decode 400 ms | 120 / 184 / 408 ms | 105 / 191 / 444 ms | 0.47 s | 0.42 s | 20–35% | 19–45% | 72–78% (5% without partials) |
| TDT v2 int8 (sherpa CPU), re-decode 400 ms | 376 / 1468 / 2060 ms | 318 / 725 / 2046 ms | 0.47 s | 0.46–0.53 s | 14–29% | 165–344% | – |
| Nemotron streaming (sherpa CPU) | 286 / 98 / 115 ms | – | 0.9 s | 0.56 s | 9–33% | 112–136% | – |
| Whisper base.en (whisper-rs), re-decode | 252 / 304 / 639 ms | 155 / 202 / 598 ms | 0.5 s | 0.42 s | 52–67% | 11–29% | – |
| Whisper large-v3-turbo (whisper-rs), re-decode | 1376 / 2046 / 2658 ms | 981 / 1047 / 2193 ms | 1.35 s | 1.0–1.1 s | 20–50% | 2–8% | – |

- ⁴ Share of partial updates that rewrote previously shown text (provisional churn). "Confirmed" text never changed in any run.
- WER of the *streamed* text (no final re-decode), all 13 speech clips: Unified 1040/1040 0.55%, 560/160 1.37%, 160/160 1.64%; Nemotron 6.58% (`core-evidence/stream-wer/`). With `final_redecode` the pasted text is the offline 0.55% result whatever the streaming window.
- Share of the final text already confirmed when the key was released: 91–99% for Parakeet re-decode, 73–100% for Unified streaming, 82–93% for Whisper base.en.
- Re-decode cost scales with the window: Parakeet CPU passes average 145 / 356 / 514 ms for 3 / 11 / 30 s clips (20 s window cap), which is why ORT CPU partials keep 3–4 cores busy.
- Under CPU contention ONNX Runtime degrades badly (spin-waiting thread pool): the same 34 s clip took 2.4 s quiet and 7.4–11.2 s while another agent's `rustc` used 850% CPU (`bench-parakeet-under-load-contaminated.json`). I didn't measure the Metal engines under the same contention.

### Build and binary size

| Metric | Value |
|---|---|
| Clean release build, default features (transcribe-cpp + sherpa-onnx + cpal), 146 crates | 29.4 s wall, 182 CPU-s (sherpa archive from `vendor/`; +~1 s to download it) |
| First release build of the library alone, whisper-rs + sherpa-onnx (before the bench crate existed) | 27.9 s wall, 108 CPU-s |
| Incremental release rebuild after touching `easyvox-core` / the bench binary | 1.9 s / 1.2 s |
| Stripped binary, no engines (history, clipboard, paste, bench CLI) | 2.7 MB |
| + cpal (mic) | +0.2 MB |
| + transcribe-cpp (ggml + Metal shaders) | +4.6 MB (7.3 MB) |
| + whisper-rs (whisper.cpp + Metal) | +1.8 MB (4.5 MB) |
| + sherpa-onnx (ONNX Runtime 1.28 + kaldi/fst/espeak libs) | +18.5 MB (21.2 MB) |
| Default features | 26.0 MB |

Source: `core-evidence/binary-sizes/sizes.tsv`.

## Recommendation

**Start with Parakeet Unified EN 0.6B (Q5_K_M GGUF, 541 MB) on transcribe.cpp/Metal, using its native streaming for the live preview and one offline pass of the whole recording on release** (`EngineConfig::recommended`, streaming window chunk 560 ms / right context 160 ms).

Why:
- It is DESIGN.md's approach B with a single model in memory: the same weights stream the preview and produce the final text. 673 MB footprint, loads in 0.15–0.3 s, so keep it loaded and unload after idle.
- Fast release: 0.09 s for a short command, 0.2 s for a 10 s sentence, 0.5 s for 30 s, with the best accuracy measured (0.55% WER, tied with TDT v2 and large-v3-turbo). This meets the "short sentence pasted within half a second" target with room to spare.
- Light while speaking: ~10% of one CPU core plus the GPU, versus 165–344% CPU for the same-size model on ONNX Runtime with re-decode partials. Partials never need a second model.
- Static link, no `.dylib`, Metal shaders embedded; the same runtime also runs Whisper GGUF models, so EasyVox needs only one ggml runtime. transcribe.cpp and these GGUF conversions come from the Handy team (`handy-computer`), and Handy's README recommends Parakeet Unified, which is a decent maturity signal.

Trade-offs and tuning:
- First words appear ~1.35 s after speech starts. If that feels slow, use chunk 160 ms (0.85 s, updates every 0.17 s, ~19% CPU, more GPU), or switch to **Parakeet TDT 0.6B v2 Q8_0 with re-decode partials** (first text 0.47 s, 0.12–0.41 s finalize, but 20–45% CPU and more GPU while speaking, 857 MB). Both are already in the catalog; it's a one-line change.
- Drop `final_redecode` to paste the streamed text instead: ~0.1 s finalize at any length, 1.4% WER.
- Fallback runtime: sherpa-onnx (official, mature, identical on Linux) with Parakeet TDT v2 int8 — works, but costs 3–4 cores during partials, 2 s finalize for 30 s, 1.3 GB footprint, and is fragile under CPU contention.
- Risks: transcribe-cpp is young (0.3.x, README still says "in development", 0.1→0.2 was breaking), the exit-abort shim depends on atexit ordering, and it can't share a binary with whisper-rs. Keep the `SpeechModel` trait boundary so the sherpa adapter stays a drop-in fallback.
- Whisper: base.en is the "tiny and fast" option (213–307 MB, 1.4–2.2% WER here); large-v3-turbo matches Parakeet's accuracy but takes 1–2.7 s to finalize and hallucinates on silence (use `vad_trim`). Nemotron streaming (sherpa, CPU) shows text after 0.9 s and finalizes in ~0.1 s, but lacks number formatting (6.6% WER) and keeps more than a full core busy.

## Microphone (what happened)

- `mic_permission_status()` → `NotDetermined` (AVFoundation, never prompts).
- Attempt 1 (`mic-attempt-1.json`): opening the default input didn't return within the 5 s timeout. No error, no audio, no prompt anyone could see.
- Attempt 2 (`mic-attempt-2.json`, 25 s timeout, step tracing and `sample`): cpal found the default input "Jump Desktop Microphone" (8 ch, 48 kHz, f32) within 21 ms, then `build_input_stream` blocked in `AudioComponentInstanceNew → AUHAL → AudioDeviceCreateIOProcID → HALC_ProxyIOContext::_TellServerAboutStreamUsage → mach_msg` (waiting on coreaudiod) for the full 25 s (`mic-attempt-2-stack-sample.txt`, `mic-attempt-2-trace.log`, `mic-attempt-2-tcc-log-excerpt.txt`). The unified log shows only TCC *preflight* checks for `kTCCServiceMicrophone` returning `authValue=1` (unknown), attributed to the responsible process `herdr` (the terminal multiplexer hosting this shell). Status was still `NotDetermined` afterwards. I stopped after two attempts.
- Code consequences: `start(Microphone)` now returns an error immediately when the status is `NotDetermined` (`mic-gated.json`) or denied; `request_mic_permission()` is provided for onboarding (not called here: nobody can click it); a blocked open times out after 5 s instead of hanging the caller; mid-session, 1.5 s of exact digital zeros produces `Error("…not been granted Microphone permission")`.
- Not verified: actual capture, the resampler on real device audio (unit test only: 48 kHz → 16 kHz length and amplitude), device switching.

## Clipboard and paste (what happened)

- `AXIsProcessTrusted()` = false, so `paste_text` returns "Accessibility permission is missing, so Cmd+V can't be simulated … The text is on the clipboard." The Cmd+V events were never posted.
- Clipboard write verified from metadata: `changeCount` 390 → 391 and `public.utf8-plain-text` present.
- Read-back: a child process read the text back in 7 ms with no alert, and `pbpaste` matched. `NSPasteboard.accessBehavior` reported **2 (alwaysAllow)** for this unbundled CLI, so macOS 26's paste-privacy prompt didn't apply here. A bundled app starts at `default` (asks on programmatic *reads*). EasyVox only writes, and writes don't prompt, but the UI apps should avoid reading the pasteboard (e.g. to restore old contents) without testing that hands-on.

## Developer experience

- **Build:** fast for a C++-heavy stack. sherpa-onnx ships prebuilt static libs (download at build time, overridable with `SHERPA_ONNX_ARCHIVE_DIR`); whisper.cpp and transcribe.cpp compile with cmake in ~20 s each. Clean build ~30 s, incremental ~2 s.
- **Docs:** sherpa-onnx has a Rust example for nearly every model (Parakeet, VAD, streaming). The transcribe-cpp README is good, but the Parakeet Unified streaming knobs are documented only in a C header, and the valid (chunk, right) menu lives in GGUF metadata (I read it with a 20-line script). whisper-rs docs are fine.
- **API churn:** cpal 0.17→0.18 changed `StreamConfig`/device naming and error types; whisper-rs's segment API changed in 0.15; transcribe-cpp 0.1→0.2 broke; the official sherpa-onnx Rust crate is new this year. Pin versions.
- **What bit me:** (1) ONNX Runtime slowing 4× under CPU contention from other agents' builds; benchmarks now wait for a quiet machine. (2) Local agreement duplicated words ("and and") because TDT word boundaries jitter by ~80 ms between passes; fixed with an n-gram dedupe at the commit boundary (unit-tested). (3) Nemotron dropping the first word without leading silence. (4) transcribe.cpp returning "model busy" when a dropped stream hadn't been torn down yet; the stream wrapper now joins its thread. (5) The ggml-metal exit abort and the duplicate-ggml link, both above. (6) The mic hang, which gives no error at all.
- **Error messages:** transcribe-cpp's are excellent ("model busy: a stream is active on this model; finish or drop it before run()", "unsupported timestamp granularity"). sherpa-onnx returns a null recognizer for any bad config, so you only get "could not create". cpal's error kinds are good, but the TCC hang never surfaces as an error.

## Linux outlook

Desk research only.
- Engines port directly: sherpa-onnx publishes static and shared libs for Linux x64/aarch64 ([sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx)); transcribe.cpp builds for CPU, Vulkan, CUDA and ROCm ([transcribe.cpp](https://github.com/handy-computer/transcribe.cpp); the Parakeet Unified model card lists 14× real time on a Ryzen 4750U CPU and 25× with Vulkan); whisper.cpp has CPU, Vulkan and CUDA backends.
- Audio: cpal 0.18 adds native PipeWire and PulseAudio hosts next to ALSA ([CHANGELOG](https://github.com/RustAudio/cpal/blob/master/CHANGELOG.md)). No TCC-style permission, but Flatpak needs the audio portal.
- Paste is the hard part: X11 works via XTest (`xdotool`/`enigo`). On Wayland there is no generic synthetic input. Options are `wtype` (wlroots virtual-keyboard protocol), `ydotool` (uinput; needs a daemon and device permissions) or the XDG RemoteDesktop portal (asks the user); Handy documents exactly this matrix ([Handy README](https://github.com/cjpais/Handy)). Clipboard on Wayland needs `wl-clipboard` or a data-control-capable crate. `paste.rs` would need a Linux backend; the rest of the crate is portable.

## Hands-on checklist

Prereq: connect with Jump Desktop, unlock, open **Terminal.app** (not herdr/tmux, so macOS attributes permissions to Terminal), then:
`cd ~/Development/easyvox-spikes/core && source "$HOME/.cargo/env"`.

1. **Mic input device:** System Settings → Sound → Input. "Jump Desktop Microphone" carries your client's mic if Jump audio redirection is on; otherwise pick the Mac's own mic.
2. **Mic permission + capture:** run `./target/release/easyvox-bench mic --request --secs 8`. `--request` calls `request_mic_permission()` (AVFoundation), so expect a prompt like "Terminal would like to access the microphone" → **Allow**, then speak for 8 s. Check the JSON: `requested: "Authorized"`, `opened: true`, `input_peak` > 0, `Partial` events and a `Final` with your words, no "digital silence" error. If no prompt appears, try `EASYVOX_MIC_FORCE_OPEN=1 EASYVOX_MIC_OPEN_TIMEOUT_S=60 ./target/release/easyvox-bench mic --secs 8` (opens CoreAudio directly; that's the path that hung here) and note what happens.
3. Run `./target/release/easyvox-bench mic --secs 8` again without flags: it should open immediately (status `Authorized`).
4. **Real-voice accuracy:** repeat step 3 a few times reading a paragraph, with `--model parakeet-unified-en-0.6b-q5_k_m-gguf`, then `--model parakeet-tdt-0.6b-v2-q8_0-gguf`. Note errors and how quickly partials appear.
5. **Accessibility + paste:** System Settings → Privacy & Security → Accessibility → **+** → add **Terminal** → enable. Open TextEdit with a new document. In Terminal run `sleep 5; ./target/release/easyvox-bench paste --text "Hello from EasyVox"` and click into the TextEdit document within 5 s. Expect the text to appear and `"paste_text": {"ok": true}`.
6. **Clipboard stays:** after step 5, press Cmd+V in another app; the transcript should paste again. Note any "would like to paste" privacy alert (none expected for writes).
7. **UI apps:** grant Microphone and Accessibility to each `.app` (Tauri, GPUI) in the same panes. After every ad-hoc re-sign, re-check Accessibility (toggle off/on if paste stops working).
8. **Quit test:** quit each UI app with a model loaded; there should be no "quit unexpectedly" dialog (the ggml-metal exit abort is handled in easyvox-core).

## Versions used

- macOS 26.6.2 (25G83), Apple M4, 16 GB; Rust 1.99.0; Xcode 27 (Apple clang 21.0.0); cmake 4.4.4.
- `sherpa-onnx` / `sherpa-onnx-sys` 1.13.8 (prebuilt `sherpa-onnx-v1.13.8-osx-arm64-static-lib`, ONNX Runtime 1.28.2).
- `transcribe-cpp` / `transcribe-cpp-sys` 0.3.1 (bundled ggml 0.25.3).
- `whisper-rs` 0.16.0 / `whisper-rs-sys` 0.15.0 (whisper.cpp 1.8.3, ggml 0.9.5).
- `cpal` 0.18.2, `rusqlite` 0.40.2 (`libsqlite3-sys` 0.38.2, SQLite 3.53.2), `hound` 3.5.1, `objc2` 0.6.5, `objc2-app-kit` / `objc2-foundation` / `objc2-core-graphics` / `objc2-av-foundation` 0.3.2, `block2` 0.6.2, `libc` 0.2.190, `anyhow` 1.0.104, `serde_json` 1.0.151.
- Models: sherpa-onnx `asr-models` release (Parakeet TDT 0.6B v2 int8, Nemotron speech streaming en 0.6B 560 ms int8 2026-04-25, silero_vad.onnx); `ggerganov/whisper.cpp` (ggml-base.en, ggml-large-v3-turbo-q5_0); `handy-computer/*-gguf` (parakeet-tdt-0.6b-v2 Q8_0, parakeet-unified-en-0.6b Q5_K_M, whisper-base.en Q8_0). All in `/Users/agentbox/Development/easyvox-spikes/models`.
