# Tauri v2 report

## Summary

**Phase 2 (real core).** The app now runs the real `easyvox-core`. Parakeet Unified runs on transcribe.cpp/Metal, built without sherpa-onnx, and the full press → partials → release → Final → clipboard → history flow is verified end to end with a WAV replay:

- first words in the overlay 1.35 s after press;
- partials about every 0.56 s;
- release → Final in the overlay 0.19–0.21 s, with the clipboard holding the text 1 ms later;
- 0 % WER on the 11 s clip.

Costs and friction:

- **Integration was smooth.** It took one Cargo line plus ~250 lines of app changes: mic-permission UX, a SQLite history, a model list from the catalog, and a session timeline for the self-test. Nothing in `../core` was touched and no core bug needed a workaround.
- **With the model loaded, idle memory is dominated by the model**: ~666 MB summed footprint, of which the WebKit and audio helper processes are ~32 MB.
- **The core's GPU runtime has an idle cost.** A ggml-metal residency heartbeat costs ~180–215 wakeups/s and ~0.2–0.3 % CPU forever, until the model unloads (the setting that turns this off is in the integration section).
- **First launch is slow once.** The first launch per app identity compiles ggml's Metal shaders (~15 s); after that the model loads in ~0.17 s.
- **Overlay level.** The overlay now uses level 101, the same as GPUI's PopUp.

Details are in "Integration with easyvox-core" below. Other sections describe phase 1 (stub core) unless they say otherwise.

**Phase 1 (stub core).** Tauri v2.12 covers almost the whole EasyVox UI shell with built-in APIs and maintained plugins. Tray, accessory mode, global press/release shortcut with runtime rebinding, windows, events, bundling and ad-hoc signing all needed no native code of our own. The critical overlay works and verifies cleanly from code:

- it's an `NSPanel` subclass with the NonactivatingPanel bit, level 25, all Spaces + full-screen auxiliary, can't become key or main, and is click-through;
- `NSApp.isActive` stays false and the frontmost app is unchanged while it streams;
- every DOM read-back matched the text Rust sent (39–40 updates per run), with ~2 ms Rust→DOM latency;
- WebKit snapshots show the rendered pill.

It gets there through `tauri-nspanel`, which isa-swizzles Tao's window into an NSPanel and calls a private AppKit selector. A hand-rolled alternative (no swizzle, ~20 lines of objc2) gives the same observable state minus the nonactivating bit.

The biggest costs are memory and the web-view process model:

