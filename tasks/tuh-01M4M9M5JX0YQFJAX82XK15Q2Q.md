# Hands-on: overlay animation (open and close, words fading in, Reduce motion)

`tuh-01M4M9M5JX0YQFJAX82XK15Q2Q`

- **Status:** on hold — deliberately paused
- **Priority:** 2
- **Labels:** `human` `hands-on` `overlay`
- **Created:** 2026-10-11 01:35 UTC by `brandon`

## Description

Brandon's check at an unlocked screen; agents never take it. Built in 5bd95f9. Setup: `scripts/bundle.sh && open dist/EasyVox.app` (grant Accessibility and Microphone again after a rebuild). With TextEdit in front:

1. Hold the shortcut and say a sentence: the pill fades in at once and rises a few points into place, narrow ("Listening…"); it widens smoothly as words come, new words fade in, and the fainter provisional words firm up.
2. Keep talking past the pill's width: the text glides left without jumps, and the oldest words fade out at the left edge.
3. Release: the text pastes, the green dot shows for about 0.6 s, then the pill fades out sliding down. Press again during that fade: it comes straight back.
4. Turn on System Settings > Accessibility > Display > Reduce motion and dictate again: the pill fades in and out in place (no slide), and widens and scrolls in jumps; the words still fade.
5. Does any of it feel slow or distracting? Every timing is under [advanced] in ~/.config/easyvox/config.toml (overlay_open_ms, overlay_close_ms, overlay_slide_pt, overlay_words_ms, overlay_glide_ms, overlay_frame_ms; 0 turns one off) and applies from the next time the pill opens.
6. Optional, the on-screen cost (unmeasured so far, the screen was locked): run macmon while `open -W dist/EasyVox.app --args --self-test --replay <absolute path to testdata/longform/clips/lf_3m.wav>` (self-test build), defaults vs the four durations at 0.

## History

_No activity yet._
