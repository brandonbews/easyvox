# Idle wake-ups from ggml-metal's residency loop

`tuh-01M4H2EGG52WC4E29B82S4KSMT`

- **Status:** done
- **Priority:** 1
- **Labels:** `engine` `perf`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: DESIGN.md "Engine and macOS gotchas" → "Idle wake-ups": ggml's Metal code wakes the process ~210 times a second for as long as its GPU device exists, even after the model is unloaded. GGML_METAL_NO_RESIDENCY=1 brings it to ~3.5 a second, but its effect on the first transcription after a long idle is unmeasured. A small upstream fix could make the loop sleep when idle.

The ask: measure idle wake-ups and the first dictation's latency after 5 and 30 minutes idle, with and without GGML_METAL_NO_RESIDENCY=1. Then pick: set the variable in the app, patch ggml upstream (via transcribe.cpp), or leave it, and implement the pick.

Acceptance: the numbers recorded in docs/model-bench.md; the chosen fix in place; idle wake-ups below ~10 a second with the app idle, or the reason not documented.

Pointers: crates/easyvox-core/src/tcpp.rs; the transcribe-cpp crate and its vendored ggml (ggml-metal residency sets).

Constraints: no regression in time to first words.

In progress 2026-10-09: written but UNCOMMITTED in the working tree (app not yet compiled, gates and self-test not run). Backup patch: orchestrator scratchpad idle-wip.patch. Finish per easyvox-handoff-3.md. Approach: each binary sets GGML_METAL_NO_RESIDENCY=1 at the top of main (EASYVOX_METAL_RESIDENCY=1 restores the old behaviour). Measured: idle wake-ups 145/s -> 0.3/s, idle CPU 0.2 % -> 0.002 % of a core, no measurable first-dictation latency cost after 5 min idle.


Done 2026-10-09 in d33ab36. Picked: set GGML_METAL_NO_RESIDENCY=1 at the top of each main (easyvox_core::process_env; EASYVOX_METAL_RESIDENCY=1 restores the old behaviour). Idle wake-ups 145/s -> 0.3/s (loaded or unloaded), idle CPU 0.15-0.24 % -> 0.002 % of a core, loaded footprint 687 -> 667 MB. First words and final latency unchanged warm and after 5 min idle. 30 min idle not measured separately (unload at 10 min makes it a reload case). Numbers in docs/model-bench.md. Upstream patch stays a separate held task (tuh-01M4HDR43KDP1NSH6C16DFH2JJ).

## History

### 2026-10-09 21:43 UTC — edit by `brandon`

priority 3→1

### 2026-10-09 23:37 UTC — edit by `brandon`

description edited

### 2026-10-10 00:57 UTC — edit by `brandon`

description edited · status open→done
