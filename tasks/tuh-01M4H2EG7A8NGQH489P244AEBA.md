# Core: TranscriptEvent::Level, and a level meter in the overlay

`tuh-01M4H2EG7A8NGQH489P244AEBA`

- **Status:** done
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

Measured 2026-10-10 (implementation agent; details in docs/model-bench.md "Level meter" and DESIGN.md "Overlay animation"): the app replaying the private 5-minute ramble in real time with the overlay up, Parakeet TDT v2, macmon around each run, 2 runs per setting in reverse-order rounds, M4 Mac mini.
- Meter off (meter_interval_ms = 0): 35.5 J/min of dictation, GPU 0.51 W, app CPU 10.99 % of one core.
- Meter on at the default 15 ticks/s (66 ms), not drawn: 38.1 J/min, app CPU 11.07 %; it moved 7.2 times/s, overlay redraws 7.9/s (text alone: 0.6/s).
- Same, overlay drawn at each redraw (layout and paint on the CPU): 35.4 J/min, GPU 0.52 W, app CPU 11.29 %: +0.30 points of a core (+2.7 %), no energy cost distinguishable from the CPU rail's ±0.1 W noise.
- 30 ticks/s (33 ms), drawn: 42.8 J/min, app CPU 11.42 % (+0.43 points); moved 10.9 times/s.
Not measured: the screen was locked, so GPUI presented no frames; the GPU's and WindowServer's share of on-screen drawing still needs a person at an unlocked screen (same replay command without --draw, plus macmon).

Done in b9dd9fb. Level meter at ~15/s, sent only on visible change; shared loudness with the SpeechDetector (core level.rs); NoInput for no frames or early digital silence. Light/dark pairs read per draw, WCAG AA unit test. A/B (ramble replay, overlay drawn, screen locked): meter off 10.99 % CPU / 35.5 J/min, 15/s 11.29 % / 35.4, 30/s 11.42 % / 42.8. On-screen compositing cost unmeasured (hands-on). Self-test 33/33.

## History

### 2026-10-10 19:46 UTC — edit by `brandon`

description edited

### 2026-10-10 19:51 UTC — edit by `brandon`

description edited · status open→done
