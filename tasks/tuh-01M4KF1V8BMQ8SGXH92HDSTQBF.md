# Hands-on: Settings Models page, downloads, and About

`tuh-01M4KF1V8BMQ8SGXH92HDSTQBF`

- **Status:** on hold — deliberately paused
- **Priority:** 1
- **Labels:** `human` `hands-on`
- **Created:** 2026-10-10 17:51 UTC by `brandon`

## Description

Brandon's check at an unlocked screen; agents never take it. Built in 695c6d1; the self-test drew every state, but no person has looked at it yet.

Setup: `scripts/bundle.sh && open dist/EasyVox.app`, and grant Accessibility and Microphone again after a rebuild (the ad-hoc signature changes). Deleting and downloading Whisper here really deletes and re-downloads it (85 MB) in your models folder.

1. Settings > Models: two rows (Parakeet TDT v2, Whisper base.en), v2 marked Active, its Delete greyed out. Does the page read well: sizes, the 'pick this if' lines, spacing?
2. Delete on Whisper: a confirmation saying 'Delete 85 MB'; after it, the row offers Download.
3. Download: a progress bar; Cancel leaves no file behind. Download again to the end.
4. Use on Whisper: it becomes Active and the next dictation uses it. Switch back to v2.
5. Click 'CC BY 4.0', 'handy-computer' and 'Model card': each opens in the browser.
6. Tray > About EasyVox: the About page with both models' credits.
7. 'Third-party notices' opens the HTML; 'License' opens the MIT text.

## History

_No activity yet._
