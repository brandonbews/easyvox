# Drop or use DecodeMode

`tuh-01M4HDR42WSW44E2TM6C940JJ5`

- **Status:** done
- **Priority:** 4
- **Labels:** `engine` `cleanup`
- **Created:** 2026-10-09 22:50 UTC by `brandon`

## Description

DecodeMode (Partial vs Final) is now ignored by every adapter after the one-pipeline change. Keep it (future use) or remove it.

Finding (2026-10-10, agent): removed. Every adapter took it as `_mode` and ignored it (tcpp.rs, both sherpa.rs models, the session test's Counting model), so `SpeechModel::transcribe` is now `(&self, pcm)`, and the session's preview/final calls and `Engine::transcribe_pcm` just decode. Doc comments that spoke of `Final`-mode and `Partial`-mode decodes (lib.rs TranscriptEvent::Final, session.rs module doc, decode_final) now say final decode / preview. If an adapter ever needs cheaper partials, it can come back as a real setting. Checked with the gates plus `cargo clippy -p easyvox-core --features sherpa --all-targets` (the sherpa adapters aren't built in CI).

Done in cd78c42: removed.

## History

### 2026-10-10 20:39 UTC — edit by `brandon`

description edited

### 2026-10-10 20:46 UTC — edit by `brandon`

description edited · status open→done
