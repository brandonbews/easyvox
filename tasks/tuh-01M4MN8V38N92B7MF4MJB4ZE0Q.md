# Decide: keep the level bars easing (~0.15–0.2 W while talking) or let them snap

`tuh-01M4MN8V38N92B7MF4MJB4ZE0Q`

- **Status:** on hold — deliberately paused
- **Priority:** 2
- **Labels:** `decision` `human` `overlay` `energy`
- **Created:** 2026-10-11 04:59 UTC by `brandon`

## Description

The overlay's easing, as Brandon asked for it (c87dd40), draws about 29 frames a second while you talk instead of 10. That's roughly 0.15–0.2 W more, against about 0.65 W for the whole dictation (measured with Jump Desktop capturing the screen, so roughly). Most of it is the level bars easing between heights; the pop costs the app nothing.

Options:
- Keep the bars easing (the default now).
- Bars snap, everything else stays eased: overlay_meter_ms = 0 by default, back to about 10 frames a second.
- Ease the bars at a lower frame rate (overlay_meter_frame_ms 50 or 66): cheaper, choppier.

Decide after the hands-on check (tuh-01M4M9M5JX0YQFJAX82XK15Q2Q), step 6.

## History

_No activity yet._
