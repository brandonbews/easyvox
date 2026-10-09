# Spike reports

These are the reports behind the UI-stack decision (Rust + GPUI Kit). The summary and the decision are in [`../ui-stack-spike.md`](../ui-stack-spike.md).

- [`core.md`](core.md): the speech core (`easyvox-core`): engines, models, live partials, paste, history.
- [`gpui-kit.md`](gpui-kit.md): the GPUI Kit app shell. This is the stack we chose.
- [`tauri.md`](tauri.md): the Tauri v2 app shell. Tauri was dropped; this report is kept for reference.
- `*.png`: snapshots of the spike apps' windows, named after the stack.

They're kept as written, so a few things in them are out of date:

- File and code paths (`native.rs:35-53`, `tauri/MANUAL_TESTS.md`, `../core` and so on) point into the throwaway spike folder, `~/Development/easyvox-spikes`, which is no longer kept. The code that survived now lives in `crates/`, often under new names.
- `reports/<name>.png` means the image of that name in this folder.
- The raw JSON evidence (benchmarks, self-test output, `core-evidence/`) was dropped. The numbers that matter are quoted in the reports.
