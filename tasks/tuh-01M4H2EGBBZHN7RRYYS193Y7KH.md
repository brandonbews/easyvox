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

## History

_No activity yet._
