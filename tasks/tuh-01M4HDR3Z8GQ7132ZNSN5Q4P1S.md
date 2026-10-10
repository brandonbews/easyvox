# Skip preview passes when nothing new was heard

`tuh-01M4HDR3Z8GQ7132ZNSN5Q4P1S`

- **Status:** done
- **Priority:** 1
- **Labels:** `engine` `perf` `energy`
- **Created:** 2026-10-09 22:50 UTC by `brandon`

## Description

The preview re-decodes every 400 ms even through silence, costing about half a core and most of the GPU while the key is held. Skip a pass when no new speech arrived since the last one; try a shorter window now that chunks polish. Measure CPU/GPU while talking before and after (bench sessions).

In progress 2026-10-09: written but UNCOMMITTED in the working tree (app not yet compiled, gates and self-test not run). Backup patch: orchestrator scratchpad idle-wip.patch. Finish per easyvox-handoff-3.md. SpeechGate in pause.rs skips preview passes when nothing above the noise gate arrived, plus 2 settle passes. Before/after CPU/GPU still being measured.


Done 2026-10-09 in d33ab36. SpeechGate (pause.rs) skips a preview pass when nothing above the noise gate arrived since the last one, after 2 settle passes. Real voice (structured 81 s / ramble 5 min): skips 3/194 and 10-13/~719 passes; CPU 45-48/41-44 % -> 44-46/40-43 %, GPU ~88-92 % -> 87-91 %; first words, final latency and WER (2.10 %) unchanged. Real pauses are mostly 0.4-0.8 s breaths, shorter than the ~1.2 s before skipping starts. 44 s TTS clip with long pauses: CPU 40 -> 27 %, GPU 89 -> 70 %, but final latency after a long pause +25 ms (median 0.28 -> 0.31 s), likely GPU clocks dropping. Kept. A shorter window / longer interval was not tried; that moves to the energy spike (tuh-01M4HDR400G242K017BJ1HB8RB).

## History

### 2026-10-09 23:37 UTC — edit by `brandon`

description edited

### 2026-10-10 00:57 UTC — edit by `brandon`

description edited · status open→done