- At idle the app is **5 processes, ~169 MB summed RSS, ~51 MB summed phys_footprint**.
- Streaming the overlay at 10 Hz costs **~5 % of one core** and lifts footprint to ~106 MB (mostly WebKit's GPU process).
- Each extra window adds a WebContent process (+35–46 MB RSS), which goes away when the window closes.

Surprises:

- ⌥Space registers fine on macOS 26.6.2 (the Sequoia Option-only restriction didn't bite).
- On macOS 26 the status item is hosted by Control Center, not our process.
- `CGEventTapCreate` succeeded without Input Monitoring.
- With the screen locked WebKit stops producing frames (requestAnimationFrame never fires), so on-screen smoothness, real focus behaviour and "comes to front" all still need hands-on checks.

## Feature results

| Feature | Status | Hackiness (0–4) | How (API / crate / shim) | Evidence |
|---|---|---|---|---|
| Accessory app (no Dock icon) | verified | 0 | `app.set_activation_policy(Accessory)` + `LSUIElement` in `src-tauri/Info.plist` (Tauri merges it) | `activation_policy: 1` in every `app_*` block of `tauri-selftest.json`; `LSUIElement => true` in the bundle's Info.plist |
| F1 Menu bar icon + menu | verified (menu clicks: needs hands-on) | 0 | `TrayIconBuilder` + `Menu`/`MenuItem` (feature `tray-icon`), template PNG via `include_image!` | `1_tray`: Tauri handle exists, `rect` 34×30 at y=0; `NSStatusItem.isVisible: true`, button window `NSStatusBarWindow` visible, level 25. On macOS 26 the item is drawn by a **Control Center**-owned layer-25 window at about the same x (within a few points); our pid owns no status-layer CG window (`cg_status_layer_windows_in_menu_bar_band_any_owner`) |
| F2 Global PTT shortcut (press + release) | registration + handler verified; real key delivery needs hands-on | 1 | `tauri-plugin-global-shortcut` 2.4.0 (global-hotkey 0.8, Carbon `RegisterEventHotKey`), `ShortcutState::Pressed/Released` | `2_shortcut`: `Alt+Space` registered at startup; probes for ⌥⇧Space, ⌃⌥Space, ⌃Space, ⌘⇧Space, ⌥K, F13 all register; handler driven directly: press → session, release → Final |
| F2 Runtime rebinding + recorder | verified (synthetic key events) | 1 | unregister/register through the plugin; JS recorder on `keydown` (`e.code` + modifiers → `Control+Alt+KeyJ`) | `rebind`: new registered, old not; `settings_page.recorder_synthetic_ctrl_alt_j`: state updated, registered, label `⌃⌥J` |
| F2 stretch: hold right ⌥ (CGEventTap) | permission check + failure path verified; delivery needs hands-on | 2 | own listen-only `CGEventTap` on flagsChanged, keycode 61 (`modtap.rs`) | `2b_modifier_only_tap`: `CGPreflightListenEventAccess() = false` → clear message. Run 1 (`tauri-selftest-run1-nspanel.json`): `CGEventTapCreate` returned **non-NULL** without permission |
| F3 Overlay: borderless, transparent, rounded pill, bottom center | verified | 1 | `WebviewWindowBuilder` (`decorations(false)`, `transparent(true)` + `macOSPrivateApi`), CSS pill; position via Tauri `cursor_position` / `monitor_from_point` / `work_area` | `native_after_show.frame_cocoa` 560×56 at bottom center of work area; `is_opaque: false`; `reports/tauri-overlay-nspanel.png` |
| F3 Above all windows, all Spaces, full screen | properties verified; behaviour over full-screen apps needs hands-on | 1 | tauri-nspanel 2.1.1: level `PopUpMenu` (101, phase 2; was `Status` 25), `CanJoinAllSpaces \| FullScreenAuxiliary \| Stationary \| IgnoresCycle` | phase 2: `level: 101`; `collection_behavior_flags` as listed, `is_on_active_space: true`, CG `is_onscreen: true`. Level survey in the integration section |
| F3 Must not take focus / activate | verified from code (as far as a locked screen allows) | 1 (plugin) / 2 (hand-rolled) | NSPanel subclass + `NonactivatingPanel` + `canBecomeKey/Main = NO`, shown with `orderFrontRegardless` | `class: EasyVoxOverlayPanel` (chain `NSPanel → NSWindow`), `nonactivating_panel_bit: true`, `can_become_key_window: false`, `is_key_window: false`, `NSApp.isActive` false→false, `key_window_number: null`, frontmost `loginwindow`→`loginwindow` |
| F3 Click-through | property verified; real clicks need hands-on | 0 | `set_ignore_cursor_events(true)` (Tauri API) | `ignores_mouse_events: true` |
| F3 Confirmed normal, provisional dimmed | verified | 0 | two `<span>`s, provisional in a 45 %-alpha colour | snapshot PNG: bright text + dim last two words |
| F3 Updates ≥10 Hz from a Rust thread (phase 1, stub) | DOM verified; on-screen frame pacing needs hands-on | 0 | stub thread → bridge thread → `app.emit_to("overlay", …)` → `listen()` | `render`: 40/40 DOM read-backs match, emit→DOM ack p50 1.6–2.0 ms, p95 2.1–2.4 ms, max ≤2.8 ms; ~9.4 Hz (stub pacing: 100 ms sleep + ~4 ms slack, also seen Rust-side). `raf_acks: 0` because the locked screen produces no frames |
| F3 Hides after final | verified | 0 | bridge thread → `orderOut:` after 0.6 s (1.6 s when showing the paste hint) | `overlay_hidden_after_final: true`, ~1.6 s |
| F4 Settings + History open from menu | windows verified; "comes to front" needs hands-on | 0 | `WebviewWindowBuilder` on demand + `set_focus()` (Tao: `makeKeyAndOrderFront` + `activateIgnoringOtherApps`) | both `is_visible: true`, CG `is_onscreen: true`, page ready in 76–127 ms; but `is_key_window: false`, `nsapp_is_active: false`, which is expected with loginwindow frontmost on a locked screen |
| F4 History: ~200 rows with Copy, smooth scroll | rows + Copy verified; smoothness needs hands-on | 1 (Copy via clipboard plugin) | plain DOM list; Copy uses `@tauri-apps/plugin-clipboard-manager` `writeText` | 202 rows, build+layout 20 ms, forced-layout scroll step avg 0.02 ms / max 1 ms; Copy button clicked via `eval` → clipboard read back matches. `rafFrames: 0`, `visibilityState: hidden` (locked) |
| F4 Model picker | verified | 0 | `<select>` with 5 fake models → `set_model` command | `modelOptions: 5`, `tauri-settings.png` |
| F5 Clipboard + ⌘V + AX check | clipboard + error verified; real paste needs hands-on | 2 (in core) / 0 (in the UI shell) | `easyvox_core::paste_text` (stub: NSPasteboard + `CGEventPost` ⌘V + `AXIsProcessTrusted`); UI banner polls `AXIsProcessTrusted` | `5_paste`: `AXIsProcessTrusted: false`, clipboard written and left, release-flow clipboard = final text, error text recorded; Settings banner text in `settings_page.initial_report`, visible in `tauri-settings.png` |
| F6 Engine integration (phase 2: real easyvox-core) | verified with WAV replay; microphone needs hands-on | 0 | press → `Engine::start(WavReplay \| Microphone)` + overlay; bridge thread drains the `Receiver` and owns the `Session` (its blocking drop never runs on the UI thread); release → `stop()` → Final → `paste_text` → `History::add` (SQLite) → `history-added` → hide | `3_overlay_end_to_end.session`: 18/18 partials shown, first at 1.35 s, release → Final 0.19–0.21 s, clipboard = final text, WER 0, history row matches; `microphone_session_starts: 0` |
| F7 `.app` with mic string + LSUIElement, ad-hoc signed | verified | 0 | `tauri build --bundles app`, `signingIdentity: "-"`, `hardenedRuntime`, `Entitlements.plist` (audio-input) | `codesign -dv`: `flags=0x10002(adhoc,runtime)`, entitlement present, `--verify --deep --strict` OK; 7.84 MiB with the stub, **14.40 MiB with the real core** (31.82 MiB if sherpa-onnx is kept) |
| Quit with the model loaded | verified | 0 | menu Quit = `app.exit(0)`; easyvox-core's `atexit` hook frees the ggml model | `scripts/quit-check.sh`: exit code 0 (direct exec) and no new `DiagnosticReports` entry (direct and `open`); same in every self-test run |
| F7 Developer ID + notarization | researched (not run) | 0 | Tauri bundler + env vars (see below) | — |

Two automated runs are kept: `reports/tauri-selftest.json` (tauri-nspanel overlay, the default) and `reports/tauri-selftest-plain.json` (hand-rolled overlay). Both pass the same checks.

These files now hold the **phase-2** runs (real core; the overlay block is `3_overlay_end_to_end`). The phase-1 stub-era rows above cite keys from those runs as they were then. The surviving phase-1 JSON evidence is `reports/tauri-selftest-run1-nspanel.json` (first stub run, same keys) and `reports/tauri-metrics.json`.

## Integration with easyvox-core

### What changed

- **Dependency.** `src-tauri/Cargo.toml` now has `easyvox-core = { path = "../../core/easyvox-core", default-features = false, features = ["transcribe-cpp", "mic"] }`.
  - The README allows dropping sherpa-onnx, so the build carries only transcribe.cpp (ggml + Metal), cpal and the core.
  - `.app` size: **14.40 MiB without sherpa-onnx, 31.82 MiB with it** (binary 15.0 MB vs 33.3 MB; zipped 5.66 MB).
  - No `SHERPA_ONNX_ARCHIVE_DIR` is needed. It was set only for the one comparison build.
- **Model.** `EngineConfig::recommended(MODELS_DIR)`, i.e. Parakeet Unified EN 0.6B Q5_K_M with `final_redecode`.
  - It loads on a background thread at startup. A press before the model is ready shows "Model is still loading…".
  - The Settings dropdown is now the core's `catalog()`. sherpa-onnx models show as "not in this build"; models whose files aren't downloaded show as disabled.
  - Switching models reloads off the main thread, and the old engine keeps serving until the new one is ready. The self-test does not exercise this.
- **Sessions** (`src-tauri/src/lib.rs`, `ptt_with` / `bridge`):
  - The bridge thread owns an `Arc<Session>`, so `Session`'s blocking `Drop` (it joins the worker) never runs on the main thread. A release that holds the last reference hands it to a throwaway thread.
  - A WAV replay ends by itself. On `Final` the bridge clears the active session, so the next press starts fresh even if no release arrived.
  - The `Final` text replaces the partials in the pill, as the core README asks.
- **Microphone.**
  - Push-to-talk uses `AudioSource::Microphone`. Automated modes (`--self-test`, `--bench-*`) are hard-wired to `WavReplay`; the self-test JSON records `microphone_session_starts: 0`.
  - A push-to-talk press first checks `mic_permission_status()`, which never prompts. If the status is NotDetermined, it shows "Allow microphone access…" and calls `request_mic_permission()` on a background thread; that call is user-initiated only.
  - The new menu item **"Test with microphone…"** asks for permission, then records 5 s.
  - **"Test dictation (sample audio)"** replays `cmd_samantha.wav`, so the pipeline can be checked without a microphone.
- **History.**
  - It uses the core's SQLite `History` at `~/Library/Application Support/EasyVox Tauri Spike/history.sqlite3`. The name is separate from the GPUI app's on purpose.
  - It is opened off the main thread and seeded with 200 placeholder rows once, when empty.
  - The `get_history` command became `async`.
- **Overlay level.** Changed from 25 to **101** (see below).
- **Untouched.** Nothing under `../core` was edited, and no core bug needed a workaround. The stub stays in `tauri/stub-core/` with the same API.

### End-to-end self-test (`reports/tauri-selftest.json`, `tauri-selftest-plain.json`)

Setup: `punct_samantha.wav` (11.0 s, 30 words), replayed in real time by the core. The press is simulated through the same `ptt()` entry point the shortcut handler uses. The release comes 50 ms before the clip ends, like a user letting go after speaking. The screen is locked, and AX is not granted.

| Measure | nspanel mode | plain mode | How measured |
|---|---|---|---|
| Model load at startup | 163–170 ms (warm Metal cache); **14.6 s on the very first launch** | 170–193 ms | `Engine::load_ms()`, `ENGINE_LOADED` line |
| Press → first partial | 1346 ms in Rust, **1348 ms in the overlay DOM** | 1351 / 1353 ms | bridge receive time; JS read-back ack of that event |
| Partial cadence as the overlay received it | p50 **563 ms**, p95 695 ms, 17 intervals, 18 partials, all shown | p50 ~560 ms | Rust-side ack arrival per partial |
| Release → `Final` event | **187–208 ms** (core `finalize_ms` 186) | 196–204 ms | |
| Release → final text in the overlay DOM | 188–209 ms | 197 ms | |
| Release → clipboard holds the final text | **188–209 ms** (+1 ms after Final) | 197 ms | polling `NSPasteboard` on the self-test thread every 1 ms |
| Final text vs ground truth | **WER 0 % (0/30)**; differs only in punctuation (`:` and `.` became `,`) | WER 0 % | normalized Levenshtein |
| History | row id 202 = latest row, text matches | same | `History::recent(1)` |
| Overlay while streaming | level 101, NonactivatingPanel, not key, `NSApp.isActive` false→false, frontmost unchanged; 19/19 DOM read-backs match; hides 1.6 s after Final | level 101, Tao window, same | |
| Rendering | `reports/tauri-overlay-nspanel.png` (mid-stream), `reports/tauri-overlay-final-nspanel.png` (green dot, final text) | `tauri-overlay-plain.png`, `tauri-overlay-final-plain.png` | WebKit `takeSnapshotWithConfiguration:` |
| Paste | AX false → core error "Accessibility permission is missing … The text is on the clipboard." | same | |
| Quit with the model loaded | exit code 0, no crash report | | `scripts/quit-check.sh` + `selftest.sh` |

A side test drove the shortcut handler directly on `cmd_samantha.wav`, releasing after 1.5 s. The Final was "Send a message to Sarah", 135 ms after release.

### Metrics with the model loaded

Source: `reports/tauri-metrics-core.json`, measured with load average 1.5–2.0, after waiting for a quiet machine.

| Metric | Value |
|---|---|
| Clean release build + bundle (271 crates, includes transcribe.cpp's cmake build) | **86.5 s** wall / 495 CPU-s (load 2.7 at start; our own build pushed it to 13.8) |
| Incremental release, touch one Rust file / one frontend file | **12.9–14.6 s** / **12.7 s** (same as phase 1; the C++ isn't rebuilt). A retry under load 12–24 took 38 s |
| `.app` size | **14.40 MiB** (binary 15.0 MB; zipped 5.66 MB). With sherpa-onnx: 31.82 MiB |
| Launch: `open` → tray ready / → model loaded | **169 ms / 333 ms** median of 5 (in-process main → loaded 244–253 ms; core load 162–165 ms) |
| First launch per app identity | ~14.6 s model load: ggml-metal compiles its shaders in `MTLCompilerService`, which briefly showed 870 MB RSS, and caches them in `$DARWIN_USER_CACHE_DIR/dev.easyvox.spike.tauri/com.apple.metal` (23 MB). Measured under load ~12, so treat it as an upper bound |

| State | Summed RSS | Summed phys_footprint | CPU (% of one core) | Wakeups/s |
|---|---|---|---|---|
| Idle, model loaded (5 processes) | **811 MB** (app 726, WebKit 72, audio helper 13) | **666 MB** (app 635, WebKit GPU 10, Networking 5.4, WebContent 11, audio helper 5.2) | **0.23 %** (app 0.21) | **218** (app 216) |
| Idle, model loaded, `GGML_METAL_NO_RESIDENCY=1` | 356 MB | 668 MB | 0.03 % | 3.5 |
| 30 s WAV session (`long_samantha.wav`, 50 partials, finalize 467 ms) | **838 MB** (max 842) | **781 MB** (app 699, WebKit GPU 63) | **9.9 %** (app 9.0, WebContent 0.5, GPU 0.3) | 558 |
| Phase 1 for comparison: idle (stub) / 10 Hz stub stream | 169 / 178 MB | 51 / 106 MB | 0.01 % / 5.0–5.3 % | 1.4 / 146–153 |

What this shows:

- **The model dominates memory.** The UI's share at idle is ~72 MB RSS / ~32 MB footprint for the three WebKit processes, plus Tauri's own part of the app process. For comparison, the stub-era app process had ~19 MB footprint.
- **The UI's CPU share of a real session is small.** The overlay costs ~0.8 % of one core (WebContent + GPU process) because partials arrive at ~2 Hz, not 10 Hz. The app's 9 % is mostly the streaming decode, which matches the core report's ~10 %.
- **Locked-screen caveat** (as in phase 1): WebKit paints no frames, so on-screen rendering cost is still to be measured unlocked.

### Friction and findings

1. **No API friction.** The stub had the contract's shape and the real crate kept it, so the first `cargo check` against the real crate passed.
   - Additions used: `EngineConfig::recommended`, `catalog()`, `History`, `mic_permission_status` / `request_mic_permission`, `Session::stats()`.
   - Core edition 2024 next to app edition 2021 is fine.
   - `objc2` 0.6 / `objc2-app-kit` 0.3 are shared with Tauri, so there's no duplicate AppKit bindings crate.
2. **ggml-metal residency heartbeat (core / ggml issue, affects any UI).** `ggml-metal-device.m` starts a GCD thread that runs `usleep(5 ms)` in an endless loop for as long as a model is loaded. It requests residency for the first 3 minutes after each decode and keeps waking after that. That is ~180–215 wakeups/s and ~0.2–0.3 % CPU at idle, against 1.4/s with the stub.
   - `GGML_METAL_NO_RESIDENCY=1` removes it (3.5 wakeups/s, same footprint, `reports/tauri-idle-residency-check.json`). Its effect on latency after long idle (memory being reclaimed) isn't measured, so the app doesn't set it.
   - The real fix is the planned "unload after idle", or a patched ggml that sleeps when `d_loop == 0`. It's flagged here and to the coordinator for the core agent.
3. **First-launch shader compile.** ggml-metal's Metal library is compiled at the first model load **per app bundle id**. The core's CLI benchmarks never saw it because their cache was warm.
   - Users will hit ~15 s once after install, and probably after macOS updates. The UI should show "Preparing the speech model…".
   - Our app does show "Model is still loading…" if the user presses early.
4. **Clipboard measurement gotcha.** `clearContents` and `setString:` are two steps, so a `changeCount` bump can be seen before the text lands. Another agent's self-test also wrote to the shared clipboard during one run. The probe now polls the string itself.
5. **Status quo kept.** Tauri needed nothing special for the C++/Metal dependency: `tauri build` and the bundler just linked it statically, with no `.dylib` to bundle, and ad-hoc signing and hardened runtime still validate.

### Overlay window level: 25 vs 101

Window levels on this Mac (macOS 26.6.2, `CGWindowLevelForKey` plus a `CGWindowListCopyWindowInfo` survey of the layers system processes actually use):

| Who | Level |
|---|---|
| Normal windows (and **full-screen app windows**, in their own Space) | 0 |
| Floating | 3 |
| UserNotificationCenter alerts | 8 |
| Dock | 20 |
| **Notification Center banners** | 21 |
| **Spotlight** | 23 |
| Menu bar (Window Server) | 24 |
| Status items (Control Center) | 25 |
| **Pop-up / context menus** | 101 |
| Overlay | 102 |
| Control Center panels | 2005 |
| loginwindow (lock screen) | 2001 / 2004 |
| Input-source switcher HUD | 2147483628 |

Findings:

- **Full-screen apps aren't a level question.** They sit at level 0 in their own Space. What lets the pill join that Space is `CanJoinAllSpaces | FullScreenAuxiliary` in an accessory app, which we set either way. So 25 already beat full-screen apps, the Dock, Notification Center banners, Spotlight and the menu bar.
- **What 25 lost to:** the target app's pop-up, context and autocomplete menus (101), and anything else at 25 ordered after us (other menu bar apps' panels).
- **Decision: switched to 101.** It matches GPUI's PopUp and is above every menu. It is still below Control Center panels, the lock screen and the input-source HUD, which is correct.
- **The change was trivial:** one line per mode, `PanelLevel::PopUpMenu.value()` in `overlay.rs` and `setLevel(101)` in `mac.rs`. The self-test confirms `level: 101` in both modes.
- **Still hands-on:** showing over an actual full-screen app (checklist step 7).

## Hackiness details

**F3 overlay with tauri-nspanel (rated 1, but read this).** Our code is about 25 lines of plugin calls: `src-tauri/src/overlay.rs:38-46` (the `tauri_panel!` class definition) and `overlay.rs:74-95` (level, style mask, collection behaviour, mouse, transparency).

Under the hood the plugin is the hack:

- **Class swap.** `from_window` calls `object_setClass` on Tao's live `TaoWindow` to turn it into our NSPanel subclass (tauri-nspanel `src/panel.rs:683-757`).
- **Private selector.** `set_style_mask` calls the private `_setPreventsActivation:`, because the class swap skips NSPanel's init (`panel.rs:632-641`).
- **Patched WebKit view.** It also `class_addMethod`s `windowLevel` onto the WKWebView subclass so input-method popups appear above the panel (`panel.rs:18-107`).

How fragile it is:

- Anything in Tao that later touches `TaoWindow` ivars on the swapped object (for example `set_focusable`, which writes the `focusable` ivar) would hit the wrong class. We never call those.
- A future AppKit change to how `_setPreventsActivation` works could break non-activation.

Mitigating factors:

- The plugin is now on crates.io (2.1.1, published 2026-10-08).
- It is maintained and used by Handy, Cap, screenpipe and others.
- It catches Objective-C exceptions from style-mask changes.

We also rely on two other details:

- **Transparency needs a private API.** Transparent windows require `app.macOSPrivateApi: true`, which uses WKWebView's private `drawsBackground` KVC key. Tauri documents the flag but it rules out the App Store.
- **Never use Tauri's `show()` on the overlay.** It calls `makeKeyAndOrderFront:` (tao `util/async.rs:211-217`). We show with `orderFrontRegardless` through the plugin's `show()`.

**F3 overlay hand-rolled (rated 2).** `src-tauri/src/mac.rs:51-73` is about 20 lines of typed objc2 on Tao's own window, with no class swap:

- `setLevel(25)` and the collection behaviour;
- `setIgnoresMouseEvents`, `setHidesOnDeactivate(false)` and `setHasShadow(false)`;
- `orderFrontRegardless` / `orderOut:`.

It relies on Tauri's `focusable(false)` (Tao overrides `canBecomeKeyWindow`/`canBecomeMainWindow` to return NO). All of it is documented API and not fragile.

What it can't do is set `NonactivatingPanel`, which only NSPanel honours. If the pill ever received a click, the app would activate. Click-through makes that moot for EasyVox. Whether a non-panel window shows over other apps' full-screen Spaces as reliably as a panel is the one open question (checklist step 7).

**F2 stretch: right-⌥ CGEventTap (rated 2).** `src-tauri/src/modtap.rs` is 137 lines, about 45 of them logic; the rest is CoreGraphics/CoreFoundation FFI declarations and constants.

- It is a listen-only `kCGSessionEventTap` for `flagsChanged`. Keycode 61 plus the `NX_DEVICERALTKEYMASK` bit gives press and release, and it re-enables itself after `kCGEventTapDisabledByTimeout`.
- It is documented API, not Tauri-specific (any Rust UI would need the same code), and not fragile.
- Finding: on macOS 26.6.2 `CGEventTapCreate` returned a valid tap with no Input Monitoring grant. Tap creation is therefore **not** a permission check; we use `CGPreflightListenEventAccess()`. Whether events are actually delivered without the grant is checklist step 12.

**F5 paste (rated 2, but in the core).** `stub-core/src/lib.rs:140-214` is about 50 lines: NSPasteboard write, `AXIsProcessTrusted`, then a `CGEventPost` of ⌘V with flags forced to Command only, so a still-held ⌥ doesn't turn it into ⌥⌘V. The real `easyvox-core` owns this, so it costs both UI stacks the same. The Tauri shell itself only adds a one-line `AXIsProcessTrusted` FFI (`mac.rs:36`) for the Settings banner.

**Optional: App Nap opt-out during sessions.** `src-tauri/src/lib.rs:306-340` is about 30 lines (`NSProcessInfo beginActivityWithOptions:` with UserInitiated + LatencyCritical, held until the paste completes). It's not required by any feature, and it barely changed the stub's timer slack: JS inter-arrival p50 was 107 ms without it (run 1) and 105 ms with it. It's worth keeping for the real engine so the final pass isn't throttled.

**Self-test tooling (not product code).** About 700 lines:

- `mac.rs:89-397`, which reads window, app, status-item, CG-window and process state;
- `selftest.rs`;
- a WKWebView `takeSnapshotWithConfiguration:` → PNG helper, which proves what WebKit paints even on a locked screen.

It uses two private, read-only diagnostics: `-[WKWebView _webProcessIdentifier]` and `-[NSWindow _preventsActivation]`.

## Metrics (phase 1, stub core)

Phase-2 numbers with the real model are in "Integration with easyvox-core" above.

Machine: M4, 16 GB, macOS 26.6.2, screen **locked**. Two other agents were building at the same time: load average was 2–15 during these measurements (noted per line).

Memory and CPU are summed over EasyVox plus every process macOS attributes to it (`responsibility_get_pid_responsible_for_pid`). That means the app, WebKit GPU, Networking and WebContent, and `com.apple.audio.SandboxHelper`. The app is launched through `open` so the attribution is right. CPU and wakeups come from `proc_pid_rusage`. Raw data is in `reports/tauri-metrics.json`; the script is `tauri/scripts/measure.py`.

**Build and size**

| Metric | Value |
|---|---|
| Clean release build + bundle (`npx tauri build --bundles app`, deps pre-downloaded, 252 crates) | **63.6 s** wall / 325 s CPU, under contention (load 8→15) |
| Incremental release, touch one Rust file | **14.6 s** (Rust 13.8 s; the app crate re-monomorphizes Tauri + links) |
| Incremental release, touch one frontend file | **13.7 s** (Vite 21 ms; assets are embedded at compile time, so the Rust crate rebuilds) |
| Debug: full build / touch one Rust file | 28.9 s / **1.2 s**. In `tauri dev`, frontend edits hot-reload through Vite without any Rust rebuild (not timed) |
| Size-tuned profile (`lto`, `opt-level="s"`, `codegen-units=1`, `panic=abort`): clean build | 61.4 s |
| `.app` size (release + `strip`) | **7.84 MiB** (binary 8.3 MB); zipped 3.05 MB |
| `.app` size (size-tuned profile) | **3.72 MiB** (binary 3.8 MB); zipped 1.74 MB |

**Launch and responsiveness**

| Metric | Value |
|---|---|
| Launch to tray-ready (`open` → first main-loop turn after the NSStatusItem exists) | **153 ms** median of runs 2–6; 431 ms on the first launch after a rebuild |
| …in-process main → tray ready / → overlay web view created | 83–94 ms / 131–165 ms |
| Overlay: Rust emit → JS DOM updated (round trip back to Rust) | p50 1.6–2.0 ms, p95 2.1–2.4 ms, max 2.8 ms |
| Settings / History: open → page ready | 76–127 ms |

**Memory and CPU**

| State | Processes | Summed RSS | Summed phys_footprint | CPU (% of one core) | Wakeups/s |
|---|---|---|---|---|---|
| Idle (overlay web view loaded, hidden) | 5 | **169 MB** | **51 MB** | **0.006–0.009 %** | 1.4 |
| Overlay streaming at 10 Hz | 5 | **178 MB** | **106 MB** | **5.0–5.3 %** | 146–153 |
| Streaming + one forced WebKit paint per update (upper bound) | 5 | 177 MB | 105 MB | 7.0 % | 169 |
| Settings + History open | 7 | 253 MB | 86 MB | 1.1 % | 48 |
| Idle after closing both | 5 | 187 MB | 58 MB | 0.02 % | 2.2 |

Memory and CPU notes:

- **Where the idle memory sits:** app 84 MB RSS / 19 MB footprint, WebKit.GPU 27 / 10, Networking 15 / 5.6, WebContent 31 / 11, audio.SandboxHelper 13 / 3.
- **Where the streaming CPU goes:** app 1.3 %, WebContent 2.5 %, GPU 1.6 %. The GPU process footprint rises from 10 MB to 64 MB while the pill is on screen.
- **Each extra window** adds its own WebContent process (35 and 46 MB RSS). Closing the window ends it.
- RSS double-counts shared framework pages; phys_footprint is the better "what it costs" number.
- **Locked-screen caveat.** The screen was locked, so WindowServer composited nothing visible and requestAnimationFrame never ran. The 10 Hz row therefore covers IPC, JS, style/layout and WebKit's compositing work, but not real display frames. The force-paint row (one `takeSnapshot` per update) bounds it at about 7 %. Checklist step 14 re-measures this unlocked.

## Developer experience

- **Docs and API quality.** v2 docs plus crate source were enough. Every API used is stable 2.x: tray, menus, windows, `emit_to`, `background_throttling`, `focusable`, `set_ignore_cursor_events`, activation policy, Info.plist merge, bundler signing.
- **API churn.** **Tauri 3.0.0-alpha.4 is already on crates.io** (Sept–Oct 2026), so expect migration work within the year. Pin `tauri = "2.12"`.
- **Plugin packaging.** `tauri-nspanel` moved from git-only to crates.io a day before this spike.
- **The "write Rust once, compile slowly" tax is real but moderate.** A 14 s release incremental is dominated by the app crate, where `generate_context!` embeds the frontend and all the Tauri generics instantiate. Debug incremental is ~1 s. The web frontend gets Vite HMR in `tauri dev`.
- **Things that bit us** (all found by reading source, not docs):
  1. The global-shortcut plugin calls your handler **while holding its own shortcut-table mutex**. Calling `register`/`unregister` from the handler would deadlock.
  2. A shortcut recorder must **unregister the live hotkey while recording**. Otherwise Carbon swallows the combo before the web view sees it.
  3. Tauri's `WebviewWindow::show()` is `makeKeyAndOrderFront:`. It's wrong for a non-activating overlay, and nothing in the docs says so.
  4. Sync `#[tauri::command]`s run on the main thread. Rebinding goes through the plugin's own main-thread hop, so we used `#[tauri::command(async)]`.
  5. WebKit throttles web views in hidden or occluded windows by default. We set `background_throttling(Disabled)` (`WKPreferences.inactiveSchedulingPolicy = none`) on the overlay so the first update after show isn't delayed. This was not A/B-tested here.
  6. CSS `backdrop-filter` can't blur the desktop behind a transparent web view. Use Tauri's window effects (NSVisualEffectView) for frosted glass.
- **Measurement friction.** Memory is split across XPC processes that `ps` lists with ppid 1. Attributing them needs `responsibility_get_pid_responsible_for_pid`, and needs the app launched through LaunchServices; if you launch from a shell, the shell's terminal becomes "responsible".
- **Error messages.** Tauri's own were fine. Cargo reported a feature-name typo in `objc2-foundation` as a `tauri` version conflict, which was misleading but not Tauri's fault.
- **Frontend.** Plain TypeScript + Vite 8.3 (Rolldown), 3 pages, ~300 lines total. No framework, because the UI is tiny and the overlay page should stay light (JS 0.7 kB + 1.8 kB shared). Vite strips types without type-checking.

## Linux outlook

Desk research only; a sub-agent checked sources on 2026-10-09.

- **Tray:**
  - Works through libayatana-appindicator. Vanilla GNOME needs the [AppIndicator extension](https://extensions.gnome.org/extension/615/appindicator-support/).
  - Tray click events aren't emitted on Linux, only the menu ([Tauri tray docs](https://v2.tauri.app/learn/system-tray/)).
- **Push-to-talk:**
  - `tauri-plugin-global-shortcut` / global-hotkey is **X11 only**, with release events ([global-hotkey README](https://github.com/tauri-apps/global-hotkey), [#28](https://github.com/tauri-apps/global-hotkey/issues/28)).
  - On Wayland the XDG [GlobalShortcuts portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html) has `Activated`/`Deactivated` (press and release). It's implemented on KDE, Hyprland and GNOME 48+, but not wlroots/Sway, and buggy on GNOME 49 for non-Flatpak apps ([Handy#1236](https://github.com/cjpais/Handy/issues/1236)).
  - The alternative is evdev, which needs the `input` group.
  - Handy offers the plugin, evdev ([handy-keys](https://github.com/handy-computer/handy-keys)) and SIGUSR2; its portal PR [#1287](https://github.com/cjpais/Handy/pull/1287) is still open.
- **Overlay:**
  - A Tauri window can become a wlr-layer-shell surface through `gtk_window()` + gtk-layer-shell before `show()` ([tauri#15913](https://github.com/tauri-apps/tauri/issues/15913)). Handy does this ([overlay.rs](https://github.com/cjpais/Handy/blob/main/src-tauri/src/overlay.rs)).
  - It's GTK3 only, layer shell isn't supported on GNOME/Mutter or X11, and the AppImage `GDK_BACKEND=x11` trap ([#15781](https://github.com/tauri-apps/tauri/issues/15781)) silently disables it.
  - Handy ships its overlay **off by default on Linux** because some compositors give it focus and paste breaks.
  - Click-through (`set_ignore_cursor_events`) is implemented on Linux through an input shape.
- **Paste:**
  - X11 uses xdotool / XTest.
  - On Wayland it's per compositor: wtype doesn't work on KDE or GNOME; dotool and ydotool need uinput. The RemoteDesktop portal shows a consent dialog. Handy chains these ([clipboard.rs](https://github.com/cjpais/Handy/blob/main/src-tauri/src/clipboard.rs)).
- **Clipboard:** the clipboard plugin uses arboard with `wayland-data-control`. Mutter lacks it, so on GNOME it falls back to X11.
- **WebKitGTK:** memory growth from high-rate IPC to the overlay ([Handy#1279](https://github.com/cjpais/Handy/issues/1279)), so throttle overlay events. There are also NVIDIA, DMA-BUF and transparency issues ([#9304](https://github.com/tauri-apps/tauri/issues/9304), [#10702](https://github.com/tauri-apps/tauri/issues/10702)).
- **Verdict:** X11 is workable today. On Wayland, push-to-talk, the overlay and paste each need per-desktop work. That holds for any toolkit, but Tauri adds WebKitGTK's quirks.

## Developer ID signing and notarization (research, not run)

**The user's "Apple Distribution" certificate can't be used.** It's for App Store / TestFlight submission. The notary service rejects anything not signed with **Developer ID Application** ([Apple: certificates](https://developer.apple.com/help/account/certificates/certificates-overview/), [notarizing](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)).

**Steps:**

1. **Create the certificate.** Only the **Account Holder** can create a Developer ID certificate ([Apple](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/)); for an individual membership that's the user. Make a CSR with Keychain Access → Certificate Assistant → Request a Certificate From a Certificate Authority (saved to disk). Then go to developer.apple.com → Certificates → + → **Developer ID Application**, upload the CSR, download the `.cer` and double-click it.
2. **Check it installed.** `security find-identity -v -p codesigning` should list `Developer ID Application: Name (TEAMID)`.
3. **Configure `src-tauri/tauri.conf.json` → `bundle.macOS`.** Keep `hardenedRuntime: true` and `entitlements: "Entitlements.plist"`. The plist already has `com.apple.security.device.audio-input`, which is all the hardened runtime needs. Accessibility and Input Monitoring are TCC grants with no entitlement, and no JIT entitlement is needed because WebKit's JIT runs in its own process (Handy ships the same way). Set `signingIdentity` to the identity string, or leave it and use `APPLE_SIGNING_IDENTITY`.
4. **Provide notarization credentials** through environment variables Tauri reads ([ENVIRONMENT_VARIABLES.md](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-cli/ENVIRONMENT_VARIABLES.md)). Use **either**:
   - an App Store Connect API key: `APPLE_API_ISSUER`, `APPLE_API_KEY` (the key **ID**) and `APPLE_API_KEY_PATH` (the `.p8`); **or**
   - an Apple ID: `APPLE_ID`, `APPLE_PASSWORD` (app-specific password) and `APPLE_TEAM_ID`.
5. **Build.** Run `npx tauri build --bundles app,dmg`. Tauri signs inside-out with `--options runtime`, zips with `ditto`, runs `notarytool submit --wait` and **staples the .app**.
6. **The DMG isn't notarized.** Tauri signs it but doesn't notarize or staple it ([#7533](https://github.com/tauri-apps/tauri/issues/7533)). Afterwards run `xcrun notarytool submit EasyVox_*.dmg --key … --key-id … --issuer … --wait && xcrun stapler staple EasyVox_*.dmg`.
7. **Verify:**
   - `codesign --verify --deep --strict --verbose=2 EasyVox.app`
   - `spctl -a -vvv -t exec EasyVox.app` should print `source=Notarized Developer ID`
   - `spctl -a -t open --context context:primary-signature -v EasyVox.dmg`
   - `xcrun stapler validate` on both

   (`-t install` is for .pkg files.)
8. **CI.** Use `tauri-apps/tauri-action@v1` on `macos-latest`. Store `APPLE_CERTIFICATE` (base64 .p12) and `APPLE_CERTIFICATE_PASSWORD` as secrets, plus the API-key secrets, and add a final DMG notarize + staple step. Grep for "Developer ID Application" when picking the identity; Tauri's docs example greps "Apple Development".

TCC grants are tied to the signature, so switching from ad-hoc to Developer ID resets the user's Accessibility and Input Monitoring grants once.

## Hands-on checklist

The full version is in `tauri/MANUAL_TESTS.md`. Launch with:

```sh
open /Users/agentbox/Development/easyvox-spikes/tauri/src-tauri/target/release/bundle/macos/EasyVox.app
```

1. Check the menu bar icon is there, with no Dock or ⌘Tab entry. The menu has Test dictation (sample audio), Test with microphone…, Settings…, History… and Quit.
2. In TextEdit, hold ⌥Space and speak. The first press asks for Microphone access; allow it and press again. The pill streams your words, and TextEdit stays the active app (title bar and menu bar).
3. Release. The pill says "Copied — press ⌘V…". ⌘V pastes the text.
4. Grant **Accessibility** to `EasyVox.app` (System Settings → Privacy & Security → Accessibility), then relaunch.
5. Dictate again. It auto-pastes, including when ⌥ is still held as Space is released.
6. Click on the pill while dictating. The click goes through, and EasyVox doesn't activate.
7. Repeat over a full-screen app and on another Space. Then repeat with `open --env EASYVOX_OVERLAY=plain …` to compare plugin vs. hand-rolled.
8. With TextEdit active, open Settings… and History… from the menu. Each comes to the front and is focused.
9. Use the recorder to bind ⌃⌥K. It works at once; Esc cancels. Rebind ⌥Space.
10. Scroll History quickly and check it's smooth. Copy on a row pastes that row.
11. Tick "Hold right ⌥". First hold right ⌥ **without** granting **Input Monitoring** and note whether it fires. Then grant it to EasyVox, relaunch, and check hold-to-talk with right ⌥.
12. Unlocked, run `scripts/selftest.sh` and `scripts/measure.py`. Check that `raf_acks > 0`, see whether Settings `is_key_window` is now true, and compare session CPU.
13. **End-to-end with the microphone (phase 2).**
    - Pick the input device in System Settings → Sound → Input (the Mac's microphone, or Jump Desktop Microphone with audio redirection on).
    - Choose menu → **Test with microphone…** and Allow the "EasyVox would like to access the microphone" prompt. If no prompt appears, enable EasyVox under Privacy & Security → **Microphone**.
    - Choose the item again and speak for 5 s. The first words should appear about 1.3 s in, and the final text should paste into TextEdit.
    - Then dictate with ⌥Space. Note how quickly the first words appear, how accurate the text is, and how long from release to paste (0.2 s measured for an 11 s clip). Do one dictation longer than 30 s.
    - "The microphone is delivering digital silence" means the permission or the input device is wrong.
14. **Quit with the model loaded.** Choose menu → Quit EasyVox. There must be no "quit unexpectedly" dialog and no new `easyvox*.ips` file in `~/Library/Logs/DiagnosticReports`.

Every rebuild changes the ad-hoc signature, so re-add EasyVox to the permission lists after rebuilding. If a stale "receive keystrokes" prompt from the first self-test is waiting, click Deny.

## Swapping in the real core

Done in phase 2 (see "Integration with easyvox-core"). To go back to the stub, point `easyvox-core` in `src-tauri/Cargo.toml` at `../stub-core`. Its `paste_text`, `Engine`, `Session` and `EngineConfig::new` match the real API, but the phase-2 glue also uses `History`, `catalog()` and the mic-permission functions, so those would need stubbing too.

## Versions used

| Component | Version |
|---|---|
| Rust | 1.99.0 (edition 2021 for the app) |
| Node / npm | 24.21.0 / 11.19.0 |
| Xcode | 27.0 |
| macOS | 26.6.2 (25G83), Apple M4 |
| tauri | 2.12.2 |
| tauri-build | 2.7.1 |
| tauri-runtime-wry / wry / tao | 2.12.1 / 0.57.0 / 0.37.1 |
| tray-icon / muda | 0.25.1 / 0.20.0 |
| tauri-cli (npm) | 2.12.1 |
| @tauri-apps/api | 2.12.2 |
| tauri-plugin-global-shortcut (global-hotkey) | 2.4.0 (0.8.0) |
| tauri-plugin-clipboard-manager (arboard), Rust and JS | 2.4.1 (3.6.1) |
| tauri-nspanel | 2.1.1 (crates.io) |
| objc2 / objc2-app-kit / objc2-foundation / block2 | 0.6.5 / 0.3.2 / 0.3.2 / 0.6.2 |
| Vite | 8.3.4 (no framework, plain TS) |
| easyvox-core (phase 2) | 0.1.0, path `../../core/easyvox-core`, features `transcribe-cpp` + `mic` |
| transcribe-cpp / -sys (bundled ggml) | 0.3.1 (ggml 0.25.3, Metal) |
| cpal / rusqlite / hound / objc2-av-foundation | 0.18.2 / 0.40.2 / 3.5.1 / 0.3.2 |
| Model | `parakeet-unified-en-0.6b-q5_k_m-gguf` (541 MB) from `easyvox-spikes/models` |

Evidence files:

- phase 2: `reports/tauri-selftest.json` and `reports/tauri-selftest-plain.json` (end-to-end, real core), `reports/tauri-metrics-core.json`, `reports/tauri-idle-residency-check.json`, `reports/tauri-overlay-final-nspanel.png`, `reports/tauri-overlay-final-plain.png`, `tauri/logs/build-clean-core.log`, `tauri/scripts/quit-check.sh`

- `reports/tauri-selftest.json` (tauri-nspanel mode) and `reports/tauri-selftest-plain.json` (hand-rolled mode)
- `reports/tauri-selftest-run1-nspanel.json`, the run that tried the event tap without permission
- `reports/tauri-metrics.json`
- WebKit snapshots: `reports/tauri-overlay-nspanel.png`, `reports/tauri-overlay-plain.png`, `reports/tauri-settings.png`, `reports/tauri-history.png`
- clean build log: `tauri/logs/build-clean-1.log`
