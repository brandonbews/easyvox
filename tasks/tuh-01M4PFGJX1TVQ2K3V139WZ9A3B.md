# Flaky self-test: 'Reduce motion swaps the open and close's pop…' read the open's scale as 0.85 once

`tuh-01M4PFGJX1TVQ2K3V139WZ9A3B`

- **Status:** done
- **Priority:** none
- **Labels:** `test` `flaky` `overlay`
- **Created:** 2026-10-11 21:57 UTC by `brandon`

## Description

11 Oct: failed once in selftest24 (b31d2c4 plus copy-only changes), passed in selftest23, 25 and 26. Detail: with reduce_motion true, the 'open' motion state had scale (0.85, 0.85) and no scale animation (scale_secs null), so scaled_in_place() failed; the samples show scale 1.0 from the second sample on, at opacity 0.14. Likely the previous close's 85% transform read before the open's reset is committed, a race in the check or in the panel's order of operations. Find which; if the panel can really show a frame at 85% under Reduce motion, fix the panel; otherwise make the check read the committed state. Detail in the session scratchpad selftest24.json.

Agent, 11 Oct: a race in the check; nothing wrong on screen. orderOut opens a Core Animation transaction, and OverlayPanel's begin/commit inside it only committed at the end of the run loop's turn. GPUI runs tasks as main-queue blocks, so the close's order-out and the self-test's poll-then-show sometimes share a turn. The open's first read then saw the presentation layer from before the order-out: the close's held end, opacity 0 at 85 %. It could never be seen, because the held scale and fade are added and removed together. A Swift probe (scratchpad flaky-selftest/probe) reproduces it 3 of 3 times, with orderOut and the reopen in one turn or in two main-queue blocks, and reads it fresh with a flush. The Reduce motion off step's first sample was stale in every run, because the resting read opens the transaction. It passed only because the model's scale animation exempts it. Fix: without_implicit_animations flushes after its commit (platform/macos.rs), noted in DESIGN.md. Every gate passes except cargo fmt --check, which already fails on about.rs from 6927ccf. Self-test loop not run: Brandon's EasyVox was running. Aside, not changed: an open under Reduce motion that interrupts a close begun without it grows back from the close's scale with the pop's overshoot. That needs Reduce motion switched on mid-dictation.

Fixed in 8c0de4c: CATransaction::flush() after the commit in without_implicit_animations (orderOut's outer transaction delayed the nested commit to the end of the run loop's turn). Self-test 10 of 10 passed, 41 checks each.

## History

### 2026-10-11 22:53 UTC — edit by `brandon`

description edited

### 2026-10-12 05:05 UTC — edit by `brandon`

description edited

### 2026-10-12 05:05 UTC — edit by `brandon`

status open→done
