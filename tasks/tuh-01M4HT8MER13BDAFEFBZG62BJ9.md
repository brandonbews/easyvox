# Widen the chunk-join slack so soft words at a cut aren't lost

`tuh-01M4HT8MER13BDAFEFBZG62BJ9`

- **Status:** open — ready
- **Priority:** 1
- **Labels:** `engine`
- **Created:** 2026-10-10 02:28 UTC by `brandon`

## Description

Found by the real-voice bench (docs/model-bench.md "Real voice", 9a2e921). On a 5-minute real ramble, words went missing where polished chunks join: v2 lost 1, Unified lost 2 and doubled 1, Whisper lost 1. For v2's loss, the two decodes placed a soft word on opposite sides of the cut, so both dropped it.

An offline replay with SLACK_SECS raised from 0.3 to 0.5 s (crates/easyvox-core, chunk joining) recovered all three Parakeet losses and doubled nothing.

The ask: raise the slack (or make the join smarter), then re-check the joins on recordings/ramble.wav for all three models (count lost/doubled words against a --chunk 0 decode, by reading; never record the text), plus the long TTS clips in testdata/longform/ (WER) and the structured clip (WER). Final latency must not regress.

Acceptance: no lost or doubled words at ramble joins for v2 (or the reason documented); WER not worse anywhere; numbers in docs/model-bench.md.

## History

_No activity yet._
