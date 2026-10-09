# Hands-on: Settings and History come to the front; model dropdown and shortcut recorder

`tuh-01M4H2EFZA72VNDBQ1G6FHV4XX`

- **Status:** on hold — deliberately paused
- **Priority:** 0
- **Labels:** `human` `hands-on`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Brandon's task; agents release it. Mark it done when finished, and file a task for anything that fails.

Setup: `scripts/bundle.sh && open dist/EasyVox.app` at an unlocked screen. The ad-hoc signature changes on every rebuild, so grant Accessibility (and Microphone) again after each one.

- With another app in front, open Settings and then History from the menu bar icon: each comes to the front with focus (both use the deprecated activateIgnoringOtherApps).
- Settings: the model dropdown lists the default first, then the downloaded alternatives; a switch applies to the next dictation.
- Record a new shortcut: the live shortcut pauses while recording, and the new combo works afterwards.
- History: newest first; Copy puts the entry on the clipboard.
- While you're there: idle CPU in Activity Monitor, with and without a window open.

Pointers: crates/easyvox-app/src/settings.rs, history.rs, tray.rs.

## History

_No activity yet._
