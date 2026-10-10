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

Finding (2026-10-10, agent): upstream, not ours to move. `cargo tree -i block` (all features, all targets): block 0.1.6 comes only through GPUI: gpui-pre-macos -> cocoa 0.26.1 / cocoa-foundation 0.2.1, and gpui-pre / gpui-pre-apple -> core-video 0.5.2 (-> core-graphics2 0.5.2) and metal 0.33.0. None is our direct dependency (objc-foundation, screencapturekit-sys and cocoa 0.25 are in Cargo.lock only as GPUI's unused optional deps). The latest releases of every one of them (cocoa 0.27.0, cocoa-foundation 0.2.1, core-video 0.6.1, core-graphics2 0.6.1, metal 0.33.0) still depend on block 0.1, and gpui-kit 0.7.1 / gpui-pre 0.3.8 are the newest published, so no upgrade drops it. The lint: block's `static _NSConcreteStackBlock: Class` is a static of an uninhabited type (rust-lang/rust#74840). block is unmaintained (last push 2024, no release since 2016). Upstream issues: https://github.com/SSheldon/rust-block/issues/21 (this warning) and https://github.com/zed-industries/zed/issues/22408 ("Migrate Zed to objc2", open; a 2026-05-29 comment reports this exact warning). Nothing filed: no gpui-kit issue exists; a draft is in the agent's report for Brandon to decide. Not done: a [patch] of block (one more special case, for a warning that isn't an error yet).

## History

### 2026-10-10 20:37 UTC — edit by `brandon`

description edited
