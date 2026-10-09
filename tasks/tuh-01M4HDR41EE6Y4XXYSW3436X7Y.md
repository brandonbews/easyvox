# Add the long-dictation clip generator to testdata/

`tuh-01M4HDR41EE6Y4XXYSW3436X7Y`

- **Status:** done
- **Priority:** 3
- **Labels:** `bench` `testdata`
- **Created:** 2026-10-09 22:50 UTC by `brandon`

## Description

The long-dictation clips (1/3/5/9-10 min TTS with reference transcripts) and their generator live only in the orchestrator's scratchpad (longform/gen.py, paragraphs.txt). Move the generator and reference text into testdata/ (don't commit the WAVs if large; generate on demand) and document the bench commands.

Done 2026-10-09 in 32b1af7: testdata/longform/ has generate.py (from gen.py; nested clips now cut at fixed paragraph counts 3/8/13/22 so references can't drift with voice updates), paragraphs.txt (written for this project), manifest.tsv (committed references), README.md (generation + bench commands) and a .gitignore for clips/. A --text-only run matched the scratch manifest for all 5 clips. Not copied: the scratch analysis tools (tools/*.py, harness/), which depend on the scratch harness. Follow-up: docs/model-bench.md "Long dictation" still says "32 paragraphs" and "not in the repo"; fixed with the idle commit.

## History

### 2026-10-09 23:49 UTC — edit by `brandon`

description edited · status open→done
