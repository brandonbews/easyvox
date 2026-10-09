# GPUI Kit spike report

## Summary

**Phase 2 (real `easyvox-core`):** swapping the stub for the real crate was a one-line dependency change plus about 150 lines of app code, all of it things the stub couldn't exercise: dropping a `Session` off the UI thread, mic-permission gating, persistent history, and model switching. The UI architecture didn't change. End to end with no human input: a synthetic ⌥Space press → WavReplay of an 11 s clip through Parakeet Unified (transcribe.cpp/Metal) → 18 real partials in the overlay → synthetic release → Final → clipboard → SQLite history. The final text has **0 word errors** against the ground truth. The clipboard holds the text **0.20–0.21 s after release**. GPUI (Metal) and ggml-metal link and run side by side without conflicts. Quitting with the model loaded exits with code 0 and leaves no crash report. Two new costs come from the engine layer, not GPUI: the **first launch under a new bundle id takes ~14 s to load the model** (ggml compiles its Metal shaders into a per-app cache), and **ggml-metal wakes the process ~210×/s** while a model is loaded. Details in "Integration with easyvox-core".

**Phase 1 (stub):** GPUI Kit covered every EasyVox UI feature. The only native code is a small, well-understood objc2 shim. The overlay was the expected risk, and GPUI's own `WindowKind::PopUp` turned out to be most of the answer: it creates a `GPUIPanel` (an NSPanel subclass) with the non-activating style bit, `NSPopUpWindowLevel` (101), and `CanJoinAllSpaces | FullScreenAuxiliary`. About 30 lines of objc2 add click-through, a borderless mask, no shadow, and show/hide without activation. `tray-icon` and `global-hotkey` (Tauri team, used without Tauri) work alongside GPUI's NSApplication with no glue beyond a channel. A tray menu item action and a synthetic Carbon hot-key event both travel through the real AppKit/Carbon dispatch into GPUI and open windows or start sessions.

The biggest surprises:

- **Builds are fast on this M4:** with the real core, a clean debug build took 63 s and a clean release build 109 s (stub: 50 s and 80 s); an incremental debug build takes about 1 s.
- **The UI is light; the model is not.** The UI shell alone (stub) idles at about 22 MB physical footprint, 60 MB RSS and about 0 % CPU. With Parakeet Unified loaded, idle is 636 MB footprint and 0.2 % CPU, and a session costs about 10 % of one core. As DESIGN.md predicted, the model dominates.
- **The locked screen hides a lot from testing.** GPUI correctly stops drawing occluded windows, so with the screen locked no frame reaches the screen. On-screen frame cadence and real focus behaviour need a person at the machine. Forced draws plus Metal snapshots (PNG) stand in as evidence that the views render the streamed text.

## Feature results

