# Multi-minute dictations: preview and final text hold up when the key is held for minutes

`tuh-01M4H2EG038JCXHPQYECTZ1XNE`

- **Status:** on hold — deliberately paused
- **Priority:** 0
- **Labels:** `engine`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Held while an investigation runs outside tuhdoo (started 2026-10-09). When its findings land, write them into this description and reopen it, or split it into fix tasks.

Context: DESIGN.md "Requirements": long dictations are normal use, holding the key for several minutes and rambling. The live preview has to keep up the whole time, and the final text has to arrive quickly with nothing dropped. Brandon often holds the key for minutes. The bench and tests so far cover clips of 3–30 s.

The ask, once reopened: find where multi-minute sessions degrade (preview lag, memory growth, final-pass time over the whole recording, dropped audio or text) and fix it.

Acceptance (sharpen from the findings): a replayed 5- and 10-minute recording yields a complete final transcript; the preview keeps up for the whole recording; finalize time and peak memory are measured and recorded in docs/model-bench.md.

Pointers: crates/easyvox-core/src/session.rs, tcpp.rs, partials.rs, audio.rs; crates/easyvox-bench (`session`); testdata/long_*.wav.

Constraints: one model in memory; the final text still comes from one offline pass unless the findings say otherwise.

## History

_No activity yet._
