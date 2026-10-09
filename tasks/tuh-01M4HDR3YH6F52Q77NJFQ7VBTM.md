# Decide the default model: Parakeet TDT v2 or Unified

`tuh-01M4HDR3YH6F52Q77NJFQ7VBTM`

- **Status:** done
- **Priority:** 0
- **Labels:** `human` `decision` `models`
- **Created:** 2026-10-09 22:50 UTC by `brandon`

## Description

Brandon's call. The one-pipeline bench (docs/model-bench.md) favours Parakeet TDT v2: same CPU and first-word time as Unified now that native streaming is gone, half the errors on long dictation (0.40 vs 0.86 % at 9 min), always ends with a full stop (Unified dropped it on 4 of 13 clips), cleaner word timings for chunk cuts, CC-BY-4.0 license. Against: Unified ranks slightly better on public benchmarks, and TTS clips can't settle that. On approval: switch RECOMMENDED_MODEL and the Offering in models.rs, update display names ('fastest first words', 'recommended'), fetch-model.sh default, DESIGN.md, README.

Decision (2026-10-09, Brandon): make Parakeet TDT v2 the default; Unified becomes an alternative.

## History

### 2026-10-09 22:50 UTC — edit by `brandon`

status open→held

### 2026-10-09 23:08 UTC — edit by `brandon`

description edited · status held→done
