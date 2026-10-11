# Hands-on: Microphone permission prompt, then real mic dictation

`tuh-01M4H2EFXEY2V2A097BVAVPY6M`

- **Status:** done
- **Priority:** 0
- **Labels:** `human` `hands-on`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Brandon's task; agents release it. Mark it done when finished, and file a task for anything that fails.

Setup: `scripts/bundle.sh && open dist/EasyVox.app` at an unlocked screen. The ad-hoc signature changes on every rebuild, so grant Accessibility (and Microphone) again after each one. Start with Microphone not yet granted to this build (`tccutil reset Microphone dev.easyvox.EasyVox`).

- Settings → Permissions → Microphone → "Allow…" shows the system prompt; after allowing, the row reads "Allowed".
- Hold ⌥Space and speak: the text comes from the real mic. Only WAV replay has been tested so far.
- Note the time to first words and anything odd in the preview.

Pointers: crates/easyvox-app/src/settings.rs (permissions page); DESIGN.md "Engine and macOS gotchas" (opening the mic before permission is asked hangs).

Brandon, 11 Oct, hands-on round on b31d2c4 (permissions reset, fresh install), walked through step by step: the new Microphone prompt appeared from setup and real mic dictation worked.

## History

### 2026-10-11 21:49 UTC — edit by `brandon`

description edited

### 2026-10-11 21:49 UTC — edit by `brandon`

status held→done
