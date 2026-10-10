# Onboarding: Microphone and Accessibility, default model download, shader warm-up

`tuh-01M4H2EG9QB2ETECD30F9RBR58`

- **Status:** open — ready
- **Priority:** 1
- **Labels:** `onboarding` `app`
- **Depends on:** [`tuh-x7tv`](tuh-01M4H2EG8YM9TXGK81DC6WX7TV.md) (done)
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: DESIGN.md "Next steps" 2 and "Default: Parakeet Unified". The first launch has to:
- ask for Microphone (opening the mic before the permission has been asked hangs with no error; the core refuses until then);
- ask for Accessibility (needed for the ⌘V paste);
- download, verify and activate the default model (Parakeet Unified, 541 MB; the first run needs a network connection);
- warm up ggml's Metal shaders: the first model load under a new bundle id compiles them (~14 s), later loads take ~0.2 s.

The ask: a first-launch onboarding window in the GPUI Kit shell that walks through these steps with progress and clear failure states (no network, permission denied, checksum mismatch → retry), loads the model once after the download so the shader compile happens there rather than on the first dictation, and remembers completion. Settings can reopen any step that's missing later (e.g. the model was deleted).

Acceptance:
- With a fresh data folder (EASYVOX_DATA_DIR pointing at an empty folder) and reset grants (`tccutil reset Microphone dev.easyvox.EasyVox`, same for Accessibility), launch shows onboarding; finishing it leaves the default model verified and loaded and both permissions granted; the first dictation starts without the 14 s stall.
- A relaunch skips onboarding.
- The step logic is unit-tested; file a human-labelled held task for the hands-on pass at an unlocked screen.

Pointers: crates/easyvox-app/src/settings.rs (permission rows, request_mic_permission), controller.rs, main.rs, platform/macos.rs; easyvox_core::{mic_permission_status, request_mic_permission}; the in-app download API this task depends on.

Constraints: the bundle id dev.easyvox.EasyVox must not change; never open the mic before the permission has been asked.

## History

_No activity yet._
