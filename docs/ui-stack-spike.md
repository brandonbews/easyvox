# UI stack spike: Tauri vs GPUI Kit

**Decision: Rust + GPUI Kit.** Tauri is dropped; only its report is kept. The hands-on checks below weren't done on the spikes. They're pending, and will be done on the real app instead (see "Hands-on checks").

October 2026. Three agents built throwaway versions of EasyVox on this Mac (M4, 16 GB, macOS 26.6.2):

- **`easyvox-core`**: a UI-agnostic Rust crate for audio, speech models, live partials, paste and history.
- **A Tauri v2 shell** (2.12.2).
- **A GPUI Kit shell** (`gpui-kit` 0.7.1 on `gpui-pre` 0.3.8).

Both shells implement the same features and run the same real core end to end.

The full reports are in [`spikes/`](spikes/README.md): [`core.md`](spikes/core.md), [`gpui-kit.md`](spikes/gpui-kit.md) and [`tauri.md`](spikes/tauri.md), plus window snapshots. The spike code and the raw JSON evidence lived in a throwaway folder that is no longer kept; the code worth keeping was ported into `crates/`.

## Bottom line

- **Both stacks can build EasyVox on macOS.** Every feature was built in both, and neither hit a blocker.
- **The speech engine dominates the cost, not the UI.** With the model loaded, the two apps idle within about 30 MB of each other (666 vs 636 MB). Release-to-clipboard time is identical (about 0.2 s) because it's the same engine.
- **GPUI Kit is lighter when the model isn't loaded:**
  - one process instead of five
  - about half the idle memory: 22 vs 51 MB footprint, or 60 vs 169 MB RSS
  - much cheaper overlay updates
  - all Rust
- **Tauri is the more mature, better-trodden path:**
  - built-in tray, bundling and notarization
  - a stable 2.x API
  - Handy, an open source app very close to EasyVox, uses it
- **Overall hackiness is similar, but it sits in different places:**
  - Tauri's overlay leans on a third-party plugin that does a runtime class swap and calls a private AppKit API.
  - GPUI Kit's overlay is mostly built into GPUI, plus about 60 lines of our own native code.
  - GPUI Kit's bigger risk is churn, not hacks: weekly releases, exact version pins, and runtime panics where you'd want compile errors.
- **The same checks remain unverified in both.** The screen was locked, so nobody could press real keys, check focus, see a paste land, or use the mic.

**Recommendation (adopted): GPUI Kit.** It best matches "lightweight, fast, Rust, Linux maybe." The choice is also cheap to reverse. Everything except the shell lives in `easyvox-core`: swapping the spike stubs for the real crate took one Cargo line plus 150–250 lines of app code in each shell. Switching to Tauri later would mean rewriting only a small shell. Pick Tauri instead if you'd rather follow Handy's footsteps and not track a fast-moving framework.

## Feature comparison

Hackiness: **0** built in · **1** maintained plugin/crate · **2** small native shim of our own · **3** big or fragile workaround · **4** couldn't do it.

