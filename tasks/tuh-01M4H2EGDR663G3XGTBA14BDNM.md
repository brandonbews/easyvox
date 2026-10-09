# Bench the model list on the real-voice clips

`tuh-01M4H2EGDR663G3XGTBA14BDNM`

- **Status:** open — blocked on dependencies
- **Priority:** 2
- **Labels:** `models` `bench`
- **Depends on:** [`tuh-h9ae`](tuh-01M4H2EG4X1ZDTHE9QVGJ5H9AE.md) (on hold)
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: DESIGN.md "Next steps" 4. The TTS clips can't tell the top models apart (all around 0.5 % WER); this task waits on Brandon's real-voice clips in testdata/.

The ask: run easyvox-bench over the real-voice clips for the Default and Alternative models, plus Parakeet Unified at 160 ms chunks (EASYVOX_PK_CHUNK_MS=160), and record the results.

Acceptance: docs/model-bench.md gains a real-voice section with WER, time to first words, finalize time, CPU while speaking and memory per setup, and a short read on whether the default still holds. DESIGN.md "Models" changes only if the list or default changes.

Pointers: crates/easyvox-bench (`help`, `session`), docs/model-bench.md, testdata/manifest.tsv.

Constraints: bench on an otherwise idle machine; numbers only compare within one run.

## History

_No activity yet._
