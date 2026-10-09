# Settings file: ~/.config/easyvox/config.toml, applied live

`tuh-01M4H2EGC4XDQT10EWCXX1STG4`

- **Status:** open — ready
- **Priority:** 2
- **Labels:** `settings` `app`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: DESIGN.md "Local storage": settings live in a TOML file at ~/.config/easyvox/config.toml ($XDG_CONFIG_HOME/easyvox/config.toml when that's set), and EASYVOX_CONFIG points at a different file. The app watches it and applies changes live; the Settings window writes the same file, so agents and dotfiles can configure EasyVox. Today AppSettings is in memory only (crates/easyvox-app/src/state.rs).

The ask: load at startup (defaults when the file is missing), write from Settings, watch for outside edits and apply them live. Resolve the path in easyvox-core next to the data and models folders.

Acceptance — tests for:
- path resolution: default, XDG_CONFIG_HOME, EASYVOX_CONFIG;
- round trip: Settings writes, a reload reads the same values;
- an invalid file: keep the last good settings, log the error, never overwrite the user's file;
- unknown keys: warn, don't fail;
- live reload: an outside edit applies without a restart, and the app's own writes don't trigger a reload loop.

Pointers: crates/easyvox-app/src/state.rs, settings.rs; crates/easyvox-core/src/paths.rs.

Constraints: the path is ~/.config on macOS too, not ~/Library/Preferences. Agents can't grant Microphone or Accessibility, so the file holds no permission state.

## History

_No activity yet._
