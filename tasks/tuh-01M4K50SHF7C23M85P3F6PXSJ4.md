# Drop Parakeet Unified from the app (keep it bench-only)

`tuh-01M4K50SHF7C23M85P3F6PXSJ4`

- **Status:** open — ready
- **Priority:** 1
- **Labels:** `models`
- **Created:** 2026-10-10 14:56 UTC by `brandon`

## Description

Brandon, 2026-10-10: "If Unified is worse, do we even want it?" Agreed, unless he objects. On his voice Unified was ~1 word worse than v2 (2.10 % vs 1.68 % WER on the 81 s read-aloud), the same speed, slightly more CPU, often leaves out the last full stop, and its one advantage (native streaming, faster first words) went away with one pipeline. Its NVIDIA Open Model License carries use terms that would need an "I agree" step, a special case. Public benchmarks score it a little above v2; his voice says otherwise.

The ask: the app offers v2 (default) and Whisper base.en (the small option) only. In crates/easyvox-core/src/models.rs make Unified bench-only (Offering::BenchOnly or equivalent), keep its pinned URL and sha256 so easyvox-bench and fetch-model.sh can still fetch it; update scripts/fetch-model.sh's row (drift-guard test), the Settings model list, DESIGN.md "Models", README, docs/model-bench.md and docs/settings.md wherever Unified is offered. A config.toml that names Unified as `model` falls back to the default with a warning (the existing unknown/invalid path). The NVIDIA notice work in tuh-01M4H2EGCYFBHHG4S9307GK2W3 shrinks to "none in the app" (keep the constants for the bench docs if useful).

## History

_No activity yet._
