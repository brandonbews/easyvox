# Widen the chunk-join slack so soft words at a cut aren't lost

`tuh-01M4HT8MER13BDAFEFBZG62BJ9`

- **Status:** done
- **Priority:** 1
- **Labels:** `engine`
- **Created:** 2026-10-10 02:28 UTC by `brandon`

## Description

Found by the real-voice bench (docs/model-bench.md "Real voice", 9a2e921). On a 5-minute real ramble, words went missing where polished chunks join: v2 lost 1, Unified lost 2 and doubled 1, Whisper lost 1. For v2's loss, the two decodes placed a soft word on opposite sides of the cut, so both dropped it.

An offline replay with SLACK_SECS raised from 0.3 to 0.5 s (crates/easyvox-core, chunk joining) recovered all three Parakeet losses and doubled nothing.

The ask: raise the slack (or make the join smarter), then re-check the joins on recordings/ramble.wav for all three models (count lost/doubled words against a --chunk 0 decode, by reading; never record the text), plus the long TTS clips in testdata/longform/ (WER) and the structured clip (WER). Final latency must not regress.

Acceptance: no lost or doubled words at ramble joins for v2 (or the reason documented); WER not worse anywhere; numbers in docs/model-bench.md.


Done 2026-10-09 in 39b51e0. Raising the slack alone was marginal (v2's word sat 0.40 s before the cut) and could not fix the doubled word, so the join now matches words (Polished::continuation, MATCH_SECS = 1 s). Ramble lost/doubled: v2 1/0 -> 0/0, Unified 2/1 -> 0/0, Whisper 1/0 -> 1/0 (its chunk decode, without context, dropped the word; fixing needs Whisper word timings). WER unchanged: structured v2 1.68 %, Unified 2.10 %, Whisper 6.72 %; lf_3m/lf_3m_pauses/lf_10m unchanged. Final latency unchanged except when release lands during a running preview pass (waits for it; e.g. Unified 0.51 s once, 0.37 s on rerun) -> looked at in tuh-01M4HVK6QMYNHCMAGZ57G0Z7DR. Forced mid-word cuts (offline, 56/model): doubled 18 -> 1.

## History

### 2026-10-10 04:14 UTC — edit by `brandon`

description edited · status open→done
