# Quiet the gpui "RefCell already borrowed" error logged at every launch

`tuh-01M4H2EGH07D79BJEV7Y3NCKCX`

- **Status:** done
- **Priority:** 3
- **Labels:** `app`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: setting the overlay's style mask fires a synchronous resize while GPUI is mid-update, so GPUI logs one harmless "RefCell already borrowed" (gpui window.rs, on_resize) at every launch. Harmless, but an error line in every log hides real ones.

The ask: avoid it, e.g. by applying the style mask outside GPUI's update (next run-loop turn) or before GPUI wraps the window. If there's no clean fix, filter exactly that message in the logger, with a comment saying why.

Acceptance: a launch logs no "RefCell already borrowed"; the overlay is still borderless, click-through and non-activating, and never becomes key.

Pointers: crates/easyvox-app/src/platform/macos.rs (overlay `attach`, key guard), logger.rs.

Finding (2026-10-10, agent): fixed without a log filter. OverlayPanel::attach (platform/macos.rs) now sets the style mask on the next run-loop turn (cx.spawn, the way GPUI defers its own appearance changes), when the App isn't borrowed, so GPUI's on_resize handler runs cleanly. AppKit does resize GPUI's view during setStyleMask and back (content frame 640x56 before and after). The key guard moved into the same deferred task, after the style mask: swizzling first crashed the app (NSRangeException: NSTitlebarView couldn't remove its KVO observer "_titlebarBackdropGroupName" from the swizzled EasyVoxOverlayPanel). Until that task runs, the panel is hidden and unfocused (show: false, focus: false). The self-test's two panel checks now also check the style mask (borderless, non-activating). Self-test: 33/33, no "RefCell already borrowed" (the baseline run logged it at 0.109 s). Not touched: the self-test (baseline included) logs six "window not found" errors (gpui window.rs:1760/1852/1863) around the onboarding download check at ~37 s.

Done in 7ad0869.

## History

### 2026-10-10 20:31 UTC — edit by `brandon`

description edited

### 2026-10-10 20:46 UTC — edit by `brandon`

description edited · status open→done
