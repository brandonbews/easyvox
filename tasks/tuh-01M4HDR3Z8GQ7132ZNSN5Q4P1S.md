# Skip preview passes when nothing new was heard

`tuh-01M4HDR3Z8GQ7132ZNSN5Q4P1S`

- **Status:** open — ready
- **Priority:** 1
- **Labels:** `engine` `perf` `energy`
- **Created:** 2026-10-09 22:50 UTC by `brandon`

## Description

The preview re-decodes every 400 ms even through silence, costing about half a core and most of the GPU while the key is held. Skip a pass when no new speech arrived since the last one; try a shorter window now that chunks polish. Measure CPU/GPU while talking before and after (bench sessions).

In progress 2026-10-09: written but UNCOMMITTED in the working tree (app not yet compiled, gates and self-test not run). Backup patch: orchestrator scratchpad idle-wip.patch. Finish per easyvox-handoff-3.md. SpeechGate in pause.rs skips preview passes when nothing above the noise gate arrived, plus 2 settle passes. Before/after CPU/GPU still being measured.

## History

### 2026-10-09 23:37 UTC — edit by `brandon`

description edited
