# Flaky test: session::tests::speech_moves_the_level_meter_at_up_to_its_rate failed once under load

`tuh-01M4NXG7XGT9V9TPESKKJESVBN`

- **Status:** open — ready
- **Priority:** 3
- **Labels:** `test` `flaky`
- **Created:** 2026-10-11 16:42 UTC by `brandon`

## Description

11 October: failed once in cargo test --workspace (load average ~3, the windows-restyle worktree; easyvox-core unchanged since 7b2f6f5), then passed 5 reruns alone. It's timing-based; make it robust to a loaded machine (wider tolerance or a fake clock) rather than retrying.

11 Oct, agent (worktree agent-a5c0f787750044b9e, uncommitted): with 4 niced `yes` the old test failed 3 of 20, always on the count's lower bound (fewer than 4 levels, at 66 ms). In agent shells a 66 ms sleep takes about 206 ms (coalesced timers). Under load the meter ticks less often than a syllable, so every tick sees a whole syllable and the meter barely moves. The spacing check also stamped levels on receipt, so a late test thread could fail it. Fix (test-only): no lower bound (level::tests::syllables_move_it_most_ticks covers it on exact times); spacing now checked on send times (cfg(test) SessionStats::level_sent_at). 20 of 20 under the same load; fmt, clippy, workspace tests pass. Still tied to real-time deadlines, not changed: a_spoken_clip_lifts_the_meter_and_it_rests_after (level > 0.5 before 0.4 s), digital_silence_keeps_the_meter_flat_and_says_it_cant_hear (NoInput in 1.5..1.8 s), a_short_word_is_previewed_soon_after_it_starts_and_decoded (preview pass before the clip ends at 1.3 s).

## History

### 2026-10-11 17:41 UTC — edit by `brandon`

description edited
