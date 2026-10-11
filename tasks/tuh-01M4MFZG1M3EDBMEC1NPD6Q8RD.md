# Decide: delete Parakeet Unified and v3 from the real models folder (1.1 GB, bench-only)

`tuh-01M4MFZG1M3EDBMEC1NPD6Q8RD`

- **Status:** on hold — deliberately paused
- **Priority:** 4
- **Labels:** `decision` `human` `models`
- **Created:** 2026-10-11 03:26 UTC by `brandon`

## Description

Parakeet Unified (541 MB) and Parakeet TDT v3 (549 MB) sit in the real models folder, ~/Library/Application Support/EasyVox/models/gguf/. Both are bench-only now: the app doesn't offer them. Unified lost to v2 on 9 Oct, and v3 was no better than v2 on Brandon's voice and cost more (docs/model-bench.md).

Options: delete both and free about 1.1 GB, or keep them for future benches.

Recommendation: delete. scripts/fetch-model.sh fetches either again, checked against its pinned sha256, if a bench ever needs it. Agents never delete without Brandon's yes.

## History

_No activity yet._
