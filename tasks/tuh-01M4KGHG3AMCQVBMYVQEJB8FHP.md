# Hands-on: first-launch onboarding from a clean slate

`tuh-01M4KGHG3AMCQVBMYVQEJB8FHP`

- **Status:** on hold — deliberately paused
- **Priority:** 1
- **Labels:** `human` `hands-on`
- **Created:** 2026-10-10 18:17 UTC by `brandon`

## Description

Brandon's check at an unlocked screen; agents never take it. Built in d989050. Resets this Mac's Microphone and Accessibility grants for EasyVox (you'll grant them again in the flow) and uses a fresh data folder, so it downloads the 539 MB model once.

1. `scripts/bundle.sh --release; tccutil reset Microphone dev.easyvox.EasyVox; tccutil reset Accessibility dev.easyvox.EasyVox; D=$(mktemp -d); open --env EASYVOX_DATA_DIR=$D --env EASYVOX_CONFIG=$D/config.toml dist/EasyVox.app`
2. 'Set Up EasyVox' opens showing '0 of 4 done'.
3. Model > Download shows progress. Turn Wi-Fi off briefly: it says the download picks up by itself.
4. Microphone > Allow... > Allow: the step gets a tick.
5. Accessibility > Allow... > Open System Settings > turn it on: tick within about 1 s, no relaunch.
6. 'Getting ready...' for about 14 s, then 'All set' > Start dictating.
7. Hold Option-Space in TextEdit: no stall, text pasted on release.
8. Quit and run the open line again: no onboarding. Does the window read well throughout?

## History

_No activity yet._
