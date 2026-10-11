# Decide: file the GPUI Kit issue about the block 0.1.6 warning

`tuh-01M4MFZG2KBAWFT6A7AZ5STYNB`

- **Status:** on hold — deliberately paused
- **Priority:** 4
- **Labels:** `decision` `human` `upstream`
- **Created:** 2026-10-11 03:26 UTC by `brandon`

## Description

block 0.1.6 raises a Rust future-incompatibility warning on every build. It comes in only through GPUI (gpui-pre's cocoa, core-video and metal crates), not from EasyVox's own code. It's already tracked upstream in SSheldon/rust-block#21 and zed-industries/zed#22408.

An agent drafted an issue for longbridge/gpui-kit titled "block 0.1.6 future-incompat warning via gpui-pre (cocoa, core-video, metal)". It asks GPUI Kit to adopt Zed's objc2 snapshot or to patch block.

Options: file it (outward-facing, so it needs Brandon), or skip.

Recommendation: skip. It's only a warning today, and the upstream issues already cover it.

## History

_No activity yet._
