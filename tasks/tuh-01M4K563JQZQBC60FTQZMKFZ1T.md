# Bench Parakeet TDT v3 on Brandon's voice

`tuh-01M4K563JQZQBC60FTQZMKFZ1T`

- **Status:** open — ready
- **Priority:** 1
- **Labels:** `models` `bench`
- **Created:** 2026-10-10 14:58 UTC by `brandon`

## Description

Brandon asked, 2026-10-10. v3 is v2's multilingual successor (25 European languages, same size/speed, CC-BY-4.0). On the public leaderboard it's a little behind v2 on English (avg WER ~6.82 vs ~6.43; model research), and it has never been run on Brandon's recordings.

Pinned (from the model research): handy-computer/parakeet-tdt-0.6b-v3-gguf, file parakeet-tdt-0.6b-v3-Q5_K_M.gguf, 548,946,272 bytes, sha256 cc722e76adc1a629fc0b2535de879d99b8160d07ad4c0215e2ca7d7ea0ae4b8f, commit 90f082450fcbacdb54e5900c44ef697c9ea59622. Runs on transcribe-cpp 0.3.1.

The ask: add v3 to the catalog as bench-only (models.rs + fetch-model.sh row, drift-guard test), fetch it (sha256-verified, into the models folder alongside the others; never touch existing files), and bench it exactly like docs/model-bench.md "Real voice" with today's defaults: recordings/structured.wav WER (3 runs) vs v2; recordings/ramble.wav joins lost/doubled, final latency, preview lag, energy J/min (macmon, as in "Lightness"). Add the numbers to the Real voice section. If v3 beats v2 on his voice, say so plainly; the default doesn't change without Brandon.

## History

_No activity yet._
