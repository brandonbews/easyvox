# Core: TranscriptEvent::Level, and a level meter in the overlay

`tuh-01M4H2EG7A8NGQH489P244AEBA`

- **Status:** open — ready
- **Priority:** 2
- **Labels:** `engine` `overlay`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: DESIGN.md "Overlay animation" → "Hearing input": a small level indicator (a few bars or a waveform) moves with your voice and rests flat in silence. It's the first sign the mic works. If no audio arrives at all, show a clear "can't hear you" state instead of a flat line. The core's TranscriptEvent only carries text today.

The ask:
- easyvox-core: add `TranscriptEvent::Level(f32)`, the smoothed input level (0..1) about 30 times a second while a session runs.
- easyvox-app: draw the meter in the overlay pill from it, and a "can't hear you" state when no audio frames arrive (distinct from quiet audio).

Acceptance:
- Tests: a WavReplay session emits Level at ~30 Hz; testdata/silence_2s.wav stays near 0; speech clips rise above a threshold.
- The meter redraws only while the overlay is visible; nothing runs at idle.
- CPU while speaking, with the screen unlocked, measured and recorded on this task (the spike only measured text updates, not on-screen drawing).

Pointers: crates/easyvox-core/src/lib.rs (TranscriptEvent), audio.rs, session.rs; crates/easyvox-app/src/overlay.rs, controller.rs.

Constraints: keep the level computation reusable: the no-speech gate for the final pass needs the same signal.

## History

_No activity yet._
