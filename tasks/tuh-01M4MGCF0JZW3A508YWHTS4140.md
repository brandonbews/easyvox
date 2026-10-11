# Website: a condensed, visual page of our benchmark and efficiency findings

`tuh-01M4MGCF0JZW3A508YWHTS4140`

- **Status:** done
- **Priority:** none
- **Labels:** `website` `docs`
- **Created:** 2026-10-11 03:33 UTC by `brandon`

## Description

Brandon's idea (10 Oct): the website gets a condensed, visual digest of our research for someone deciding whether to use EasyVox: model accuracy, latency, energy and idle cost, silence handling, discoveries. Pick the right metrics, keep the caveats honest (one Mac, one speaker), and never include transcripts of recordings/.

Built 11 Oct (agent, uncommitted, in site/): /results with three SVG charts (word errors on the read-aloud passage; first words and text ready; energy per minute and peak memory), all 14 candidates from the 'candidates' study, plus idle stats and the speech detector's stress test as tables, and a 'How we measured' section. Data: src/lib/bench.ts (RFC 4180 parser, per-file schema, a free-text cell guard that never echoes the cell, exact run counts) and src/lib/results.config.ts (studies, builds, models, runs pinned). Per Brandon's addendum: no dates, commits or repo references on the page; model-bench.md and bench/README.md unpublished from nav.ts; homepage reworked (headline 'Simple, local, fast speech-to-text. That's it.', How it works cut, Why EasyVox = Simple/Local/Fast). Every printed number matches docs/model-bench.md. To check: idle.csv's settings-file build (8f3f0b3) reads 113.7 MB after the unload; the doc gives no unloaded figure for it, and the site and DESIGN.md say about 90 MB from the earlier build. Results is 15.4 KB gzipped (HTML + CSS).

Committed as dd4b089 (pushed): /results, the bench docs unpublished, the homepage led by Brandon's pitch. Site gates and the leak check passed. Follow-up: the social images re-rendered with the new headline.

## History

### 2026-10-11 21:58 UTC — edit by `brandon`

status inbox→open

### 2026-10-11 22:26 UTC — edit by `brandon`

description edited

### 2026-10-11 22:29 UTC — edit by `brandon`

description edited

### 2026-10-11 22:29 UTC — edit by `brandon`

status open→done
