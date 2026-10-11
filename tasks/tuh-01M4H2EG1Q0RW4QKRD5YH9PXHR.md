# Hands-on: hold right ⌥ as the shortcut (Input Monitoring, relaunch)

`tuh-01M4H2EG1Q0RW4QKRD5YH9PXHR`

- **Status:** done
- **Priority:** 1
- **Labels:** `human` `hands-on`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Brandon's task; agents release it. Mark it done when finished, and file a task for anything that fails.

Setup: `scripts/bundle.sh && open dist/EasyVox.app` at an unlocked screen. The ad-hoc signature changes on every rebuild, so grant Accessibility (and Microphone) again after each one.

- Turn on the right-⌥ hold setting in Settings and grant Input Monitoring (System Settings → Privacy & Security → Input Monitoring), then relaunch: the grant only applies after a relaunch.
- Hold right ⌥, speak, release: same flow as ⌥Space.
- A quick right-⌥ tap, left ⌥, and ordinary ⌥ combos (e.g. ⌥E) don't start a stray recording.
- Without the grant, Settings says so (the tap is created anyway; the app checks CGPreflightListenEventAccess).

Pointers: crates/easyvox-app/src/controller.rs (RightOption), hotkey.rs; DESIGN.md "Push-to-talk shortcut".

Brandon, 11 Oct, hands-on round on b31d2c4 (permissions reset, fresh install), walked through step by step: turning on Hold right ⌥ put EasyVox in Input Monitoring (b31d2c4); after allowing it, right ⌥ dictated with no relaunch.

## History

### 2026-10-11 21:49 UTC — edit by `brandon`

description edited

### 2026-10-11 21:49 UTC — edit by `brandon`

status held→done
