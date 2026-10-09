# Quiet the gpui "RefCell already borrowed" error logged at every launch

`tuh-01M4H2EGH07D79BJEV7Y3NCKCX`

- **Status:** open — ready
- **Priority:** 3
- **Labels:** `app`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: setting the overlay's style mask fires a synchronous resize while GPUI is mid-update, so GPUI logs one harmless "RefCell already borrowed" (gpui window.rs, on_resize) at every launch. Harmless, but an error line in every log hides real ones.

The ask: avoid it, e.g. by applying the style mask outside GPUI's update (next run-loop turn) or before GPUI wraps the window. If there's no clean fix, filter exactly that message in the logger, with a comment saying why.

Acceptance: a launch logs no "RefCell already borrowed"; the overlay is still borderless, click-through and non-activating, and never becomes key.

Pointers: crates/easyvox-app/src/platform/macos.rs (overlay `attach`, key guard), logger.rs.

## History

_No activity yet._
