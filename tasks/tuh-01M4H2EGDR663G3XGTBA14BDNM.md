# Bench the model list on the real-voice clips

`tuh-01M4H2EGDR663G3XGTBA14BDNM`

- **Status:** done
- **Priority:** 2
- **Labels:** `models` `bench`
- **Depends on:** [`tuh-h9ae`](tuh-01M4H2EG4X1ZDTHE9QVGJ5H9AE.md) (done)
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: DESIGN.md "Next steps" 4. The TTS clips can't tell the top models apart (all around 0.5 % WER); this task waits on Brandon's real-voice clips in testdata/.

The ask: run easyvox-bench over the real-voice clips for the Default and Alternative models, plus Parakeet Unified at 160 ms chunks (EASYVOX_PK_CHUNK_MS=160), and record the results.

Acceptance: docs/model-bench.md gains a real-voice section with WER, time to first words, finalize time, CPU while speaking and memory per setup, and a short read on whether the default still holds. DESIGN.md "Models" changes only if the list or default changes.

Pointers: crates/easyvox-bench (`help`, `session`), docs/model-bench.md, testdata/manifest.tsv.

Constraints: bench on an otherwise idle machine; numbers only compare within one run.


Done 2026-10-09 in 9a2e921 (docs/model-bench.md 'Real voice'). structured.wav (81 s, 3 runs): v2 WER 1.68 %, Unified 2.10 %, Whisper base.en 6.72 % (Canary 180M 11.76 %, footnote). First words ~0.5 s for all; final 0.37-0.42 s (v2). v2 used the least CPU (39 % vs 43-45 %). Real voice supports v2 as the default. Whisper is much worse on real voice (6.7 %) than on TTS clips (1.4 %). The EASYVOX_PK_CHUNK_MS run was skipped: that setting went away with native streaming.

## History

### 2026-10-10 02:28 UTC — edit by `brandon`

description edited · status open→done
