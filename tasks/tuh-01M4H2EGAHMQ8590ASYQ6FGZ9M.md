# Settings: click-to-download the alternative models

`tuh-01M4H2EGAHMQ8590ASYQ6FGZ9M`

- **Status:** done
- **Priority:** 2
- **Labels:** `models` `app`
- **Depends on:** [`tuh-x7tv`](tuh-01M4H2EG8YM9TXGK81DC6WX7TV.md) (done)
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: Settings lists only the models that are downloaded and runnable (DESIGN.md "Models"). The alternatives (Parakeet TDT v2, Whisper base.en) should be downloadable from Settings with the same flow onboarding uses.

The ask: in Settings' model section, list every Default and Alternative model with its size and its one-line "pick this if". Models not yet downloaded get a Download button with progress and cancel; downloaded ones can be activated or deleted, with disk usage shown.

Acceptance: download, cancel, delete and activate each work for both alternatives; a checksum failure shows an error and leaves nothing behind; BenchOnly models never appear; a switch applies to the next dictation; only one model is ever loaded.

Pointers: crates/easyvox-app/src/settings.rs, state.rs; crates/easyvox-core/src/models.rs (offering, download_bytes); DESIGN.md "Models" (the "pick this if" lines).

Done in 695c6d1: Settings > Models page from a reusable row (crates/easyvox-app/src/model_row.rs, for onboarding): size, pick-this-if, Download with progress and Cancel, Use, Delete with confirm and freed size (refused while active or downloading). Switch frees the old model first under MODEL_SWAP. Self-test covers download from 127.0.0.1, cancel, delete, switch (26/26). UI not yet seen by a person: hands-on tuh in the backlog.

## History

### 2026-10-10 17:51 UTC — edit by `brandon`

description edited · status open→done
