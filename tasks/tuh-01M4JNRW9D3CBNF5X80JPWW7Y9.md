# Show first words sooner: run the first preview pass early

`tuh-01M4JNRW9D3CBNF5X80JPWW7Y9`

- **Status:** open — ready
- **Priority:** 2
- **Labels:** `engine`
- **Created:** 2026-10-10 10:29 UTC by `brandon`

## Description

After the lightness change (preview every 1600 ms, tuh-01M4HVK6QMYNHCMAGZ57G0Z7DR), first words appear ~1.7 s after the key press instead of ~0.5 s. One early pass is cheap: run the first preview pass as soon as there's enough speech (e.g. ~0.5-0.8 s after the gate first hears speech), then fall back to the normal interval. Add the delay to Tuning (e.g. preview_first_ms) with docs and validation; same for every model.

Acceptance: first words back to ~0.5-0.8 s on the short testdata clips and the ramble; energy per minute on the 5-min ramble within ~5 % of 39 J/min (macmon, quiet machine, as in docs/model-bench.md "Lightness"); final text and WER unchanged; gates and self-test pass.

## History

_No activity yet._
