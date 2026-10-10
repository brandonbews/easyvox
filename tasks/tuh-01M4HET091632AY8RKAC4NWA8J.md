# Make Parakeet TDT v2 the default model

`tuh-01M4HET091632AY8RKAC4NWA8J`

- **Status:** done
- **Priority:** 0
- **Labels:** `models`
- **Created:** 2026-10-09 23:08 UTC by `brandon`

## Description

Brandon decided v2 is the default (decision tuh-01M4HDR3YH6F52Q77NJFQ7VBTM). Switch RECOMMENDED_MODEL and the Offering in crates/easyvox-core/src/models.rs (v2 Default, Unified Alternative), update display names ('fastest first words', 'recommended'), the fetch-model.sh default row (the drift-guard test checks it), DESIGN.md Models section, README, docs/model-bench.md wording, and any app text naming the default. Unified's NVIDIA license notice still applies when it's downloaded.


Done 2026-10-09 in 1544b11: RECOMMENDED_MODEL, Offering::Default and display name ("Parakeet TDT 0.6B v2 (recommended)"; Unified is "Parakeet Unified 0.6B (best public benchmark scores)", Alternative), fetch-model.sh default row (now executable), DESIGN.md Models, README (~539 MB), docs/model-bench.md. Settings are not persisted yet, so nothing to migrate; AppSettings::default() follows RECOMMENDED_MODEL. Gates pass; self-test 15/15 on v2. The NVIDIA notice for Unified stays tuh-01M4H2EGCYFBHHG4S9307GK2W3.

## History

### 2026-10-10 01:05 UTC — edit by `brandon`

description edited · status open→done
