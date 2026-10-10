# Whisper base.en preview: heavy flicker and growing CPU

`tuh-01M4HDR425AX71RAQFA15Q8KK7`

- **Status:** done
- **Priority:** 3
- **Labels:** `engine` `models`
- **Created:** 2026-10-09 22:50 UTC by `brandon`

## Description

On the one pipeline Whisper's preview revises ~50 % of words, one preview pass took 1.9 s, and its CPU rose over 3 minutes (40 -> 56 %). Investigate or accept as the small/light option's limits.

Findings, 10 Oct 2026 (agent; uncommitted working tree on cd78c42, for review). Full numbers: docs/model-bench.md "Whisper's preview".
- Re-measured on today's defaults (pass every 1.6 s from the confirmed words, speech detector, meter): real-time session replays, macmon energy, quiet machine, models alternating. Ramble x2 (v2 x3), lf_3m, lf_10m.
- CPU growth and slow passes: gone since Lightness. Whisper CPU 11-13 % of a core, flat in every minute of 9 min (was 40 -> 56 %); slowest pass 0.20-0.43 s, p95 0.17-0.19 s (was 1.9 s on the 30 s clip, now 0.23 s there). Energy 39-44 J/min vs v2 24-41 J/min (GPU +0.01-0.09 W). Lag 0.77-0.81 / 1.70-2.19 s (median / p95), like v2.
- Flicker: still there. Updates that changed words on screen 62-63 % ramble, 42 % lf_3m, 54 % lf_10m (v2 32-35 / 21 / 22 %). Cause: half of Whisper's passes confirmed nothing. Whisper only times segments, words are spread evenly over a pass, so the time boundary of the confirmed words landed a word or two off between passes and LA-2 never agreed (40 of 93, 24 of 36, 82 of 130 stalled passes were just that offset); the rest is Whisper hearing words differently. Side effect: words left out of the confirmed preview until chunk polish (254 of 1,732 on lf_10m).
- Fix (general, every model): partials.rs LocalAgreement starts a pass's new words after its text copy of the confirmed words' end, searched near the time boundary (new Tuning key agreement_reach_words = 2, [advanced]; 0 = much as before). Whisper: revising updates 50-53 / 31 / 27 %, stalled passes -25 % ramble, -73..81 % TTS, lf_10m missing words 254 -> 23. v2: rule differs in 1 of 185 passes (logged side by side); numbers within run spread. Final text identical everywhere; read-aloud WER v2 1.68 %, Whisper 6.72 %, unchanged.
- Gates 5/5 PASS, bundle + self-test PASS (33 checks). Remaining Whisper flicker on real voice is base.en hearing the same audio differently pass to pass: accepted as the small option's limit.

Committed and pushed as fc7c60a after the orchestrator re-ran the 5 gates, bundle and self-test (33/33). Remaining Whisper flicker on real voice accepted as the small option's limit.

## History

### 2026-10-10 23:39 UTC — edit by `brandon`

description edited

### 2026-10-10 23:42 UTC — edit by `brandon`

description edited · status open→done
