# Bench every candidate model on the real recordings with the current pipeline (adds Whisper large-v3-turbo)

`tuh-01M4N0BZXWBR99VD2A6KVBJEXJ`

- **Status:** open — ready
- **Priority:** 1
- **Labels:** `bench`
- **Created:** 2026-10-11 08:13 UTC by `brandon`

## Description

Brandon asked (11 October) for apples-to-apples numbers for every candidate model, with the approach we've landed on: the current build and its defaults (a preview pass every 1.6 s, local agreement, ~30 s chunks, speech detection). Real voice: recordings/structured.wav (with its reference) and recordings/ramble.wav; plus the 14 text-to-speech clips and the silence clip. Whisper large-v3-turbo is new; the rest are the models in the catalog and in DESIGN.md's 'Other models we looked at' that run on transcribe-cpp 0.3.1. New downloads are bench-only catalog entries in a separate models folder. Results: numbers only, in docs/model-bench.md and docs/bench/*.csv.

## History

_No activity yet._
