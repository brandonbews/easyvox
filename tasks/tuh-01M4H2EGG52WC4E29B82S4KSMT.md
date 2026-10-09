# Idle wake-ups from ggml-metal's residency loop

`tuh-01M4H2EGG52WC4E29B82S4KSMT`

- **Status:** open — ready
- **Priority:** 3
- **Labels:** `engine` `perf`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: DESIGN.md "Engine and macOS gotchas" → "Idle wake-ups": ggml's Metal code wakes the process ~210 times a second for as long as its GPU device exists, even after the model is unloaded. GGML_METAL_NO_RESIDENCY=1 brings it to ~3.5 a second, but its effect on the first transcription after a long idle is unmeasured. A small upstream fix could make the loop sleep when idle.

The ask: measure idle wake-ups and the first dictation's latency after 5 and 30 minutes idle, with and without GGML_METAL_NO_RESIDENCY=1. Then pick: set the variable in the app, patch ggml upstream (via transcribe.cpp), or leave it, and implement the pick.

Acceptance: the numbers recorded in docs/model-bench.md; the chosen fix in place; idle wake-ups below ~10 a second with the app idle, or the reason not documented.

Pointers: crates/easyvox-core/src/tcpp.rs; the transcribe-cpp crate and its vendored ggml (ggml-metal residency sets).

Constraints: no regression in time to first words.

## History

_No activity yet._
