# Overlay animation: open and close, words easing in, Reduce motion

`tuh-01M4H2EGEJ4R8B326AEDY4VRED`

- **Status:** open — ready
- **Priority:** 3
- **Labels:** `overlay`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: DESIGN.md "Overlay animation": subtle and quick, never slowing you down or pulling attention from what you're typing into.

The ask:
- Open and close: a fast fade plus a slight scale or slide, ~150 ms in and ~120 ms out. The pill opens on key press without waiting for the model and closes right after the paste.
- New words fade in over ~100–150 ms; provisional text eases to full strength once confirmed.
- No layout jumps: the pill's width grows smoothly, and long text scrolls smoothly to keep the newest words in view.
- With "Reduce motion" on, swap movement for plain fades.

Acceptance: each behaviour visible in a screen recording (attach or describe on the task; a person has to watch it); no frames scheduled while the overlay is hidden (a frame counter or log shows zero at idle); Reduce motion honoured.

Pointers: crates/easyvox-app/src/overlay.rs, controller.rs; GPUI Kit's animation support (gpui-kit is pinned at =0.7.1).

Constraints: the overlay never takes focus; nothing runs at idle.

Agent notes, 2026-10-10 (built, uncommitted in the working tree; DESIGN.md "Overlay animation" has the why):
- Open/close: Core Animation on the GPUI view's layer (opacity + y translation), run by the window server: no redraws, no app wake-ups. 150 ms in sliding 8 pt up, 120 ms out sliding down; a press during the close reverses it from where it is. Window moves were rejected: GPUI redraws whenever its window moves. A timer orders the panel out (CA callbacks don't come while the screen is locked).
- In the pill (GPUI): new words fade in 120 ms, provisional words firm up, the pill fits and grows with its text and long text scrolls flush right, gliding 150 ms; kept words stay put on screen across rewrites and the cut (overlay_max_chars now 200, off screen). Frames every 16 ms only while something moves.
- Reduce motion: NSWorkspace.accessibilityDisplayShouldReduceMotion read at each open; zeroes slide and glide, keeps fades (Motion params, one code path).
- New [advanced] keys: overlay_open_ms 150, overlay_close_ms 120, overlay_slide_pt 8, overlay_words_ms 120, overlay_glide_ms 150, overlay_frame_ms 16 (0 turns a duration off).
- Cost (3-min clip replay, --draw, screen locked): draws 7.0 -> 12.6/s (+5.6 frames/s); app CPU 12.46 -> 12.68 % of a core (+0.2 points; +0.4 vs animations off, about run-to-run spread), drawing timed at 0.14 points. Idle: 0 frames/draws/layer animations while hidden (self-test checks). Compositing/GPU not measured (screen locked). docs/model-bench.md "Overlay animation".
- Self-test: 36 checks, all pass (new: animations stop and nothing runs hidden; Reduce motion swaps movement for fades; a press during the close brings it back).
- Still needed (hands-on, a person at the screen): 1) hold the shortcut over TextEdit: the pill fades in and rises into place, narrow, widening smoothly as words fade in; 2) talk past its width: text glides left, oldest words fade at the left edge; 3) release: paste, green dot ~0.6 s, fade out sliding down; press during the fade: it comes straight back; 4) with Reduce motion on: fades in place, widens/scrolls in jumps, words still fade.

## History

### 2026-10-11 01:32 UTC — edit by `brandon`

description edited