| Feature | Status | Hackiness (0–4) | How (API / crate / shim) | Evidence |
|---|---|---|---|---|
| Accessory app (no Dock icon) | verified | 0 | `cx.set_activation_policy(ActivationPolicy::Accessory)`, which GPUI documents for menu bar utilities, plus `LSUIElement` in Info.plist | selftest `activationPolicy: 1`; `lsappinfo` shows `ApplicationType = UIElement`; `bundle.LSUIElement: true` |
| F1 Menu bar icon + menu | verified in-process; the human click needs hands-on | 1 | `tray-icon` 0.26.1 (template icon drawn in code), muda menu, `MenuEvent::set_event_handler` → `async_channel` → GPUI task | `1_tray`: NSStatusItem `isVisible`, button window at level 25, icon rect y=0 (in the menu bar). `4_windows.*_via_menu_action`: `performActionForItemAtIndex:` on the real NSMenu reached the controller and opened Settings in 39–75 ms and History in 46–57 ms across runs. Quit via the menu item ended the process. |
| F2 Global push-to-talk (press + release), ⌥Space | registration and handler chain verified; physical keys need hands-on | 1 | `global-hotkey` 0.8.0 (Carbon `RegisterEventHotKey`, which delivers both pressed and released events, no permission needed) | `2_hotkey`: ⌥Space, ⌥⇧Space, ⌃⌥Space, ⌘⇧Space, ⌃⇧F5, ⌘Space and F5 all register on macOS 26.6.2. A synthetic Carbon `kEventHotKeyPressed`/`Released` event posted to the main event queue reached the PTT handler in about 21–22 ms (`3_overlay.carbon_hotkey_injection`). |
| F2 Runtime rebinding + recorder | verified (logic + real key handler); the click-and-press UI needs hands-on | 1 | Unregister and re-register, with rollback if the OS refuses. The recorder is a focusable GPUI div with `on_key_down`. It suspends the OS hot key while recording so Carbon doesn't swallow the combo. | `2_hotkey.rebind_*` ok. `4_windows.shortcut_recorder`: ⌃⇧K was recorded and re-registered, a bare "k" was rejected, and ⌥Space was restored. |
| F2 stretch: hold right ⌥ (CGEventTap) | failure path verified; success needs hands-on (Input Monitoring) | 2 | listen-only `CGEventTapCreate` for `flagsChanged`, gated on `CGPreflightListenEventAccess()` | `2b_modifier_only_tap`: preflight `false` gives a clear "Input Monitoring is not granted…" error. Also found: `CGEventTapCreate` returns **non-NULL** without the permission, so the NULL check alone is not a valid permission test. |
| F3 Overlay: borderless, transparent, rounded pill, bottom-centre | verified (native props + Metal snapshot) | 2 | `WindowKind::PopUp` + `WindowBackgroundAppearance::Transparent` + `titlebar: None` + shim (`native.rs:35-53`) | `props_after_show.styleMask_bits = [NonactivatingPanel]` (borderless), `hasShadow: false`, `isOpaque: false`, frame 640×56 bottom-centre; `reports/gpui-kit-overlay.png` |
| F3 Above other windows, full screen, all Spaces | verified from properties; visual check needs hands-on | 0 | built into GPUI PopUp | `level: 101`, `collectionBehavior_bits: [CanJoinAllSpaces, FullScreenAuxiliary]` |
| F3 Never takes focus / activates | verified structurally; "previous app stays key" needs hands-on | 2 | GPUI's non-activating NSPanel, `focus: false`, our `orderFrontRegardless` (never `makeKey`), `becomesKeyOnlyIfNeeded`, click-through, plus (phase 2) a **key guard**: the panel is isa-swizzled to a subclass whose `canBecomeKeyWindow`/`canBecomeMainWindow` return NO (`native.rs:46-89`) | During show/stream/hide: `isKeyWindow: false`, NSApp `isActive: false`, our app never frontmost. With the screen locked, frontmost is always `loginwindow`, so this can't prove that TextEdit keeps focus. `7_overlay_key_safety`: class `EasyVoxOverlayPanel`, `canBecomeKeyWindow: false`. |
| F3 Click-through | verified (property) | 2 | `setIgnoresMouseEvents(true)`; GPUI has no API for it on macOS | `ignoresMouseEvents: true` |
| F3 Confirmed + dimmed provisional text | verified (snapshot) | 0 | `StyledText::with_highlights`, using `fade_out`, not `color` (see DX) | `gpui-kit-overlay.png`: "dog19" is dimmed |
| F3 ≥10 Hz updates from a background thread | delivery verified; on-screen frame cadence needs hands-on | 0 | engine thread → forwarder thread → `async_channel` → `cx.spawn` → `cx.notify()` | **Stub:** 295 Partials at 106 ms over 31 s, so the pipe sustains 10 Hz. **Real core:** Parakeet Unified sends a Partial only when text changes, about every 0.56 s (its 560 ms chunk). 18 Partials, median gap 564 ms, first one 1.35 s after press. 0 natural paints while occluded; with forced draws, 18 of 18 real Partial texts were painted (`3_overlay.e2e.rendering`, `gpui-kit-overlay-live.png`). |
| F3 Hides after the final text | verified | 1 | `orderOut:` after a timer (600 ms; 2.5 s when showing the Accessibility notice) | `hidden_after_final_ms: 2441`, `props_after_hide.isVisible: false` |
| F4 Settings window (GPUI Kit `Settings`, dropdown, recorder) | renders + opens verified; "comes to front" needs hands-on | 0 | `setting::{Settings, SettingPage, SettingGroup, SettingItem, SettingField::dropdown / switch / render}`, `Kbd`, `Button`; `cx.activate(true)` | `gpui-kit-settings.png`; window `isVisible: true`. Not key and NSApp not active, which is expected while loginwindow owns the foreground. |
| F4 History window (virtual list, 200+ rows, Copy per row) | verified (virtualisation, scroll, copy) | 0 | `v_virtual_list`, `Clipboard` (GPUI Kit copy button → `cx.write_to_clipboard`), `Scrollbar` | `history_list`: 202 entries (200 samples + 2 real transcripts from earlier self-test steps), only 12–13 rows built per render. `scroll_to_item(150)` → visible range 140..153. Copy round-trips through GPUI and NSPasteboard. `gpui-kit-history.png` |
| F5 Clipboard + ⌘V + Accessibility message | clipboard + error path verified; the ⌘V landing needs hands-on | 2 (in core) | `easyvox_core::paste_text` (real core since phase 2). The UI shows the message in the overlay without stealing focus. GPUI `cx.write_to_clipboard` / `read_from_clipboard` handle History copy. | `5_paste`: AX false, core's error text returned, NSPasteboard and GPUI read both equal the final text. An NSPasteboard `changeCount` watcher saw the clipboard change **201–211 ms after release**; Final → clipboard took 4 ms. |
| F6 Engine integration (press → start + overlay, release → stop → Final → paste → history) | **verified with the real easyvox-core** (WavReplay; the mic path needs hands-on) | 1 | `async-channel` + a forwarder thread per session; Session dropped on a background thread; no native code | `3_overlay.e2e`: Carbon press → WavReplay `punct_samantha.wav` → release at 11.0 s → Final 199–211 ms later, WER 0/30, history row with model id. `6_ptt_direct`: release mid-utterance at 8.4 s → Final 181 ms. `8_model_switch`: switched to TDT v2 Q8_0 at runtime (loaded in 302–368 ms); first partial 0.46–0.56 s. `9_mic_gate`: with mic permission NotDetermined, push-to-talk refused with a notice and opened no session. |
| F7 `.app` bundle, ad-hoc signed | verified | 1 | `scripts/bundle.sh` (26 lines: cargo build, copy, Info.plist, `codesign -s -`) | `codesign --verify --strict` passes. With the real core (transcribe-cpp + cpal, no sherpa): **25 MB `.app`, 10.1 MB zipped**. Stub: 19 MB / 8.1 MB. Models are not bundled. |
| F7 Developer ID, notarization, updates | research only | n/a | see below | — |

