# Flaky test: download range_ignored sometimes restarts from 0

`tuh-01M4K8HZ2NA22EC4M7XF1RXBEK`

- **Status:** open — ready
- **Priority:** 2
- **Labels:** `tests` `downloads`
- **Created:** 2026-10-10 15:57 UTC by `brandon`

## Description

crates/easyvox-core/tests/download.rs range_ignored failed once (2026-10-10, gates run right after a long bench) at the assert resumed_at > 0 (line 518): the retry started from byte 0 instead of resuming. Passed on 3 reruns and in CI. Likely a race between the simulated cut at 1 MiB and the .part flush under load. Find the race; make the test deterministic rather than retrying it.

## History

### 2026-10-10 18:17 UTC — edit by `brandon`

status inbox→open
