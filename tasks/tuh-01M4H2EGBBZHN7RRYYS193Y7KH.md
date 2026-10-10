# Overlay: follow the system light/dark appearance

`tuh-01M4H2EGBBZHN7RRYYS193Y7KH`

- **Status:** open — ready
- **Priority:** 2
- **Labels:** `overlay`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: the overlay's colours are hard-coded dark (the TODO at the top of crates/easyvox-app/src/overlay.rs). Settings and History already follow the system through Theme::sync_system_appearance. DESIGN.md "Requirements": light and dark, following macOS and switching live.

The ask: take the overlay's colours from the theme (or a light/dark pair), and switch live when the appearance changes, including mid-dictation.

Acceptance: a visible overlay recolours when the appearance changes, and the next one opens in the right mode; confirmed and provisional text keep readable contrast (WCAG AA) in both modes; nothing runs at idle.

Pointers: crates/easyvox-app/src/overlay.rs, controller.rs (sync_system_appearance), settings.rs (the appearance subscription).

Implemented 2026-10-10 (implementation agent; DESIGN.md "Light and dark"): a light/dark pair of palettes in crates/easyvox-app/src/overlay.rs, read from the app's live appearance at each draw and redrawn when the overlay window's appearance changes. Worst-case WCAG contrast (pill composited over black or white): confirmed text 14.5:1 dark, 13.6:1 light; provisional 5.5:1 dark (was 4.2:1, under AA, before), 5.1:1 light; unit test computes them. The self-test flips EasyVox's own NSApp appearance mid-dictation (never the system's) and checks the overlay redraws itself in the right palette both ways. Still to see by eye: a real System Settings switch with the overlay up.

## History

### 2026-10-10 19:46 UTC — edit by `brandon`

description edited
