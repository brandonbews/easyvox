# Flaky test: session::tests::speech_moves_the_level_meter_at_up_to_its_rate failed once under load

`tuh-01M4NXG7XGT9V9TPESKKJESVBN`

- **Status:** open — ready
- **Priority:** 3
- **Labels:** `test` `flaky`
- **Created:** 2026-10-11 16:42 UTC by `brandon`

## Description

11 October: failed once in cargo test --workspace (load average ~3, the windows-restyle worktree; easyvox-core unchanged since 7b2f6f5), then passed 5 reruns alone. It's timing-based; make it robust to a loaded machine (wider tolerance or a fake clock) rather than retrying.

## History

_No activity yet._