| Feature | Tauri | GPUI Kit | Notes |
|---|---|---|---|
| Menu bar icon + menu | 0, built in | 1, `tray-icon` crate | Both verified in-process. A real click needs hands-on. |
| Push-to-talk press/release (⌥Space) | 1, official plugin | 1, `global-hotkey` crate | Same Carbon API underneath, no permission needed. Registration verified; real keys need hands-on. |
| Change shortcut at runtime + recorder | 1 | 1 | Verified with synthetic keys. Both must pause the live hotkey while recording. |
| Hold right ⌥ (stretch) | 2, ~45 lines of logic | 2, ~60 lines | Same event-tap code in both. Only the "no permission" path could be verified. |
| Overlay: on top, all Spaces, never takes focus | 1 via `tauri-nspanel` (the plugin hides a class swap + private API), or 2 hand-rolled (~20 lines, no class swap) | 2: GPUI's `PopUp` is already a non-activating panel; ~30-line shim + ~30-line key guard | Window properties verified in both: never key, app never active. "Previous app keeps focus" needs hands-on. |
| Overlay click-through | 0, Tauri API | 2, part of the shim | Verified (property). |
| Live text, provisional words dimmed | 0 | 0 | Verified. Tauri read back the page's text after every update; GPUI was made to draw each update and snapshotted to PNG. |
| Settings + History windows | 0, hand-written HTML | 0, GPUI Kit Settings, virtual list, copy button | Verified rendering, 200-row history, Copy. "Comes to front" needs hands-on; **both** use the deprecated `activateIgnoringOtherApps`. |
| Clipboard + ⌘V paste | 2, in core | 2, in core | Same code. Clipboard write and the "grant Accessibility" message verified; the paste landing needs hands-on. |
| Real engine end to end (WAV replay) | 0 | 1 | Verified in both: first text 1.35 s after press, final text about 0.2 s after release, 0 word errors on an 11 s clip. Clean quit with the model loaded. |
| `.app` bundle, ad-hoc signed | 0, Tauri bundler | 1, 26-line script | Verified. |
| Developer ID signing + notarization | 0, the bundler does it (the DMG is a manual extra step) | 1, `cargo-packager` | Research only. You need a **Developer ID Application** certificate; this Mac only has Apple Distribution. |
| Auto-updates | Tauri updater plugin | `cargo-packager-updater` or a Sparkle shim | Not tested in either. |

## How hacky it turned out

**Tauri.** It needed the least custom code overall, and almost everything uses stable, documented APIs. The exception is the overlay, which is the most important feature:

- `tauri-nspanel` turns Tauri's window into a panel by swapping its class at runtime.
- It calls the private `_setPreventsActivation:` selector.
- A transparent window needs Tauri's `macOSPrivateApi` flag, which also rules out the App Store (not a goal anyway).

The plugin is widely used (Handy, Cap, screenpipe), so it's well tested, but it's someone else's hack in your critical path. The hand-rolled alternative is about 20 lines of plain AppKit calls and works too. It lacks only the non-activating flag, which click-through makes irrelevant.

Traps found only by reading Tauri's source:

- `show()` makes the window key, which is wrong for the overlay.
- The shortcut plugin calls your handler while holding its own lock, so re-registering from inside the handler deadlocks.

**GPUI Kit.** More of the native code is ours (about 60 lines of `objc2`), but the hard part of the overlay comes built in: GPUI's `PopUp` window is a non-activating panel at pop-up level, on all Spaces. Our shim adds:

- click-through and borderless styling;
- a key guard, which swaps in a subclass at runtime that refuses keyboard focus, so a paste can never land in the overlay. It's rated 2, borderline 3.

The missing pieces (tray, global hotkey, click-through, window level, bundler) are each filled by a small crate or a few lines.

The real friction is the framework's age:

- weekly breaking releases with exact pins;
- runtime panics such as "entity already being updated";
- a panic inside an AppKit callback kills the app instead of unwinding.

## Metrics

From the agents' reports. The screen was locked, so neither stack drew real frames on screen. On-screen rendering cost is still unmeasured.

| | Tauri | GPUI Kit |
|---|---|---|
| `.app` size, with engine | **14.4 MiB** (5.7 MB zipped) | 25 MB (10.1 MB zipped) |
| Idle, UI only (stub engine) | 51 MB footprint, 5 processes (169 MB RSS) | **22 MB** footprint, 1 process (60 MB RSS) |
| Idle, model loaded | 666 MB footprint | 636 MB footprint |
| Overlay updating 10×/s (stub) | 5.0–5.3 % of one core | **0.12 %** (0.69 % with a forced draw per update) |
| Real 30 s session | 9.9 % of a core (overlay ~0.8) | 10.2 % (UI ~0.3) |
| Press → first text / release → clipboard | 1.35 s / 0.19–0.21 s | 1.35 s / 0.20–0.21 s |
| Launch → menu bar icon ready | ~150–170 ms | ~140–170 ms |
| Clean release build | **87 s** | 109 s |
| Incremental build, debug / release | 1.2 s / 13–15 s | 0.9–1.5 s / 21–24 s |

