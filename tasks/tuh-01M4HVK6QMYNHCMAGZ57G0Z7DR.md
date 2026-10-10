# Make dictation light: tune the preview until the GPU mostly rests while talking

`tuh-01M4HVK6QMYNHCMAGZ57G0Z7DR`

- **Status:** done
- **Priority:** 1
- **Labels:** `engine` `perf` `energy`
- **Created:** 2026-10-10 02:52 UTC by `brandon`

## Description

Brandon, 2026-10-09: "The preview doesn't need to feel lagless and perfectly live. I want this to be performant and light on the host machine. If the fans turn on a MacBook while recording we went way overkill." Lightness beats preview liveness; the final text's accuracy and speed still matter. Being light and performant is a feature of the product, not a fan-noise threshold.

Today, with the key held, the GPU is busy ~88-92 % of the time and the CPU ~40-45 % of a core (docs/model-bench.md), because the preview re-decodes everything since the last ~30 s cut every 400 ms.

The ask (this is the GPU half of the energy spike, tuh-01M4HDR400G242K017BJ1HB8RB, done first; its runbook and macmon scripts are in that task's description):
1. Measure with macmon (`macmon pipe -i 500`; sum cpu+gpu+ane+ram power x time, idle-subtracted) on recordings/ramble.wav: energy per minute of dictation (J/min) and average added watts, GPU busy %, CPU %, preview lag (median/p95/max), final latency; WER on recordings/structured.wav. Quiet machine for every run.
2. Sweep the existing knobs (--interval, --window, --chunk), e.g. interval 400/800/1200 ms, window cap ~8/12/15 s, chunk 30/15 s. Consider small code changes if they win clearly: an adaptive interval (decode less often the longer you talk) or never re-decoding text that local agreement already committed.
3. Pick the lightest setting that keeps: final text WER and final latency no worse than today; preview words showing up within about 2 s most of the time (working bound; Brandon can loosen it). Make it the default for every model (one pipeline, no per-model cases).
4. Lightness is the goal in itself, not a threshold (Brandon clarified: fans were only an example; "this is a dictation app at the end of the day, so it needs to be light and performant as a feature"). Report sustained added watts and GPU busy % in plain context (vs idle, vs today), and push for as light as the final-text bounds allow. No MacBook fan test is needed.
5. Record Brandon's rule in DESIGN.md requirements, and the numbers in docs/model-bench.md.

Then: decide whether the Neural Engine half is still worth running, against the best GPU setup found here (Brandon's rule: the ANE must at least halve energy per minute vs that).


Added 2026-10-09: first gather every tuning constant that shapes feel or cost (preview interval and window, chunk length, join slack, SpeechGate threshold and settle passes, pause-finder levels and durations, ...) into one core struct with documented defaults (units, what each trades off), so the bench flags and the settings file (tuh-01M4H2EGC4XDQT10EWCXX1STG4, which exposes it under an advanced table) share one source of truth. The bench should be able to set any field (e.g. a generic --set key=value), which also makes the sweep easy.


Done 2026-10-10: step 1 (Tuning struct, --set) in da36537; step 2 in ffa49a5. New defaults: preview_interval_ms 1600 (was 400), new preview_context_ms 1000 (each pass starts 1 s before the confirmed words, not at the last chunk cut); window 20 s and chunk 30 s unchanged. Ramble (v2, macmon, idle-subtracted, 2-4 runs): added power 4.23 -> 0.65 W (GPU 3.56 -> 0.52 W), 254 -> 39 J/min (~6.5x), GPU active 31 -> 6 % (ioreg Device Utilization 91 -> 74 %: a poor cost measure), CPU 36 -> 8 %. Preview lag median/p95/max 0.23/0.73/1.4 -> 0.81/1.94/~4.2 s; first words ~0.5 -> ~1.7 s (follow-up: tuh-01M4JNRW9D3CBNF5X80JPWW7Y9). Final text identical; WER v2 1.68 %, Unified 2.10 %, Whisper 6.72 %, lf_3m_pauses 0 %, lf_10m 0.40 %. Release latency same or better on long dictations; an 11 s clip +25 ms (GPU clocks). Knee 1.2-1.6 s; 20/15 s chunks hurt accuracy; 2-3 s context costs more for no gain. Not built: release cancelling an in-flight pass (transcribe.cpp only checks cancel before a decode; now in flight at release 7 % of the time vs 40 %), adaptive interval. ANE spike parked (see its task).

## History

### 2026-10-10 02:59 UTC — edit by `brandon`

description edited

### 2026-10-10 03:33 UTC — edit by `brandon`

description edited

### 2026-10-10 10:30 UTC — edit by `brandon`

description edited · status open→done