## Hackiness details

- **Overlay shim (rated 2): `gpui-kit/src/native.rs:24-44, 90-98`, about 30 lines of safe objc2-app-kit.** It reaches the NSView through `raw-window-handle` (GPUI's `Window` implements `HasWindowHandle`), takes `[view window]`, and then:
  - calls `setIgnoresMouseEvents:YES` (click-through)
  - sets `setStyleMask:` to Borderless | NonactivatingPanel (GPUI always adds `Titled`, even with `titlebar: None`)
  - calls `setHasShadow:NO` and `setHidesOnDeactivate:NO`
  - calls `setBecomesKeyOnlyIfNeeded:YES`
  - shows and hides with `orderFrontRegardless` / `orderOut:`

  GPUI tolerates the style change (it does the same for simple full-screen). How fragile it is:
  - It depends on GPUI keeping PopUp = non-activating NSPanel. That has been stable, and Zed's `crates/gpui_macos/src/window.rs` has the same code.
  - The `canBecomeKeyWindow = YES` risk is closed by the key guard (next bullet).
- **Overlay key guard (rated 2, borderline 3): `native.rs:46-89`, about 30 lines.** `GPUIPanel` (GPUI's class for every PopUp/Floating window) answers YES to `canBecomeKeyWindow`. A *non-activating* panel that becomes key takes keyboard focus without activating the app, so the ⌘V would land in the overlay. The realistic paths:
  - **A stray click.** Not possible with click-through, because the window server never delivers the mouse-down. Even with click-through off, `becomesKeyOnlyIfNeeded = YES` keeps the panel from becoming key unless the clicked view asks for it, and GPUI's view doesn't override `needsPanelToBecomeKey`.
  - **`cx.activate(true)`** (called when Settings/History open). If the overlay is the only visible window, AppKit may pick it as the key window on activation.
  - **Any `window.activate_window()` or `makeKeyAndOrderFront`** on the overlay, for example in a future refactor or GPUI change.
  - **Opening Settings while the pill is up.** Settings becomes key (that's user intent), not the overlay.

  The guard: create a runtime subclass of `GPUIPanel` with objc2's `ClassBuilder` whose `canBecomeKeyWindow`/`canBecomeMainWindow` return NO, then `object_setClass` on this one window. Only the overlay is affected; GPUI's ivars, `dealloc` and `isKindOfClass:` checks are inherited. It refuses if the class isn't plain `GPUIPanel`, because swizzling over a KVO subclass would break KVO. The whole run (18 forced draws, show/hide, the key tests) ran on the guarded class without problems.

  The limit of the evidence: with the screen locked, even the *unguarded* panel didn't become key after `makeKeyWindow`, `cx.activate(true)`, or opening Settings, because nothing gets key status under the lock screen. So the self-test proves the guard is installed (`canBecomeKeyWindow: false`) and harmless, not that the risk was real here. AppKit documents that a window answering NO is never made key. Fragility: it depends on the ObjC runtime and on GPUI not checking `class == GPUIPanel` exactly (it uses `isKindOfClass:`).
- **Modifier-only tap (rated 2): `native.rs:322-412`, about 60 lines of `extern "C"` CoreGraphics/CoreFoundation, plus a 25-line probe.**
  - It's standard and well understood.
  - It needs Input Monitoring.
  - Without the permission it fails *silently* (the tap is created but sees nothing), which is why it is gated on `CGPreflightListenEventAccess`.
- **Paste (rated 2, now in core): `core/easyvox-core/src/paste.rs`.** NSPasteboard, `AXIsProcessTrusted`, and CGEvent ⌘V. The UI only calls `easyvox_core::paste_text`. The phase-1 stub version is still in `easyvox-core-stub/src/lib.rs`.
- **Test-only native code (not counted in ratings):**
  - window, app and `CGWindowList` introspection (`native.rs:109-321`)
  - synthetic Carbon hot-key injection (`native.rs:413-465`: `CreateEvent` + `PostEventToQueue`)
  - NSMenu `performActionForItemAtIndex:` (`tray.rs`)

## Integration with easyvox-core

All of this was done without touching `../core`.

**What changed:**

- **Dependency.** `easyvox-core = { path = "../core/easyvox-core", default-features = false, features = ["transcribe-cpp", "mic"] }`. sherpa-onnx is dropped: `EngineConfig::recommended` doesn't need it (`vad_trim` is off), so `SHERPA_ONNX_ARCHIVE_DIR` isn't needed either. That keeps the binary at 25.2 MB stripped; adding sherpa back (`--features with-sherpa`) makes it 42.8 MB (+17.6 MB).
- **Linking.** GPUI's Metal renderer and ggml-metal linked with no duplicate-symbol or linker warnings, and run side by side. During the e2e run, model decode on the GPU overlapped GPUI forced draws and `render_to_image` Metal snapshots without errors.
- **Engine loading.** `Controller::load_engine` builds `EngineConfig::recommended(models_dir)`, runs `Engine::load` on GPUI's background executor, and swaps the engine in on the UI thread. That takes 133–300 ms after launch, without blocking: the tray is ready at about 110–130 ms. The Settings model dropdown now lists core's `catalog()`, filtered to transcribe-cpp models that are on disk, and reloads the engine the same way. The old engine is dropped on the background thread too.
- **Session lifetime.** `Session` is kept in the state machine until `Final` arrives, then dropped on a background thread, because its `Drop` joins worker threads. Phase 1 dropped it on the UI thread at release, which would have blocked the UI for the whole final pass.
- **Contract details the stub had wrong:**
  - `Error` no longer ends the session, because core says a `Final` still follows.
  - A WavReplay `Final` can arrive while the key is still held (end of file), so the state machine accepts `Final` in either phase.
  - Partials are sent only when the text changes.
  - Parakeet Unified rarely sends provisional text, so the dimmed style is seldom visible with this model.
- **Microphone.** Push-to-talk opens the mic only if `mic_permission_status()` is `Authorized`; otherwise the overlay says to use the menu, because CoreAudio hangs while permission is undetermined. A new menu item, **Test with microphone…**, calls `request_mic_permission` off the UI thread and then records 6 s. It's for hands-on use only; automated runs never open the mic. Verified in `9_mic_gate`: status NotDetermined → push-to-talk refused with the notice, no session started.
- **History.** Real transcripts persist through core's `History` (SQLite) in `~/Library/Application Support/EasyVox GPUI spike/history.sqlite`; the self-test uses an in-memory DB. The 200 sample rows stay in memory for the scroll test.
- **Code size.** About 150 lines changed in the app (`controller.rs`, `main.rs`, `settings.rs`, `history.rs`), plus about 200 lines of self-test.

**End-to-end self-test** (`reports/gpui-kit-selftest.json`, `3_overlay.e2e`; release `.app` launched via `open`; screen locked. Ranges cover the two quiet runs, at load 3.0–4.2 and 2.1–2.9; the JSON holds the second.):

| Measure | Result |
|---|---|
| Input | synthetic Carbon ⌥Space press → `WavReplay(core/testdata/punct_samantha.wav)` (10.96 s) → synthetic release at 11.0 s |
| Time to first Partial (press → UI thread) | **1.35 s** (1.34–1.39 s over 4 runs) |
| Partial cadence as the overlay received it | 18 Partials; gaps median **562–564 ms**, min 550, max 582. This is the model's 560 ms streaming chunk, not a UI limit; the stub pushed 10 Hz through the same pipe. |
| Release → Final (UI thread) | **199–211 ms** (core `finalize_ms` 215–216 ms, measured from its own stop) |
| Release → clipboard | **201–211 ms** (independent NSPasteboard `changeCount` watcher); Final → clipboard 4 ms |
| Mid-utterance release (`long_samantha.wav`, released at 8.4 s) | release → Final **181 ms**; release → clipboard 182 ms |
| Final text vs ground truth | "Well, here's the thing, if we ship on Friday, and nobody objects, we'll have the whole weekend to fix bugs, otherwise, we wait until next Wednesday, does that sound reasonable?" is **0 word errors / 30** (punctuation differs: commas for ":" and ".") |
| History | new top row with the final text, model `parakeet-unified-en-0.6b-q5_k_m-gguf`, 10.96 s audio |
| Rendering evidence | 0 natural paints, because the lock screen occludes the panel and GPUI stops its display link. With `force_draw` (calling `Window::draw` after each Partial), 18 paints carried 18 distinct real texts, from "Well, here's" to "…does that sound reasonable"; Metal snapshot `reports/gpui-kit-overlay-live.png`. |
| Runtime model switch (TDT v2 Q8_0) | loaded in 302–368 ms; `cmd_samantha.wav` first Partial 0.46–0.56 s, Final "…I'll be 10 minutes late." (1 "error" is just ten → 10) |
| Quit with the model loaded (menu → Quit, and `cx.quit()`) | exit code **0** both ways; **no new** `easyvox-gpui-*.ips` in `~/Library/Logs/DiagnosticReports`. Core's atexit hook works inside the GPUI app. |

**Friction found** (none of it needed a core change):

1. **First launch under a new bundle id: the model takes ~14 s to load** (14.1 s and 14.7 s observed; 133–165 ms otherwise).
   - Cause: ggml-metal compiles its embedded Metal source, and macOS caches the result per app at `$(getconf DARWIN_USER_CACHE_DIR)dev.easyvox.spike.gpui/com.apple.metal/` (24 MB). Reproduced by moving that cache dir aside.
   - It is **not** per build: a rebuilt binary loaded in 141 ms.
   - Effect: every user's first launch after install, and possibly after macOS/GPU-driver updates, sees "Model still loading…" for ~14 s. GPUI's own runtime-compiled shaders add ~230 ms to the overlay's creation on that launch.
   - Fix (core's call): ship a precompiled `.metallib`, or warm up in onboarding.
2. **ggml-metal's residency-set loop wakes the process every 5 ms** while a model is loaded (`ggml-metal-device.m:983-1001`, vendored in `transcribe-cpp-sys` 0.3.1).
   - Cost: about **210 wake-ups/s and 0.2 % CPU at idle** (UI shell alone: 0.5/s, 0.006 %).
   - The loop keeps running **after the engine is dropped** (footprint falls 639 → 88 MB, wake-ups stay at about 219/s), because it belongs to ggml's process-wide Metal device.
   - The only knob, `GGML_METAL_RESIDENCY_KEEP_ALIVE_S`, changes how long it calls `requestResidency`, not the 5 ms polling.
   - Worth raising upstream for a menu bar app; nothing GPUI-specific.
3. **Session `Drop` blocks** (documented). Handled as above.
4. **The Settings dropdown overflows its row.** GPUI Kit's `SettingField::dropdown` doesn't truncate the long model name ("NVIDIA Parakeet Unified EN 0.6B (Q5_K_M…") at the right edge (`gpui-kit-settings.png`). Cosmetic.

## Metrics

All numbers are from the release `.app`. Machine: M4, 16 GB, screen locked. Raw data is in `reports/gpui-kit-metrics.json` (phase 2) and `reports/gpui-kit-selftest.json`. Phase-2 runs waited for a 1-minute load average below 3.0; the load at the start and end of each window is in the JSON (1.5–2.5 for runtime metrics).

### With easyvox-core (phase 2: Parakeet Unified on transcribe.cpp/Metal, loaded at launch)

| Metric | Result |
|---|---|
| Clean release build + bundle (now 465 + core crates, incl. cmake build of transcribe.cpp/ggml) | **108.9 s** wall (684 s CPU); load 2.9 at start |
| Clean debug build | **62.8 s** wall (444 s CPU) |
| Incremental debug build (one UI file edit) | **0.9–1.5 s** |
| Incremental release build (same edit) | **22.8–23.8 s** (thin LTO) |
| `target/` size | 4.8 GB |
| `.app` size | **25 MB** (binary 25.2 MB stripped); **10.1 MB** zipped. With sherpa-onnx added back: 42.8 MB binary. |
| Launch → tray ready | 115–124 ms (exec), **136–172 ms** via `open` |
| Launch → model ready | 133–300 ms after `main()` (core `load_ms` 131–298). **First launch under a new bundle id: ~14 s** (Metal shader compile; see Integration) |
| Idle, model loaded (30 s) | **0.22 % CPU**, **212 wake-ups/s** (ggml-metal), RSS **700 MB**, footprint **636 MB**, 1 process |
| During a 34 s WavReplay session (25 s window) | **10.2 % CPU** (one core = 100 %), 450 wake-ups/s, RSS **724–726 MB**, footprint **698 MB**. The overlay is occluded, so no on-screen rendering. |
| Same with a forced GPUI layout+paint per Partial | 10.5 % CPU, RSS 724–727 MB. UI rendering adds about 0.3 points at the model's ~1.7 Partials/s. |
| Idle right after that session | 0.30 % CPU, RSS 730 MB, footprint 652 MB |
| Settings + History open, model loaded | 0.21 % CPU, RSS 722 MB, footprint 652 MB (+16 MB for the two GPUI Kit windows, the same delta as in phase 1) |
| After dropping the engine (`--unload-after`) | footprint 639 → **88 MB**; wake-ups stay at about 219/s |

### Stub era (phase 1: UI shell only, scripted stub engine)

| Metric | Result |
|---|---|
| Clean debug build (465 crates, deps already downloaded) | **49.8 s** wall (318 s CPU) |
| Clean release build + bundle (thin LTO, strip) | **79.5 s** wall (525 s CPU) |
| Incremental debug build (one UI file, real code edit) | **0.85–1.0 s** |
| Incremental release build (same edit) | **21.3–21.7 s** (thin LTO dominates) |
| `target/` size | 4.3 GB |
| `.app` size | **19 MB** (binary 18.7 MB stripped, arm64 only); **8.1 MB** zipped |
| Cold launch → tray ready | **503 ms** first exec after rebuild; then 127–130 ms (exec) and **175–183 ms** via `open` (LaunchServices). In-app: tray ready 98–109 ms after `main()` |
| Idle (30 s) | **0.006 % CPU**, 0.5 wake-ups/s, RSS **60 MB**, physical footprint **21.8 MB**, 1 process |
| Overlay streaming at 10 Hz (25 s, Partials at 106 ms) | **0.12 % CPU**, RSS 61–65 MB, footprint 22 MB. The overlay is occluded by the lock screen, so GPUI renders nothing. |
| Same, forcing a GPUI layout+paint per update | **0.69 % CPU**, RSS 62–66 MB. Excludes the Metal present/GPU step, which can't run while locked. |
| Settings + History open, idle | 0.007 % CPU (occluded), RSS **77–80 MB**, footprint **37 MB** |
| Idle redraw check | 0 paints in 2 s with the overlay hidden; 0 natural paints at all while occluded. From GPUI source (`gpui_macos/display_link.rs`): a *visible* GPUI window keeps a CVDisplayLink subscription, waking the main thread every vsync, and draws only when invalidated. Expect a small non-zero idle cost while a window is on screen; measure hands-on (checklist step 9). |

Measurement caveats:

- **Launch method matters.** Processes exec'd from the agent's shell get throttled timers (`sleep(100 ms)` takes about 210–228 ms; this also happens to plain Python). Launched through LaunchServices (`open`), the app gets accurate 10 Hz timers. Idle and streaming runs therefore used `open`.
- **App Nap.** GPUI's `background_executor().prevent_app_nap()` (level 0) is held during sessions. With `open` launches it made no measurable difference.
- **Contention.** Another agent's builds pushed the load to 13–25 at times. Build timings started at load below 3, but the load during a build includes our own 10-core compile. The e2e self-test ran at load 3.0–4.2. Its latencies are paced by real-time audio and a ~200 ms GPU decode, so they barely moved between runs at load 13 (192 ms) and load 2–4 (199–211 ms).

## Developer experience

- **Dependency setup.** One dependency, `gpui-kit = "=0.7.1"`, which pins `gpui-pre =0.3.8` (a Zed snapshot) and re-exports GPUI, base, components and Lucide icons. This matches the README and `examples/ai_recipes`; no git dependencies or `[patch]` are needed.
- **Churn is real.** gpui-kit went 0.6.5 → 0.7.1 and gpui-pre 0.3.5 → 0.3.8 in three weeks (weekly releases). The pin must be exact (the workspace comment cites a break, #3156). Plan for upgrade work.
- **Build times are good.** First-time `cargo fetch` took about 7 s; the M4 handles the heavy crates easily. Incremental release builds take about 21 s because of thin LTO; turn it off for day-to-day work.
- **Docs are good and agent-friendly.** gpui-kit.com (`.md` per page, `llms.txt`), component pages with examples, packaging and auto-update guides, and a `skills/` folder for coding agents. The Settings component (pages, groups, dropdown/switch fields, custom render fields) was a direct fit for EasyVox Settings.
- **What bit me.** All of these are runtime panics, not compile errors:
  - **Entity re-entrancy:** "cannot read Controller while it is already being updated". A view rendered during `open_window` called by the controller read the controller. Fix: read a `Global` instead.
  - **`WindowHandle<V>::update` leases the root view,** so `window.draw()` inside it panics. `AnyWindowHandle::update` doesn't.
  - **Panics abort the process.** A panic inside an AppKit callback ends with "fatal runtime error: failed to initiate panic", not an unwind.
  - **`HighlightStyle.color` blends** over the base colour (white over white stays white). Dimming needs `fade_out`.
  - **`WindowHandle::root()`** exists only with `test-support`.
- **Bringing windows forward is a risk.** GPUI's `cx.activate(true)` calls `activateIgnoringOtherApps:`, which is deprecated since macOS 14 (cooperative activation). From a menu bar click it should still bring the accessory app's window forward, but that is unverified here (checklist steps 6–7).
- **API gaps (macOS).** No click-through, no window-level API, no status item, no global shortcuts. Zed closed its tray PR (zed#44047) unmerged.
- **Alternative tray crate.** `domenkozar/gpui-tray` 0.1.4 has a `gpui-kit` feature, with GPUI actions as menu items and SNI on Linux. Worth a look as an alternative to `tray-icon`.
- **Testing hooks are a plus.** `Window::draw` is public, and the `test-support` feature adds `Window::render_to_image()`. That rendered the real Metal frames to PNG with the screen locked and without Screen Recording permission. The PNGs in `reports/` came from `cargo run --features snapshot -- --self-test`.
- **Binary bloat.** `gpui-base` and `gpui-component` enable all of `objc2-app-kit`'s default features, so the binary links CloudKit, CoreLocation and CoreData. It's harmless but adds bloat.
- **Adding the C++/Metal engine was uneventful.**
  - `objc2` 0.6 and `objc2-app-kit` 0.3 unify between GPUI and easyvox-core.
  - transcribe.cpp/ggml compiles via cmake inside the same `cargo build`.
  - The only build tweak was `[profile.dev.package] transcribe-cpp-sys/transcribe-cpp opt-level = 3`, so dev builds don't run an unoptimised ggml. The `cmake` crate derives the build type from the package's opt-level.
  - No `.dylib`, rpath or extra `RUSTFLAGS` were needed.
- **macOS 26 surprise.** Menu bar extra windows are owned by **Control Center**, not by the app, so a `CGWindowList` filter on our pid finds no status-bar window. The tray evidence therefore comes from tray-icon's NSStatusItem handle.

## Linux outlook

Desk research only.

- **Overlay.** gpui-pre 0.3.8 has `WindowKind::LayerShell` (wlr-layer-shell) plus `Window::set_input_region` for click-through on Wayland ([zed#60161](https://github.com/zed-industries/zed/pull/60161)). This works on KDE, wlroots (Sway, Hyprland, labwc) and niri. **GNOME/Mutter has no layer-shell.** There the fallback is an `xdg_toplevel`: no always-on-top, no positioning, and possibly focus stealing. Handy turns its overlay off on Linux for this reason ([Handy](https://github.com/cjpais/Handy)).
- **Tray.** tray-icon 0.26's new `ksni` feature runs StatusNotifierItem over D-Bus on a worker thread, with no GTK loop. That suits GPUI, which has no GTK ([docs](https://docs.rs/tray-icon/latest/tray_icon/)). GNOME still needs the AppIndicator extension.
- **Global push-to-talk.** `global-hotkey` is X11-only. On Wayland, use the xdg-desktop-portal GlobalShortcuts portal via `ashpd` (GNOME 48+, KDE, Hyprland). It emits Activated and Deactivated, so press and release work ([portal-gnome source](https://gitlab.gnome.org/GNOME/xdg-desktop-portal-gnome/-/raw/main/src/globalshortcuts.c)), but the user picks the binding.
- **Paste.** wtype needs virtual-keyboard-v1, which GNOME and KDE lack. ydotool/dotool need uinput access. The RemoteDesktop portal works but shows a consent dialog. GPUI's clipboard covers the copy step.
- **Verdict.** GPUI's Linux backends exist, but the overlay, shortcut and paste each need per-desktop work (as for Tauri). GNOME Wayland is the hard case.

## Signing, notarization, updates (research only)

- **Certificate.** Notarization requires a **Developer ID Application** certificate with hardened runtime (`--options runtime`), `--timestamp`, and no `get-task-allow` ([Apple](https://developer.apple.com/documentation/security/resolving-common-notarization-issues)). The user's **Apple Distribution certificate can't be used** for direct distribution. Only the paid program's **Account Holder** can create a Developer ID certificate ([Apple](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/)).
- **Entitlements.** Add `com.apple.security.device.audio-input` (the mic, under hardened runtime). Accessibility needs no entitlement, only TCC.
- **Steps for a non-Tauri Rust app:**
  1. Run `cargo build --release`.
  2. Assemble the `.app`.
  3. Sign nested code inside-out, then the app, with `codesign --force --timestamp --options runtime --entitlements EasyVox.entitlements -s "Developer ID Application: …"`.
  4. Build a DMG and sign it.
  5. Run `xcrun notarytool submit --keychain-profile … --wait` (in CI: an App Store Connect API key).
  6. Run `xcrun stapler staple`, then `spctl -a -vv`.
  GPUI Kit's [packaging guide](https://gpui-kit.com/docs/packaging) documents exactly this sequence.
- **Tooling.** **cargo-packager 0.11.8** automates signing and notarization from environment variables. It merges a custom `info_plist_path`, and `background_app` sets LSUIElement. cargo-bundle only appends raw plist XML and doesn't notarize. Either would replace `scripts/bundle.sh`.
- **Updates:**
  - **cargo-packager-updater:** signed JSON manifest, replaces the whole `.app`. It's the simplest to wire up but has no UI.
  - **Sparkle 2.10:** embed the framework, set `SUFeedURL`/`SUPublicEDKey`, and write a small objc2 shim for `SPUStandardUpdaterController`. It has the native UI and delta updates.
  - **Avoid `self_update`:** it replaces only `Contents/MacOS/<bin>`, which breaks the bundle seal and notarization.
  - **Homebrew:** a cask is a good second channel. Homebrew 5 is phasing out unsigned casks.

## Hands-on checklist

The full list is in `gpui-kit/MANUAL_TESTS.md`. Launch with:

```sh
cd ~/Development/easyvox-spikes/gpui-kit && scripts/bundle.sh && open dist/EasyVox.app
```

1. Grant **Accessibility** to `dist/EasyVox.app` (Privacy & Security → Accessibility → +). For step 8 only, also grant **Input Monitoring** to the same app. Each rebuild re-signs ad hoc, so you may need to remove and re-add it.
2. A menu bar mic icon appears and there is no Dock icon. The menu has Test dictation (WAV replay), Test with microphone…, Settings…, History… and Quit.
   - **End-to-end microphone test (phase 2).** Open TextEdit and click into it. Menu → **Test with microphone…** → allow the Microphone prompt for EasyVox → speak for 6 s. Expect first words about 1.3 s after you start, updates about every 0.5 s, and the final text about 0.2 s after it stops. The text goes on the clipboard, is pasted if Accessibility is granted, and lands at the top of History with the model id. Then do the same with ⌥Space held and released. Check that Input → "Jump Desktop Microphone" actually carries your voice, or pick the Mac's mic.
   - On the very first launch the model needs about 14 s (shader compile); the pill says "Model still loading…".
3. Click into TextEdit and hold ⌥Space. The pill appears bottom-centre and streams text, with the provisional tail dimmed. TextEdit stays active: caret blinking, menu bar still says TextEdit. Clicks on the pill pass through to the app behind it.
4. Release. With AX granted, the text pastes into TextEdit and the pill hides. Without it, the yellow notice appears and ⌘V pastes manually.
5. Repeat over a full-screen app and on another Space.
6. Open History…: it comes to the front. Scroll quickly through about 200 rows. Click a copy icon, then ⌘V.
7. Open Settings…: it comes to the front. Use the model dropdown. Click the shortcut box and press ⌃⌥D; check that ⌥Space stops working and ⌃⌥D works; then set it back.
8. With Input Monitoring granted and the app relaunched, toggle "Hold right ⌥ instead" and hold right ⌥.
9. In Activity Monitor: idle CPU about 0.2 % with about 200 idle wake-ups/s (ggml-metal) and about 650 MB memory with the model loaded. Note CPU while holding ⌥Space for 10 s (about 10 % expected), and with Settings open but untouched.
10. Menu → Quit with the model loaded: no "quit unexpectedly" dialog, and no new `easyvox-gpui-*.ips` in `~/Library/Logs/DiagnosticReports`.

## Versions used

- **Toolchain:** macOS 26.6.2 (Apple M4), Xcode 27.0, rustc 1.99.0
- **UI framework:** gpui-kit 0.7.1 (gpui-component 0.7.1, gpui-base 0.7.1), gpui-pre 0.3.8 (gpui-pre-macos 0.3.8)
- **Tray and shortcuts:** tray-icon 0.26.1 (`default-features = false`), muda 0.21.2, global-hotkey 0.8.0 (keyboard-types 0.7.0)
- **Other crates:** objc2 0.6.5, objc2-app-kit 0.3.2, objc2-foundation 0.3.2, async-channel 2.5.0, raw-window-handle 0.6.2
- **Engine (phase 2):** `easyvox-core` 0.1.0 from `../core/easyvox-core`, features `transcribe-cpp` + `mic`. It brings transcribe-cpp / transcribe-cpp-sys 0.3.1 (ggml 0.25.3, Metal), cpal 0.18.2, rusqlite 0.40 (bundled SQLite) and hound 3.5. Model: `parakeet-unified-en-0.6b-q5_k_m-gguf` (`EngineConfig::recommended`, `final_redecode`), from `easyvox-spikes/models`. The UI additionally uses `RECOMMENDED_MODEL`, `catalog()`/`Runtime`, `History`, `mic_permission_status()` and `request_mic_permission()`.
- **Engine (phase 1):** a stub with the same API (`gpui-kit/easyvox-core-stub`, package name `easyvox-core`); kept for reference, not built.