Measurements ran while other agents were compiling. Each agent waited for a quiet machine where it could, so treat small differences as noise.

## Engine findings (same for either stack)

- **Engine:** Parakeet Unified EN 0.6B on transcribe.cpp, which runs on the GPU via Metal. One model handles both the live preview and the final pass:
  - 0.55 % word error rate on the test clips; they're computer-generated speech, which is easier than real voices
  - final text 0.09 / 0.21 / 0.49 s after release for 3 / 11 / 30 s of speech
  - about 10 % of one core while you speak
  - 541 MB download, about 670 MB of memory while loaded
- **Runtime choice:** transcribe.cpp is 4–5× faster than the same model on ONNX Runtime CPU (sherpa-onnx), which stays as the fallback. Whisper also runs on transcribe.cpp. `whisper-rs` can't be linked alongside it, because they bundle clashing copies of ggml.
- **Idle wake-ups:** ggml's Metal code wakes the process about 210 times a second whenever its GPU device exists, **even after the model is unloaded**. Unloading after idle frees the memory (639 → 88 MB) but not the wake-ups. `GGML_METAL_NO_RESIDENCY=1` stops them (measured: 3.5 wake-ups/s) but may slow the first transcription after a long idle; that's unmeasured. Options: measure that delay, or contribute a small upstream fix so the loop sleeps when idle.
- **First launch:** ggml compiles its Metal shaders once per app identity, which took about 14 s. Do this during onboarding, or ship precompiled shaders.
- **Microphone:** untested. With permission "not determined", macOS's audio system hangs when the app opens the mic, with no error. Onboarding must ask for permission first, and the core now enforces that.

macOS 26 surprises:

- ⌥Space works as a global shortcut. The Option-only restriction added in macOS 15 didn't apply.
- Creating an event tap succeeds without Input Monitoring permission, so check with `CGPreflightListenEventAccess()` instead.
- Menu bar icons are drawn by Control Center windows, not the app's own.
- The clipboard privacy alerts only apply to reading, so EasyVox should never read the clipboard.

## Linux outlook (desk research)

Both stacks would need per-desktop work on Wayland for push-to-talk, the overlay and paste.

- **GNOME on Wayland:** the hardest case for any toolkit. It has no layer-shell, the standard Wayland way to draw an overlay.
- **GPUI:** has built-in layer-shell windows and click-through regions.
- **Tauri:** goes through GTK3 layer-shell, and adds WebKitGTK quirks. Handy ships its overlay **off by default on Linux** for this reason.
- **X11:** workable today with either.

## Hands-on checks (pending)

**Not done yet.** The stack was chosen without them. They'll run on the real EasyVox app, not the spikes, once its shell is ported. They need a person at the Mac with the screen unlocked:

- [ ] Grant **Microphone** and **Accessibility** to the app. Dev builds are ad-hoc signed, so every rebuild may reset these grants.
- [ ] Real ⌥Space presses: hold to record, release to transcribe.
- [ ] The overlay shows live text while **the target app (e.g. TextEdit) keeps focus**.
- [ ] The paste lands at the cursor on release.
- [ ] The microphone delivers audio (only WAV replay has been tested so far).
- [ ] The same over a full-screen app, and in another Space.
- [ ] Settings and History come to the front when opened from the menu bar.
- [ ] Idle CPU in Activity Monitor, with and without a window open.

If one of these fails in a way GPUI Kit can't fix, the fallback is a Tauri shell around the same core (see the recommendation above).

## Next steps

1. Do the hands-on checks on the real app.
2. Create a Developer ID Application certificate. Only the account holder can.
3. Core follow-ups:
   - the idle wake-up fix
   - warming the shaders during onboarding
   - a mic-permission onboarding flow
   - unload the model after idle
