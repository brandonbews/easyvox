# Future-incompat warning from the block 0.1.6 dependency

`tuh-01M4H2EGJKYV58RHW70R9YY75B`

- **Status:** open — ready
- **Priority:** 3
- **Labels:** `deps`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: cargo reports a future-incompatibility warning for block 0.1.6. In Cargo.lock it comes in through cocoa, cocoa-foundation, core-graphics2, core-video, metal, objc-foundation and screencapturekit-sys, mostly behind gpui. A future Rust release may reject it.

The ask: trace which of our direct dependencies pull it in (`cargo tree -i block`), and whether a newer version of that dependency drops it (block2). Upgrade if it's ours to move; otherwise find or file the upstream issue.

Acceptance: `cargo report future-incompatibilities` is clean, or the source is an upstream crate we can't move, with the upstream issue linked on this task.

Constraints: gpui-kit is pinned exactly (=0.7.1, which pins gpui-pre =0.3.8); don't upgrade it as a side effect.

## History

_No activity yet._
