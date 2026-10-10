# Hands-on: 'Keep model loaded' switch, and first words after an unload

`tuh-01M4HN1AWAEEECAS8SWAJ5X9CD`

- **Status:** on hold — deliberately paused
- **Priority:** 1
- **Labels:** `human` `hands-on`
- **Created:** 2026-10-10 00:57 UTC by `brandon`

## Description

Brandon's check at an unlocked screen; agents never take it. Built in d33ab36.

Setup: `scripts/bundle.sh && open dist/EasyVox.app`, and grant Accessibility and Microphone again after a rebuild (the ad-hoc signature changes).

1. The switch: Settings shows "Keep model loaded". Turn it on, leave the app idle for over 10 minutes, then dictate: the first press should feel the same as usual. In Activity Monitor, EasyVox should stay around 600-670 MB.
2. Unload, then reload: turn the switch off and leave the app idle for over 10 minutes. Memory should drop to about 90 MB. Then hold ⌥Space and start talking straight away, mid-sentence, into TextEdit. The first word must appear in the final text (the model reloads in ~0.15 s while recording).
3. Note anything odd in the overlay while it reloads.

Record the result here and mark it done, or file a task for anything wrong.

## History

_No activity yet._
