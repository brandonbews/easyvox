# Hands-on: does the lighter preview feel OK?

`tuh-01M4JNRWA7PCN1YSN2PBY4XQ3V`

- **Status:** on hold — deliberately paused
- **Priority:** 2
- **Labels:** `human` `hands-on`
- **Created:** 2026-10-10 10:29 UTC by `brandon`

## Description

Brandon's check at an unlocked screen. Since the lightness change the preview updates every 1.6 s (was 0.4 s) and first words take ~1.7 s (unless "first preview pass early" has landed). Final text is unchanged.

`scripts/bundle.sh && open dist/EasyVox.app`, grant Accessibility and Microphone again, then dictate into TextEdit: a short sentence, then a minute or two of rambling. Judge: does the preview feel too slow or jumpy? If so, the settings file (once built) can change preview_interval_ms; say what felt right and we'll tune the default.

## History

_No activity yet._
