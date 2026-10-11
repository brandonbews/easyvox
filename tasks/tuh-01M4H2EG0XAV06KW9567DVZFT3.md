# Hands-on: live light/dark switch

`tuh-01M4H2EG0XAV06KW9567DVZFT3`

- **Status:** done
- **Priority:** 1
- **Labels:** `human` `hands-on`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Brandon's task; agents release it. Mark it done when finished, and file a task for anything that fails.

Setup: `scripts/bundle.sh && open dist/EasyVox.app` at an unlocked screen. The ad-hoc signature changes on every rebuild, so grant Accessibility (and Microphone) again after each one.

- With Settings and History open, switch System Settings → Appearance between Light and Dark: both windows follow live.
- Switch while no window is open, then open one: it opens in the new mode.
- The menu bar icon tints correctly (template image).
- The overlay stays dark for now (its colours are hard-coded until the overlay theming task lands); just note whether it looks wrong in light mode.

Brandon, 11 Oct, hands-on round on b31d2c4 (permissions reset, fresh install), walked through step by step: Settings and History switched cleanly between light and dark while open; the light pill looked right.

## History

### 2026-10-11 21:49 UTC — edit by `brandon`

description edited

### 2026-10-11 21:49 UTC — edit by `brandon`

status held→done
