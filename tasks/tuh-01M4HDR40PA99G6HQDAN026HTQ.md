# Test chunk cutting on real-voice long recordings; consider a VAD

`tuh-01M4HDR40PA99G6HQDAN026HTQ`

- **Status:** done
- **Priority:** 2
- **Labels:** `engine` `bench`
- **Created:** 2026-10-09 22:50 UTC by `brandon`

## Description

The energy-based pause finder (pause.rs) was only tested on TTS clips with clean silences. Test it on real-voice long recordings, including a noisy room; consider Silero VAD (models/vad/ already has silero_vad.onnx, but there's no ONNX runtime in the app build).


Done 2026-10-09 in 9a2e921. ramble.wav (5 min, office, AirPods): all 13 cuts landed in real pauses (0.72-1.92 s, at -40 dBFS or quieter vs speech at -17), none forced, none mid-word. Final 0.19-0.35 s after release (single pass: 2.6-8.2 s). Preview lag median 0.2 s, p95 0.7 s, max 2.1 s, flat over 5 min. Joins: v2 lost 1 word, Unified lost 2 and doubled 1, Whisper lost 1 (positions in the doc). Whisper's preview showed 'you' on room noise before the first word, never in the final. VAD: not needed; keep the loudness pause finder (~10 dB headroom over this office's noise; Silero only runs via sherpa-onnx). Chunked decoding beat one pass (v2 1.68 % vs 3.36 %). Follow-ups: wider join slack (new task), noise-floor term (inbox).

## History

### 2026-10-10 02:28 UTC — edit by `brandon`

description edited · status open→done
