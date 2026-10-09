# Test chunk cutting on real-voice long recordings; consider a VAD

`tuh-01M4HDR40PA99G6HQDAN026HTQ`

- **Status:** open — ready
- **Priority:** 2
- **Labels:** `engine` `bench`
- **Created:** 2026-10-09 22:50 UTC by `brandon`

## Description

The energy-based pause finder (pause.rs) was only tested on TTS clips with clean silences. Test it on real-voice long recordings, including a noisy room; consider Silero VAD (models/vad/ already has silero_vad.onnx, but there's no ONNX runtime in the app build).

## History

_No activity yet._
