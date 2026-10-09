# Hands-on: the debug "Test dictation" menu item

`tuh-01M4H2EG3AT1WSW8YRH9DXH9JA`

- **Status:** on hold — deliberately paused
- **Priority:** 1
- **Labels:** `human` `hands-on`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Brandon's task; agents release it. Mark it done when finished, and file a task for anything that fails.

Setup: the item exists only in debug builds: `scripts/bundle.sh --dev && open dist/EasyVox.app`, screen unlocked, Accessibility granted to this build.

- Menu bar → "Test dictation (WAV replay)": the overlay streams a testdata/ clip's text, and the final text pastes into the focused app and lands in History.

Pointers: crates/easyvox-app/src/tray.rs, main.rs (`testdata_dir`, `EASYVOX_TESTDATA`).

## History

_No activity yet._
