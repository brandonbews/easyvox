# Hands-on: overlay animation (pop in and out, eased bars, words fading in, Reduce motion)

`tuh-01M4M9M5JX0YQFJAX82XK15Q2Q`

- **Status:** on hold — deliberately paused
- **Priority:** 2
- **Labels:** `human` `hands-on` `overlay`
- **Created:** 2026-10-11 01:35 UTC by `brandon`

## Description

Brandon's check at an unlocked screen; agents never take it. Built in c87dd40 (supersedes 5bd95f9's fade and slide). Two clips of this build were sent in the session. Setup: `scripts/bundle.sh && open dist/EasyVox.app` (grant Accessibility and Microphone again after a rebuild). With TextEdit in front:

1. Hold the shortcut and say a sentence: the pill pops in (fades in while growing from slightly small, with a tiny overshoot). The level bars ease up and down with your voice, new words fade in, the fainter provisional words firm up, and the pill widens smoothly.
2. Keep talking past the pill's width: the text glides left without jumps, and the oldest words fade out at the left edge. Watch for a new word appearing cut off at the right edge before the pill has finished widening: is that fine, or distracting?
3. Release: the text pastes, the green dot shows for about 0.6 s, then the pill shrinks a little as it fades out. Press again during that: it pops straight back.
4. Turn on System Settings > Accessibility > Display > Reduce motion and dictate again: no pop, glide or bar easing; the pill and the words only fade.
5. Does any of it feel slow or distracting? The timings are under [advanced] in ~/.config/easyvox/config.toml (overlay_open_ms, overlay_close_ms, overlay_shrink_pct, overlay_words_ms, overlay_glide_ms, overlay_meter_ms, overlay_meter_frame_ms, overlay_frame_ms; 0 turns one off) and apply from the next time the pill opens.
6. Energy, see the decision task: try overlay_meter_ms = 0 (bars snap instead of easing) and see if you miss the easing.

11 October: the pill was restyled in c2e8837 (44 pt, 92 % fill, three vermilion strokes, grey dots then a check) and now sits 24 pt above the Dock. The motion is unchanged, so these steps still apply; do them together with the restyle check above (green dot → now a check).

## History

### 2026-10-11 04:59 UTC — edit by `brandon`

retitled · description edited

### 2026-10-11 16:50 UTC — edit by `brandon`

description edited
