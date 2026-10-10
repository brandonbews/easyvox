# Settings file: ~/.config/easyvox/config.toml, applied live

`tuh-01M4H2EGC4XDQT10EWCXX1STG4`

- **Status:** open — ready
- **Priority:** 1
- **Labels:** `settings` `app`
- **Depends on:** [`tuh-z7dr`](tuh-01M4HVK6QMYNHCMAGZ57G0Z7DR.md) (done)
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


Added 2026-10-09 (Brandon): every tuning constant goes in the file too, not necessarily in the UI: "make it easy for us to tune the feel of the defaults, and then users have the benefit of advanced settings through the file if they want." So the file gets an [advanced] table (name to taste) exposing the core tuning struct from tuh-01M4HVK6QMYNHCMAGZ57G0Z7DR: preview interval (today 400 ms), preview window cap, chunk length (~30 s), join slack, SpeechGate threshold and settle passes, pause-finder levels and durations, unload_after_secs, and any other constant that shapes feel or cost. Rules:
- One source of truth: the defaults live in core; the file only overrides. Missing keys = defaults, so a default can change in a release without being frozen in old files (don't write untouched advanced keys back out; consider a commented reference listing of every key and its default, e.g. in README or `easyvox --print-default-config`).
- Validate ranges; an out-of-range value logs a warning and falls back to the default for that key only.
- Engine values apply from the next recording (no restart); say so in the docs.
- Same keys for every model (one pipeline, no per-model tables).
- Document every key, its default, its unit and what it trades off, in plain words.
Priority raised to 1 so it lands right after the lightness task (which picks the defaults).


Notes from the Tuning refactor (da36537): the struct is easyvox_core::Tuning (crates/easyvox-core/src/tuning.rs; one macro generates fields, defaults, key list, set() and validate()); AppSettings.tuning holds it. serde is not a core dependency yet. (1) Loudness fields are linear levels (e.g. 0.0018, about -54.9 dBFS) to keep behaviour exact; for a human-edited file, dB is friendlier: switch deliberately, with an equivalence check. (2) Changing Tuning today means reloading the engine; make it apply from the next recording without a reload. App-level knobs to expose too: unload_after_secs, keep_model_loaded, overlay hide delays (600/2500 ms), overlay MAX_CHARS.

## History

### 2026-10-10 03:33 UTC — edit by `brandon`

description edited · priority 2→1 · depends_on +tuh-z7dr

### 2026-10-10 04:41 UTC — edit by `brandon`

description edited
