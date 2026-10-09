# Skip preview passes when nothing new was heard

`tuh-01M4HDR3Z8GQ7132ZNSN5Q4P1S`

- **Status:** open — ready
- **Priority:** 1
- **Labels:** `engine` `perf` `energy`
- **Created:** 2026-10-09 22:50 UTC by `brandon`

## Description

The preview re-decodes every 400 ms even through silence, costing about half a core and most of the GPU while the key is held. Skip a pass when no new speech arrived since the last one; try a shorter window now that chunks polish. Measure CPU/GPU while talking before and after (bench sessions).

## History

_No activity yet._
