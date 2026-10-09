# Offer the ggml-metal idle-loop patch upstream

`tuh-01M4HDR43KDP1NSH6C16DFH2JJ`

- **Status:** on hold — deliberately paused
- **Priority:** 4
- **Labels:** `human` `engine` `upstream`
- **Created:** 2026-10-09 22:50 UTC by `brandon`

## Description

transcribe.cpp/ggml-metal's residency loop (ggml-metal-device.m ~954-1003, usleep 5 ms) wakes the process ~210x/s while the Metal device exists. A ~40-line semaphore patch would let it sleep when idle with no latency cost. Offer it upstream to handy-computer/transcribe.cpp (and possibly ggml/llama.cpp). Ask Brandon before opening anything upstream.

Update 2026-10-09: GGML_METAL_NO_RESIDENCY=1 showed no measurable latency cost, so EasyVox doesn't need the patch. Offering it upstream is now optional goodwill.

## History

### 2026-10-09 22:50 UTC — edit by `brandon`

status open→held

### 2026-10-09 23:37 UTC — edit by `brandon`

description edited

### 2026-10-09 23:37 UTC — edit by `brandon`

priority 3→4
