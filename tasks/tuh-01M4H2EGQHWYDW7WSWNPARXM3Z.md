# Upgrade transcribe-cpp to 0.4 for Parakeet Ultra (French)

`tuh-01M4H2EGQHWYDW7WSWNPARXM3Z`

- **Status:** on hold — deliberately paused
- **Priority:** 4
- **Labels:** `models` `engine`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Held until French work starts (DESIGN.md "Next steps" 9 and "Stretch goals").

Context: DESIGN.md "Later: French". Parakeet Ultra (Q5_K_M, 550 MB), Moondream's further-trained Parakeet v3, would take TDT v2's slot: about v2's English and the best French of any fast model in transcribe.cpp's runs (4.66 % on FLEURS French). Its support landed after the pinned 0.3.1 (transcribe.cpp PR #192).

The ask: bump transcribe-cpp to 0.4; add an Ultra catalog entry (pinned Hugging Face commit, sha256, CC-BY-4.0 crediting Moondream); bench it in English and French; while there, re-check whether 0.4 adds number formatting (ITN) for Canary.

Acceptance: all engine tests pass on 0.4 with no regression for the default; Ultra's numbers in docs/model-bench.md; scripts/fetch-model.sh and its parity test updated.

Pointers: crates/easyvox-core/src/models.rs, tcpp.rs, Cargo.toml; scripts/fetch-model.sh; crates/easyvox-core/tests/fetch_script.rs.

## History

_No activity yet._
