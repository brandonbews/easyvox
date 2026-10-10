# Skip the final pass when a recording has no speech

`tuh-01M4H2EG840VHRYSSV8KJE1WFQ`

- **Status:** done
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

Done in f424519. No decode until speech (120 ms at 6 dB over the room or over -35 dBFS); silence and room noise give empty text on every model, the overlay says "Didn't catch anything". First preview pass 0.5 s into speech: first words 1.7 s -> 0.6 s (test clips), 1.0 -> 0.6 s (28 real starts). Quiet-mic stress test (243 real-time sessions, v2 + Whisper): all kept their text at 6 dB; 20 dB would have missed 24 of 28 starts at -30 dB. Energy, passes, WER unchanged. Numbers: docs/model-bench.md "First words and silence".

## History

### 2026-10-10 16:06 UTC — edit by `brandon`

description edited · status open→done
