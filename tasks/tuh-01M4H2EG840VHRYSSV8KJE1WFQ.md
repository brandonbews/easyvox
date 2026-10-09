# Skip the final pass when a recording has no speech

`tuh-01M4H2EG840VHRYSSV8KJE1WFQ`

- **Status:** open — ready
- **Priority:** 1
- **Labels:** `engine`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: Whisper base.en makes up words on silence (2 s of silence came out as "you"; once a stray "Mhm." at the end of a long clip), and no model should paste anything for a recording with no speech. DESIGN.md "Models" → "Alternatives" asks the app to skip the final pass when the recording holds no speech (VAD or a level check), whichever model is active.

The ask: before the final pass, decide whether the recording holds speech. If not, skip the decode and send an empty Final: nothing pasted, no History entry.

Acceptance:
- Engine test: testdata/silence_2s.wav (and low-level noise) through each offered model yields an empty Final with no decode.
- Every speech clip in testdata/ still transcribes with unchanged WER, including quiet starts and trailing pauses.
- The app pastes nothing and records nothing in History for an empty Final.

Pointers: crates/easyvox-core/src/session.rs (final pass), audio.rs, lib.rs (TranscriptEvent::Final); crates/easyvox-app/src/controller.rs (paste and History on Final). The Silero VAD model in the catalog is bench-only: it needs sherpa-onnx, which isn't compiled into the app.

Constraints: no new runtime in the app (one ggml and one ONNX Runtime per binary), so an energy or level gate in core is the likely shape. Share the level computation with tuh-01M4H2EG7A8NGQH489P244AEBA (TranscriptEvent::Level) rather than writing two.

## History

_No activity yet._
