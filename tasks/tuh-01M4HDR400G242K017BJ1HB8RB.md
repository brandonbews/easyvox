# Energy spike: GPU vs cheaper preview vs Neural Engine

`tuh-01M4HDR400G242K017BJ1HB8RB`

- **Status:** open — blocked on dependencies
- **Priority:** 2
- **Labels:** `engine` `perf` `energy` `spike`
- **Depends on:** [`tuh-z7dr`](tuh-01M4HVK6QMYNHCMAGZ57G0Z7DR.md) (open)
- **Created:** 2026-10-09 22:50 UTC by `brandon`

## Description

Measure energy per minute of dictation on the same 5-min recording (real voice if available, else the long TTS clips): (P0) today's ggml-Metal with the idle fix, plus a cheaper preview; (P1) Parakeet encoder on the ANE via the glimpse-hq transcribe.cpp fork (coreml feature); (P2) FluidAudio CLI as a pure Core ML reference; (P3) Apple SpeechAnalyzer via a small Swift script. Tools without sudo: macmon pipe (brew), top -stats idlew,power, rusage v6 ri_energy_nj. Rule: move Parakeet to Core ML/ANE (pure-Rust objc2-core-ml adapter, ~2 weeks) only if it at least halves energy per minute. Research: ane-research artifact https://claude.ai/artifact/FdvUCLeFA2pd35qhy1k8dS


Decided by Brandon 2026-10-09: (1) "halves" is measured against the best GPU setup of equal accuracy and speed (e.g. the same short windows), not today's settings; (2) uv is installed (Homebrew, uv 0.12.24) for the coremltools conversion; macmon 0.9.0 is installed too.

---
RUNBOOK (2026-10-09, read-only research agent; scripts are in its appendix)

# Energy spike plan: GPU vs cheaper preview vs Neural Engine

Prepared 9 October 2026 for ledger task `tuh-01M4HDR400G242K017BJ1HB8RB`. Research only: nothing was built, benchmarked or edited. The only things run were a 6 s and a 15 s `macmon` sample, `macmon debug`, dry runs of the scripts below with `/usr/bin/true` standing in for the bench, and `git clone` of two repos into this scratchpad.

- **Decision rule (fixed by the user, not reopened):** move Parakeet to the Neural Engine (ANE) only if it at least halves energy per minute of dictation. Apple's SpeechAnalyzer is out of scope. The extra checks come from the earlier research and still apply: real-voice WER within +0.3 points, median finalize ≤ 0.5 s, no idle regression.
- **Machine:** Mac mini M4 (`Mac16,10`), 16 GB, 6E + 4P cores, 10-core GPU, macOS 26.6.2 (25G83), Xcode 27.0 (27A266a), on AC, Low Power Mode off, no battery. A Mac mini has no battery, so the earlier "battery cross-check" can't be done. The whole-machine cross-check is SMC `PSTR` (macmon `sys_power`) instead.
- **Workload:** `recordings/ramble.wav`, 299.5 s, 16 kHz mono int16, replayed in real time through `easyvox-bench session`. Accuracy uses `recordings/structured.wav` (81.0 s) with `recordings/structured-reference.txt`.
- **Model:** Parakeet TDT 0.6B v2 Q5_K_M (`parakeet-tdt-0.6b-v2-q5_k_m-gguf`, now the default as of commit `1544b11`). File: `~/Library/Application Support/EasyVox/models/gguf/parakeet-tdt-0.6b-v2-Q5_K_M.gguf` (514 MiB).
- **Scripts:** drafted and dry-run tested in `energy-kit/` in this scratchpad: `quiet.sh`, `run1.sh`, `rotate.sh`, `energy.py`, `summary.py`, `agree.py`, `scrub.py`. Copies are in the appendix. `/private/tmp` is cleared on reboot, so the appendix is the durable copy.

`SP` below means `/private/tmp/claude-501/-Users-agentbox-Development-easyvox/762f138d-0e08-4db4-a3c0-3333bd6b1846/scratchpad`.

---

## 0. The short version

1. **Measure with `macmon pipe -i 500`** (no sudo). Each JSON line gives `cpu_power`, `gpu_power`, `ane_power` and `ram_power` (DRAM) in watts, averaged over the interval since the previous line, plus `sys_power` (whole machine), temperatures and fan speed. Multiply each sample's power by the time since the previous sample and sum: that's joules. The headline is net SoC energy per minute of audio: (CPU + GPU + ANE + DRAM) during the session, minus the idle baseline over the same time, divided by 4.99 minutes.
2. **Configurations:**
   - **A:** today's GPU path.
   - **B\*:** the best cheaper preview. Every knob it needs already exists as a bench flag: `--interval`, `--window`, `--chunk`.
   - **C0:** the glimpse-hq fork on the GPU, in the ANE-compatible pipeline shape. This is the control.
   - **C:** the fork with the encoder on the ANE and the decoder on Metal.
   - **C2:** the same, with the decoder on the CPU, so the GPU stays off.
   - **C3 (optional):** C2 with an int8, multi-size encoder.

   Three runs each, in rotated order, each run gated on a quiet, cool machine. Add runs until the 90 % CI of the ratio clears 0.5.
3. **The ANE path is a small spike, not a port.** The fork is MIT-licensed and keeps the crate version at 0.3.1. Our Rust adapter needs one changed line (`Model::capabilities()` now returns `Result`). A `[patch.crates-io]` entry and the `coreml` feature finish the job. Two environment variables then choose the path without any more code: `TRANSCRIBE_PARAKEET_COREML_MODEL=<encoder>.mlmodelc` and `TRANSCRIBE_BACKENDS=cpu`. The encoder is converted locally from our exact Q5_K_M GGUF with the fork's Python script. That needs Python 3.11–3.13 and coremltools 9.0, and **neither `uv` nor a Python ≥ 3.11 is on this Mac yet**. Rough cost: half a day of prep, 3.5–4.5 h with the machine quiet, and 1–2 h of analysis. About 1.5 days in all.
4. **Biggest risks:**
   - The ANE pass has a fixed 15 s window. Longer input silently falls back to the GPU, so the plan caps the preview window at 14 s and chunks at 6.5 s, and counts any fallbacks.
   - The fork also changes the decoder, hence the C0 control.
   - Core ML may quietly run stages on the CPU. Check `ane_power`.
   - Another session on this Mac starts Xcode builds. The quiet gate and the foreign-CPU flag handle it.

---

## 1. What gets compared

All runs use v2 Q5_K_M on `ramble.wav`, with the bench's `session` command (real-time replay, live preview, chunks polished while talking, the rest decoded on release).

| ID | Binary | Environment | Bench flags | What it answers |
|---|---|---|---|---|
| **A** | stock (crates.io transcribe-cpp 0.3.1, main at `1544b11`) | – | `--interval 400 --window 20 --chunk 30` (the defaults, written out) | Today's GPU path, with the gate and the idle fixes. The number to beat. |
| B1 | stock | – | `--interval 800` | Half the preview passes. |
| B2 | stock | – | `--window 10` | Half the preview window once it's full. |
| B3 | stock | – | `--chunk 15` | Shorter chunks. The preview window restarts at each polished chunk, so this also shortens preview windows. |
| B4 | stock | – | `--interval 800 --window 10 --chunk 15` | The knobs combined. |
| B5 | stock | – | `--window 14 --chunk 6.5` | The ANE-compatible shape on today's crate. Comparing it with C0 isolates the fork's own changes. |
| **B\*** | stock | – | the winner of B1–B5 (see 6.6) | The cheap alternative to the ANE: a one-line change to the defaults in `EngineConfig::new`. |
| **C0** | fork (`coreml` feature built in) | none | `--window 14 --chunk 6.5` | Control: the fork on the GPU, same pipeline shape as C. |
| **C** | fork | `TRANSCRIBE_PARAKEET_COREML_MODEL=$ENC_FP16` | `--window 14 --chunk 6.5` | Encoder on the ANE (fp16, 4 chained programs), decoder on Metal. |
| **C2** | fork | as C, plus `TRANSCRIBE_BACKENDS=cpu` | `--window 14 --chunk 6.5` | Encoder on the ANE, decoder on the CPU: no GPU at all. This is how Glimpse ships it. The likely best ANE number. |
| C3 (optional) | fork | `TRANSCRIBE_PARAKEET_COREML_MODEL=$ENC_INT8MF TRANSCRIBE_BACKENDS=cpu` | `--window 14 --chunk 6.5` | int8 encoder with smaller 10.24 s and 5.12 s functions, so short windows don't pay for a full 15 s pass. |

**Why the ANE runs need `--window 14 --chunk 6.5`.** The fork's Core ML encoder has a fixed capacity of 1501 mel frames, which is 240,000 samples or 15.0 s (NeMo framing: frames = samples/160 + 1). Longer input silently falls back to the ggml encoder on the GPU or CPU: `parakeet/model.cpp` ~line 1262 logs a WARN, and our adapter turns logging off. The bound with these settings:

- A preview pass decodes at most 14 s.
- The chunker cuts once ≥ 6.5 s are waiting, and forces a cut by 2 × 6.5 s at the latest. At one tick per 400 ms the longest chunk is ≈ 12.9 s; with 1 s of context on each side, that's ≤ 14.9 s.
- The tail decoded on release is < 13.4 s, plus 1 s of context.

So nothing should exceed 15 s, and the spike adds a counter to prove it (6.3).

**Why C0 is needed.** The fork is upstream main from after 0.3.1, plus 58 Glimpse commits. Some of them speed up the Parakeet decoder even on the GPU: per-step weights kept quantized, each LSTM cell fused into one graph node, vDSP confidence. The pipeline shape also changes between A and C. C ÷ C0 is the pure effect of moving the encoder. C ÷ A is what a user would see. C0 ÷ B5 is the effect of the fork itself. Its ggml is identical to 0.3.1's (0.25.3; `diff -rq ggml/src` is empty), so the residency fix (`GGML_METAL_NO_RESIDENCY`) behaves the same.

**Which baseline the rule uses: a question for the user (section 8).** The shorter windows the ANE needs are themselves a cheaper preview, and B4 or B5 may save a lot on the GPU alone. The plan reports every ratio, and recommends judging the ANE against the best GPU configuration at equal quality.

---

## 2. macmon 0.9.0

Installed at `/opt/homebrew/bin/macmon`. Checked here against macmon's v0.9.0 source (`src/metrics.rs`, `src/sources.rs`) and a live run.

### Command
```sh
macmon pipe -i 500 > run.jsonl &      # one JSON object per line, about every 0.5-0.65 s, until killed
macmon pipe -i 500 -s 30 > probe.jsonl  # or a fixed number of samples (-s 0 = forever)
macmon pipe -i 500 -s 1 --soc-info      # adds a "soc" block: chip, core counts, frequency tables
```
- `-i` is the interval in ms (minimum 100). On this Mac `-i 500` produced lines 0.51–0.65 s apart, because each sample also reads the SMC, temperatures and memory.
- **Samples are contiguous.** The sampler keeps its IOReport baseline between calls and reports energy over the whole interval since the previous sample (`get_sample_interval`). So integrating Σ Pᵢ·(tᵢ − tᵢ₋₁) with consecutive `timestamp`s is exact energy. Drop the first line, which spans macmon's start-up.
- **No sudo.** It ran as `agentbox` without sudo. `macmon debug` shows the IOReport "Energy Model" channels `CPU Energy`, `GPU Energy`, `ANE`, `DRAM` and `GPU SRAM` subscribed on macOS 26.6.2, plus SMC `PSTR`.
- Its own cost: about 2 % of one core at `-i 500` (0.12 s of CPU in 6.4 s). It's the same in every run and in the baseline.

### Fields (watts unless noted)
| Field | Meaning | Use |
|---|---|---|
| `cpu_power` | CPU package (Energy Model "CPU Energy", all clusters) | rail |
| `gpu_power` | GPU ("GPU Energy") | rail |
| `ane_power` | Neural Engine ("ANE") | rail; must be > 0 for C/C2/C3 |
| `ram_power` | DRAM | rail |
| `gpu_ram_power` | GPU SRAM | reads 0.000 on this M4; ignore |
| `all_power` | `cpu_power + gpu_power + ane_power`. **Excludes DRAM** | don't use as the total |
| `sys_power` | SMC `PSTR`, whole-machine power (SSD, fan, I/O too), floored at `all_power`. Updates about once a second and lags (it moved in steps of ~1–2 s in the probe) | cross-check over whole runs only |
| `timestamp` | RFC 3339 UTC, taken when the sample is returned | integration |
| `temp.cpu_temp_avg`, `temp.gpu_temp_avg` | °C (the GPU sensor reads ~0.8 when the GPU is off) | thermal check |
| `fans[].rpm` | fan speed (999 rpm at idle here, max 4900) | thermal check |
| `gpu_active_ratio`, `gpu_freq_mhz`, `cpu_usage_pct`, `ecpu_*`, `pcpu_*` | residency and frequency | sanity checks |

**Headline rail:** `soc = cpu_power + gpu_power + ane_power + ram_power`.

**Caveats:**
- macmon's code says Energy Model CPU and ANE counters can freeze on macOS 27 without Apple's entitlement, and prefers CLPC counters where readable. That doesn't matter on 26.6.2.
- `sys_power` is an SMC estimate, not a wall meter.

**Indicative magnitudes from the probes:** idle `cpu_power` 0.03–0.4 W, `ram_power` ~0.03–0.1 W, GPU and ANE 0. During the 15 s probe, another agent was running an `easyvox-bench` session (configuration unknown): GPU 4.0 W, CPU 1.3 W mean, CPU temperature 55.8 °C. So expect A at roughly 5–6 W net, around 300–400 J per minute of audio. This is only a sanity range.

---

## 3. Measurement protocol

### One run (`run1.sh`)
1. Start `macmon pipe -i 500` → `LABEL.jsonl`. Start a `ps` sampler every 2 s (processes over 0.5 % CPU) → `LABEL.ps`.
2. **60 s pre-baseline**: machine idle, no bench running.
3. Launch the bench: `easyvox-bench session --model parakeet-tdt-0.6b-v2-q5_k_m-gguf --wav ramble.wav --idle-before 20 --quiet --out LABEL.json [config flags]`. This does:
   - the model load, which includes the Core ML load for C*;
   - a warm-up decode of 1 s of silence;
   - **20 s loaded and idle**;
   - the **real-time session**: 299.5 s of audio, then the final pass.
4. **45 s post-baseline**.
5. Write `LABEL.meta.json`: wall-clock marks, load averages, `pmset -g therm` before and after, the binary's sha256, env, args, OS and macmon version.

### Windows and integration (`energy.py`)
- Sample i covers (tᵢ₋₁, tᵢ] at constant power Pᵢ. The energy in a window is Σ Pᵢ × overlap.
- **Session window:** s1 = process exit − 0.15 s, and s0 = s1 − `cpu.wall_secs`. `cpu.wall_secs` is the bench's own wall time from session start to Final plus join. It includes the final pass after the audio ends, which is part of a dictation's cost.
- **Baseline:** P_base = mean(pre, post) per rail. It's measured right before and right after each run, so slow drift cancels. A run is flagged when |post − pre| on soc > 0.15 W.
- **Net energy:** E_net = E_session − P_base × T_session.
- **Headline:** J/min = E_net(soc) / (audio_secs / 60). Also reported:
  - J/min from `sys_power`;
  - gross J/min;
  - mean net W per rail;
  - **loaded-idle overhead**: mean soc power over the last 15 s of the `--idle-before` window, minus P_base, in mW;
  - start and peak CPU temperature, peak fan speed, mean GPU active ratio;
  - the bench's finalize ms, passes, skipped, median gap, revision rate, chunks, WER (with `--ref`) and CPU %.
- The baseline matters little: idle soc is ~0.1–0.3 W against ~5 W while dictating, about 3 % of the session energy. **Contamination matters a lot**: an Xcode build draws 10–20 W. Hence the next two parts.

### Quiet machine (`quiet.sh`, before every run)
A run starts only when all of these hold:
- 1-minute load average < 2.0. This Mac idles around 1.8 with the usual daemons; tighten it if the calibration in 6.5 shows less.
- No process matching a build, indexing or benchmark pattern above 5 % CPU, and nothing at all above 20 %. The pattern covers `xcodebuild`, `XCBBuildService`, `swift-frontend`, `clang`, `ld`, `cargo`, `rustc`, `cmake`, `coremlcompiler`, `ANECompilerService`, `easyvox-bench`, `EasyVox`, `mds_stores`, `backupd` and others.
- A 15 s macmon probe shows:
  - mean CPU < `CPU_W_MAX`, and no sample > `CPU_W_PEAK` (set both from the calibration);
  - GPU < 0.05 W and ANE < 0.02 W;
  - CPU temperature within ±1.5 °C of `TEMP_REF`.
- `pmset -g therm` reports no thermal or performance warning.

It retries every 30 s and gives up after 30 min.

Tested here: it correctly refused while another agent's `easyvox-bench` was running, and its probe path reported `cpu=1.30W cpu_peak=2.95W gpu=4.00W temp=55.8C`.

### During the run: contamination flag
`energy.py` sums the CPU % of every process other than the bench, macmon and the samplers over the session window. It flags the run when the mean exceeds 10 % of one core and prints the top three foreign processes. **Flagged runs are dropped and re-run.** Two of the earlier preview runs had to be dropped for overlapping an Xcode build, so expect this.

### Thermal state
The Mac mini has a fan: it idles near 1000 rpm, and the CPU idles around 38–45 °C. Power rises with temperature through leakage, so:
- `TEMP_REF` is the median idle `cpu_temp_avg` from the calibration. Each run waits until the CPU is back within 1.5 °C of it, which usually takes 2–4 min after a GPU-heavy run. `quiet.sh` does the waiting.
- Record the start temperature, peak temperature and peak fan speed per run. If J/min correlates with start temperature within a configuration (r > 0.8 across its runs), say so in the write-up.
- `pmset -g therm` is logged before and after every run. Any recorded thermal or performance warning invalidates the run.
- The rotated order spreads slow ambient drift across all configurations.

### Repetitions and order
- **Screening (B1–B5):** one run each on `ramble.wav`, plus one on `structured.wav` for quality. This is only to pick B\*.
- **Final:** **3 clean runs** each of A, B\*, C0, C and C2 (and C3 if done), in rotated order. Round r starts at configuration r, so every configuration sits in every position: `rotate.sh` does this.
- **Stopping rule:** `summary.py` gives each ratio with a 90 % CI (Welch t on log J/min).
  - If the CI for the deciding ratio lies entirely below 0.5 ("halves") or entirely above it ("does not halve"), stop.
  - If it straddles 0.5, add rounds (up to 5 runs per configuration).
  - Also add runs for any configuration whose CV is > 5 %.
- **Why 3 is enough:** a fixed 5-minute replay on a quiet machine should vary by a few percent run to run. At CV 3 % with n = 3 each, the ratio's 90 % CI is about ±6 %. Only a result within ~0.47–0.53 would need more runs.
- **Time per run:** about 8.5 min (60 + ~25 + 300 + 45 s, plus cool-down). The final block is 15–18 runs, so 2.1–2.6 h.

### Privacy
- The bench JSON contains `final_text` and `partials`/`confirmed_at_stop` text from the private recording. **`LABEL.json` files never leave the scratchpad.**
- `--quiet` keeps transcript text out of stderr.
- Every analysis script prints numbers only. `agree.py` compares two transcripts as word edit distances without printing words.
- After the write-up, scrub the text fields (6.10).
- Never copy, upload or delete anything in `recordings/`.

---

## 4. Preview knobs: what exists, what needs code

The app builds its engine with `EngineConfig::recommended()`, which uses the defaults in `crates/easyvox-core/src/lib.rs` (`EngineConfig::new`): `partial_interval_ms: 400`, `partial_window_secs: 20.0`, `chunk_secs: 30.0`, `threads: 4`. **There is no app setting or environment variable for these.** Shipping a B\* means changing those defaults (one line each), plus DESIGN.md.

### Exist today (bench flags, no code)
| Flag | Field | Effect on preview cost | Side effects |
|---|---|---|---|
| `--interval MS` | `partial_interval_ms` (0 = no preview) | passes ∝ 1/interval: 705–709 passes per 5 min at 400 ms | First words and confirmations come later. Local agreement needs two agreeing passes, so confirmed text trails by ~2 intervals. Updates come in bigger jumps. |
| `--window SECS` | `partial_window_secs` (min 2) | cost per pass ∝ window length once the window is full | Less context for the preview; more revisions. The final text is unaffected. |
| `--chunk SECS` | `chunk_secs` (0 = one pass on release) | After each polished chunk the preview window restarts at the cut (`session.rs`: `anchor = polished.start() - context`), so smaller chunks shorten the average preview window | More seams: check WER and lost or doubled words at joins. Chunks are 0.5–2× the target. |
| `--threads N` | `threads` | CPU threads for ggml work on the CPU | Minor; an optional extra screen. |
| `EASYVOX_METAL_RESIDENCY=1` | ggml residency sets on | idle wake-ups only | Not a preview knob. |

### Ships today but can't be tuned or turned off
- **SpeechGate** (`pause.rs`) is on in every binary since `d33ab36`, so it's part of A, B and C alike.
  - `SETTLE_PASSES = 2`, `QUIET_RATIO = 0.1` (−20 dB) and `SILENCE_RMS = 0.0018` (−55 dBFS) are constants.
  - Measured on the ramble, it skipped only 10–13 of ~720 passes (under 2 %). An offline replay says even zero settle passes would skip only 17 %.
  - So it needs no configuration of its own here. If someone wants a "before the gate" point, the pre-gate bench from the idle work is at `.../9b441e66-.../scratchpad/idle/base/easyvox-bench`, if `/private/tmp` hasn't been cleared. It accepts `--model`.

### Would need a small code change (not needed for this spike)
| Idea | Where | Size | Expected value |
|---|---|---|---|
| Gate on/off or tuning by env or config (`SETTLE_PASSES`, `QUIET_RATIO`) | `pause.rs`, `EngineConfig`, bench flag | ~10–15 lines | Low: ≤ 17 % of passes on real speech |
| Adaptive interval: back off to 800–1200 ms while the provisional text stops changing, snap back to 400 on a change | `session.rs` record loop, `EngineConfig`, bench flag | ~20–30 lines | Unknown: continuous speech changes most passes. Keeps first-words latency, unlike a flat `--interval 800`. |
| A cheaper decode for preview passes | `tcpp.rs` (`DecodeMode` is ignored today: `_mode`) | – | Nothing to switch in transcribe-cpp 0.3.1 for Parakeet: decoding is greedy TDT already, and local agreement needs word timings |
| Native streaming (Unified) | removed adapter in git history | large | Out: a per-model path, against "one pipeline for every model" |

---

## 5. The glimpse-hq transcribe.cpp fork

### What it is
- **Repo:** <https://github.com/glimpse-hq/transcribe.cpp>, a GitHub fork of `handy-computer/transcribe.cpp`. Cloned here at `$SP/spike/glimpse-transcribe.cpp` (blobless clone, with an `upstream` remote added).
- **Pin:** `ec5f294de55f0c421ffbaf2fb1275e1a607a3be7` (8 Oct 2026, "Merge upstream-merge: … main through 95d82e51"). It is 58 commits ahead of upstream main and 1 behind.
- **Maintainer:** the Glimpse dictation app (tryglimpse.cc). 0 stars, issues disabled, two merged PRs (#1, Glimpse 1.3.0 engine, 27 Sep; #2, Glimpse 1.3.1 engine, 3 Oct). Very active: pin the commit.
- **License:** MIT ("Copyright (c) 2026 The transcribe.cpp authors"), same as upstream. The derived encoder inherits the model's license: Parakeet TDT v2 is CC-BY-4.0 (NVIDIA). coremltools is BSD-3. That's fine for a local spike.

### What it changes (relevant parts)
- **Core ML encoder companion:** `src/transcribe-coreml.{h,mm}` (259 lines of ObjC++), behind CMake `TRANSCRIBE_COREML=ON` or the Rust feature `coreml`. Links CoreML.framework and Foundation.
  - `MLComputeUnitsCPUAndNeuralEngine`: the GPU is excluded from the encoder. Core ML may still use CPU ops.
  - The session loads the encoder once and reuses it. It honours output strides.
  - On macOS 15 and later it picks the smallest function of a multifunction model that fits the input.
- **Parakeet hook** (`src/arch/parakeet/model.cpp`):
  - The mel spectrogram is still computed on the host.
  - If mel frames ≤ capacity, the encoder runs on Core ML and its output feeds the existing TDT decoder on the selected ggml backend. Otherwise the ggml encoder runs, with a WARN.
  - Only offline, full-context, batch-norm, 8× subsampling encoders are accepted. v2 qualifies. Streaming and multitalker models are rejected.
- **Decoder speed-ups that also apply on the GPU:** quantized per-step LSTM/joint weights, one fused graph node per LSTM cell, closed-form vDSP token confidence, a stuck-TDT-frame guard. Plus upstream's post-0.3.1 work (roles, generic prompting, Parakeet Ultra, a long ChunkedLimited attention optimisation for Unified, serde errors). **Reason for C0.**
- Other additions we don't use: Whisper and Qwen3-ASR Core ML encoders, Whisper word timestamps, Parakeet phrase boosting and beam search (only when phrases are set), Nemotron-3 diarization, decoder-only GGUFs (V3 only).

### API compatibility with transcribe-cpp 0.3.1
- Both crates in the fork are still versioned **0.3.1**, so `[patch.crates-io]` resolves. The C ABI hash differs (`d40abf3e…` vs `57b1af43…`), but the wrapper and library come from the same fork, so that doesn't matter.
- **One compile break in our code:** `Model::capabilities()` now returns `Result<Capabilities>`. `tcpp.rs` line 73 (`model.capabilities().max_timestamp_kind`) needs a `?`.
- Everything else we call is unchanged:
  - `Model::load`, `session_with`, `SessionOptions { n_threads, ..Default::default() }` (it gains `coreml_encoder_path: Option<PathBuf>`, which is source-compatible);
  - `RunOptions { timestamps, ..Default::default() }`, `Session::run` and `Transcript` (`result.rs` identical);
  - `disable_logging` (`logging.rs` identical) and the `TimestampKind` variants.
- **No separate C++ harness is needed.** The spike runs in our exact pipeline through the existing bench.
- **Choosing the path needs no code.** The fork reads `TRANSCRIBE_PARAKEET_COREML_MODEL` when the session option is unset (`session_coreml_encoder_path` in `transcribe-arch.cpp`). `TRANSCRIBE_BACKENDS=cpu` (in 0.3.1 too) restricts ggml to the CPU, which moves the decoder off Metal.
- **Misconfiguration fails loudly.** A wrong path, a variant mismatch, or the env var set on a build without `coreml` all make session init fail, so `Engine::load` errors out.

### How the Core ML encoder is produced (v2 needs a local conversion)
- Glimpse publishes companions on Hugging Face (`Glimpse-Dictation/…`) for TDT **V3**, Unified, Ultra, Whisper and Qwen3-ASR, **but not v2**. The runtime checks the variant, so a V3 encoder can't be used.
- The converter is `scripts/convert-parakeet-gguf-to-coreml.py` (437 lines, PEP 723 header: Python ≥3.11,<3.14; `coremltools==9.0`, `gguf>=0.17`, `numpy>=1.26`, `scikit-learn`).
  - It reads the GGUF directly and dequantizes the `enc.*` tensors with `gguf.dequantize`. gguf-py implements numpy dequantization for Q5_K and Q6_K, the types inside Q5_K_M.
  - It builds a MIL program and converts it to an fp16 `mlprogram` (macOS 13 target).
  - The default is **4 chained stages** for a 0.6B encoder, because the ANE compiler rejects a single 1.2 GB program and silently runs it on the CPU.
  - It writes the variant and the source GGUF's sha256 into the metadata.
- Options:
  - `--max-frames` (default 1501);
  - `--int8` (per-channel weights, a single program, roughly half the size);
  - `--palettize-bits 6|8`;
  - `--extra-frames N…` (smaller multifunction encoders; needs `--int8` or palettization);
  - `--compile` (runs `xcrun coremlcompiler compile`, present in Xcode 27 at `…/XcodeDefault.xctoolchain/usr/bin/coremlcompiler`).
- **Files the spike needs:**
  - the existing `parakeet-tdt-0.6b-v2-Q5_K_M.gguf`, which supplies the decoder, vocabulary and mel settings;
  - one compiled directory, `v2-q5km-fp16.mlmodelc`. Expect ~1.1–1.2 GB, like Glimpse's fp16 companions (1.09 GB zipped). The int8 multifunction one is ~0.5–0.6 GB.
- Both encoders come from our exact Q5_K_M file. They carry the Q5_K_M weights, rounded to fp16, so they are like-for-like with the GPU path.
- **First load compiles for the ANE:** about 12 s on an M2 Pro (fork docs), then about 0.1 s from cache. The cache is per app and per path, and is thrown away by OS updates.

### Build steps (macOS 26.6 / Xcode 27 / M4), for later and only on a quiet machine
1. `brew install uv` (needs the user's OK). Then `uv run --python 3.11` fetches a standalone Python and the converter's wheels on first use (~0.3 GB download).
2. Convert from a clean directory, using a copy of the script (6.2).
3. Create a spike worktree from `main`, add the `[patch.crates-io]` entry and the `coreml` feature, and fix `capabilities()` (6.3). Then `cargo build -p easyvox-bench --release`.
   - cmake is at `/opt/homebrew/bin/cmake`. The fork's build script passes `-DTRANSCRIBE_COREML=ON`, compiles the `.mm` with `-fobjc-arc` and exports `-framework CoreML` to cargo through its link manifest (`cmake/transcribe-install.cmake`).
   - A clean build recompiles ggml and transcribe.cpp: expect 3–6 min.
4. No Metal toolchain is needed: ggml embeds its shader source. A new binary compiles its Metal shaders on first model load (~14 s, once).

### Known problems and limits
- **15 s capacity and silent fallback** to the ggml encoder above it, with a WARN our adapter hides. Handled by the pipeline shape and the overcap counter.
- **Fixed-shape cost.** The fp16 encoder pays for a full 15 s pass even for a 2 s window. With `--extra-frames 1024 512` (int8 only) short inputs use a smaller function. The fork says the ANE "handles up to 128 encoder frames (10.24 s) about twice as fast as the 188 of a 15 s window". That's C3.
- **fp16 numerics:** "exact transcript equality with ggml is not guaranteed".
  - The fork's own checks matched ggml transcripts for v2 on a 3 s clip, the 1501-frame boundary and the fallbacks.
  - At the boundary, two other variants lost a comma or a cut-off final word.
- **"Not a guarantee of ANE-only execution or energy savings"** (fork docs). Core ML can place stages on the CPU without saying so. The check is macmon: `ane_power` well above 0 and no CPU jump. Our build exposes no `MLComputePlan`. An Instruments Core ML trace (`xcrun xctrace record --template 'Core ML' --launch -- …`) is an optional deeper check.
- **Fork risk:** one maintainer, no issue tracker, frequent force-merges of upstream. Pin the SHA. Inside a dictation product, though, it ships in Glimpse 1.3.x with ANE encoders, and PR #2 reports a seeded LibriSpeech and FLEURS regression gate.
- Its docs contradict themselves on Unified (one paragraph says Unified offline is supported, another says it isn't). Irrelevant for v2.
- **macOS 27:** ANE memory starts counting in the process footprint. The background-ANE entitlement is unverified for menu-bar apps. macmon's Energy Model counters may freeze. None of this applies on 26.6.2.

### Difficulty: about 1.5 days end to end
| Step | Effort | Needs a quiet machine? |
|---|---|---|
| Install uv; convert the fp16 encoder, and optionally the int8 one | 30–60 min (the conversion itself 5–15 min each; CPU and RAM heavy) | No other benchmark running |
| Spike worktree, ~25 lines of changes, build | 30–45 min | No other build or benchmark running |
| Validation (load, ANE active, no overcap, short-clip transcripts, structured WER) | 1–1.5 h | Partly |
| Calibration, B screening, final rotation | 3.5–4.5 h | **Yes** |
| Analysis, write-up, ledger | 1–2 h | No |

---

## 6. Runbook

Follow it top to bottom. Coordinate with the orchestrator first: **no other cargo build, Xcode build or benchmark may overlap steps 6.2–6.8**. The other session's occasional Xcode builds are caught by `quiet.sh` and by the foreign-CPU flag.

### 6.0 Variables (paste into every shell)
```sh
export SP=/private/tmp/claude-501/-Users-agentbox-Development-easyvox/762f138d-0e08-4db4-a3c0-3333bd6b1846/scratchpad
export KIT=$SP/energy-kit                     # scripts (appendix has copies)
export OUT=$SP/energy/runs                     # all run outputs (private: contain transcripts)
export REPO=/Users/agentbox/Development/easyvox
export FORK=$SP/spike/glimpse-transcribe.cpp
export FORK_SHA=ec5f294de55f0c421ffbaf2fb1275e1a607a3be7
export GGUF="$HOME/Library/Application Support/EasyVox/models/gguf/parakeet-tdt-0.6b-v2-Q5_K_M.gguf"
export RAMBLE=$REPO/recordings/ramble.wav
export STRUCT=$REPO/recordings/structured.wav
export STRUCT_REF=$REPO/recordings/structured-reference.txt
export BIN_STOCK=$SP/energy/bin/easyvox-bench-stock
export BIN_FORK=$SP/energy/bin/easyvox-bench-fork
export ENC_FP16=$SP/ane/enc/v2-q5km-fp16.mlmodelc
export ENC_INT8MF=$SP/ane/enc/v2-q5km-int8mf.mlmodelc
export M=parakeet-tdt-0.6b-v2-q5_k_m-gguf
mkdir -p $OUT $SP/energy/bin $SP/ane/enc $SP/ane/tools
```
If `/private/tmp` was cleared:
- Recreate `$KIT` from the appendix and `chmod +x` the `.sh` files.
- Re-clone the fork: `git clone --filter=blob:none https://github.com/glimpse-hq/transcribe.cpp.git $FORK && git -C $FORK checkout $FORK_SHA`.

### 6.1 Freeze the stock binary (A, B)
```sh
cd $REPO && git status --short && git log -1 --oneline     # expect a clean main (1544b11 or later)
source "$HOME/.cargo/env" && cargo build -p easyvox-bench --release --locked
cp target/release/easyvox-bench $BIN_STOCK && shasum -a 256 $BIN_STOCK
```
Copying freezes it, so another agent's rebuild can't change the binary mid-experiment. Record the commit as `SRC_REV` for `run1.sh`: `export SRC_REV_STOCK=$(git -C $REPO rev-parse --short HEAD)`.

### 6.2 Convert the encoder(s)
Read `$FORK/scripts/convert-parakeet-gguf-to-coreml.py` first: it's untrusted code from the clone. Then run a copy from a clean directory, so nothing in the clone can shadow an import:
```sh
brew install uv                                   # only after the user OKs it (section 8)
git -C $FORK rev-parse HEAD                       # must print $FORK_SHA
cp $FORK/scripts/convert-parakeet-gguf-to-coreml.py $SP/ane/tools/convert.py
cd $SP/ane/enc
/usr/bin/time -l uv run --python 3.11 --script $SP/ane/tools/convert.py "$GGUF" \
    --output $SP/ane/enc/v2-q5km-fp16.mlpackage --compile        # -> v2-q5km-fp16.mlmodelc
# Optional C3:
/usr/bin/time -l uv run --python 3.11 --script $SP/ane/tools/convert.py "$GGUF" \
    --output $SP/ane/enc/v2-q5km-int8mf.mlpackage --int8 --extra-frames 1024 512 --compile
du -sh $SP/ane/enc/*
```
- Record the wall time and peak memory (`/usr/bin/time -l`).
- Expect "Built stage 1/4 … 4/4" for fp16 and one program for int8.
- **If `gguf.dequantize` fails on a tensor type**, convert from Handy's Q8_0 v2 GGUF instead. It's pinned in `docs/model-bench.md`: `handy-computer/parakeet-tdt-0.6b-v2-gguf` @ `382796e4…`, sha256 `f0d0e99c…`. The variant check still passes. Note in the results that the encoder then carries Q8_0 weights while the decoder GGUF is Q5_K_M.

### 6.3 Build the fork bench in a spike worktree
Never on `main`, and never merged.
```sh
git -C $REPO worktree add $SP/wt-ane -b spike/ane-energy
cd $SP/wt-ane
```
Edits:
1. Root `Cargo.toml`, appended:
   ```toml
   # SPIKE ONLY: glimpse-hq fork of transcribe.cpp with Core ML encoders (pinned in $FORK).
   [patch.crates-io]
   transcribe-cpp = { path = "/private/tmp/claude-501/-Users-agentbox-Development-easyvox/762f138d-0e08-4db4-a3c0-3333bd6b1846/scratchpad/spike/glimpse-transcribe.cpp/bindings/rust/transcribe-cpp" }
   transcribe-cpp-sys = { path = "/private/tmp/claude-501/-Users-agentbox-Development-easyvox/762f138d-0e08-4db4-a3c0-3333bd6b1846/scratchpad/spike/glimpse-transcribe.cpp" }
   ```
   A git source works too: `{ git = "https://github.com/glimpse-hq/transcribe.cpp", rev = "ec5f294de55f0c421ffbaf2fb1275e1a607a3be7" }` for both.
2. `crates/easyvox-core/Cargo.toml`: `transcribe-cpp = { version = "0.3.1", optional = true, features = ["coreml"] }`.
3. `crates/easyvox-core/src/tcpp.rs`:
   - Line 73 becomes `let max_timestamps = model.capabilities().map_err(|e| anyhow::anyhow!("transcribe.cpp capabilities: {e}"))?.max_timestamp_kind;`.
   - Add an over-capacity counter:
     ```rust
     pub static OVERCAP_DECODES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
     // first thing in SpeechModel::transcribe:
     if pcm.len() > 240_000 && std::env::var_os("TRANSCRIBE_PARAKEET_COREML_MODEL").is_some() {
         OVERCAP_DECODES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
     }
     ```
   - Re-export it from `lib.rs` (`#[cfg(feature = "transcribe-cpp")] pub fn tcpp_overcap_decodes() -> u32 { tcpp::OVERCAP_DECODES.load(Relaxed) }`).
   - Optionally, `if std::env::var_os("EASYVOX_TCPP_LOG").is_none() { transcribe_cpp::disable_logging(); }`, so validation runs can see "Core ML encoder loaded" and any capacity WARN on stderr. Keep it off in energy runs.
4. `crates/easyvox-bench/src/main.rs`, `session_cmd`: `v["tcpp_overcap_decodes"] = json!(easyvox_core::tcpp_overcap_decodes()); v["coreml_encoder"] = json!(std::env::var("TRANSCRIBE_PARAKEET_COREML_MODEL").ok());`. `energy.py` flags any run with overcap > 0.

```sh
source "$HOME/.cargo/env" && cargo build -p easyvox-bench --release     # not --locked: the lockfile changes
cp target/release/easyvox-bench $BIN_FORK && shasum -a 256 $BIN_FORK
git add -A && git commit -m "Spike: glimpse-hq transcribe.cpp fork with Core ML encoder (not for main)"
export SRC_REV_FORK=$(git rev-parse --short HEAD)
```
The commit stays on the spike branch only. Don't push it unless the orchestrator asks.

### 6.4 Validate before measuring (machine quiet-ish; short runs)
```sh
cd $REPO
T=$REPO/testdata/cmd_samantha.wav
# Load + first ANE compile (cold), then cached; text is a TTS test clip (not private).
TRANSCRIBE_PARAKEET_COREML_MODEL=$ENC_FP16 $BIN_FORK transcribe --model $M --wav $T --out $SP/ane/load-cold.json
TRANSCRIBE_PARAKEET_COREML_MODEL=$ENC_FP16 $BIN_FORK transcribe --model $M --wav $T --out $SP/ane/load-warm.json
$BIN_FORK transcribe --model $M --wav $T --out $SP/ane/load-ggml.json
python3 -I -c "import json; [print(f, json.load(open('$SP/ane/'+f))['load_ms']) for f in ('load-cold.json','load-warm.json','load-ggml.json')]"
```
- **Load time:** cold `load_ms` includes the ANE compile (expect ~10–60 s). Warm should be ~0.1–0.5 s more than ggml's. Record both: they are the "preparing" cost users would see after each macOS update.
- **Short-clip transcripts:**
  ```sh
  for e in ane ggml; do
    env $( [ $e = ane ] && echo TRANSCRIBE_PARAKEET_COREML_MODEL=$ENC_FP16 ) $BIN_FORK bench --model $M --reps 1 --out $SP/ane/clips-$e.json 2>/dev/null
  done
  python3 -I -c "
  import json; a=json.load(open('$SP/ane/clips-ane.json')); g=json.load(open('$SP/ane/clips-ggml.json'))
  same=sum(x['hypothesis']==y['hypothesis'] for x,y in zip(a['clips'],g['clips']))
  print('wer ane %.2f%% ggml %.2f%%, identical %d/%d' % (100*a['wer_total'],100*g['wer_total'],same,len(a['clips'])))"
  ```
  The two clips over 15 s (`long_daniel` 30.1 s, `long_samantha` 34.3 s) fall back to ggml here. That's expected in `bench`, which decodes whole clips.
- **ANE really used, GPU off, no fallback:** one short measured session per ANE configuration, with the runs kept out of the final set:
  ```sh
  OUT=$SP/energy/validate PRE=15 IDLE=10 POST=15 REF=$STRUCT_REF $KIT/run1.sh C2-v $BIN_FORK $STRUCT \
      TRANSCRIBE_PARAKEET_COREML_MODEL=$ENC_FP16 TRANSCRIBE_BACKENDS=cpu -- --window 14 --chunk 6.5
  OUT=$SP/energy/validate PRE=15 IDLE=10 POST=15 REF=$STRUCT_REF $KIT/run1.sh C-v $BIN_FORK $STRUCT \
      TRANSCRIBE_PARAKEET_COREML_MODEL=$ENC_FP16 -- --window 14 --chunk 6.5
  python3 -I $KIT/energy.py $SP/energy/validate
  python3 -I -c "import json; print(json.load(open('$SP/energy/validate/C2-v.json')).get('tcpp_overcap_decodes'))"   # must be 0
  ```
  Pass criteria:
  - C2: `W_ane` clearly > 0 (expect 0.2–2 W), `W_gpu` ≈ 0, `tcpp_overcap_decodes` = 0, structured WER close to A's 2.1 %.
  - If `W_ane` ≈ 0 while `W_cpu` jumps, Core ML put the encoder on the CPU. Stop and investigate: try `--stages 6`, or the int8 single program, before measuring anything.
  - C: `W_gpu` > 0 is expected (the decoder runs on Metal).
- **Preview keeps up:** in the C2 JSON, `partials.pass_ms_mean` should be well under 400 ms and `median_gap_ms` close to A's. If the CPU decoder is too slow, the preview falls behind. Report it: that would rule C2 out on latency, whatever its energy.

### 6.5 Idle calibration (quiet machine)
```sh
macmon pipe -i 500 -s 600 > $SP/energy/idle-calib.jsonl    # ~6 min, nothing else running
python3 -I - $SP/energy/idle-calib.jsonl <<'EOF'
import json, statistics as st, sys
r = [json.loads(l) for l in open(sys.argv[1])][1:]
c = [x["cpu_power"] for x in r]; s = [x["cpu_power"]+x["gpu_power"]+x["ane_power"]+x["ram_power"] for x in r]
t = [x["temp"]["cpu_temp_avg"] for x in r]
print("cpu mean %.3f sd %.3f p99 %.3f | soc mean %.3f | temp median %.1f" % (st.mean(c), st.pstdev(c), sorted(c)[int(.99*len(c))], st.mean(s), st.median(t)))
EOF
export CPU_W_MAX=...   # cpu mean + 3 sd, at least 0.2
export CPU_W_PEAK=...  # max(1.0, 2 x p99)
export TEMP_REF=...    # temp median
```

### 6.6 Screen the cheaper preview (quiet machine, ~50 min)
```sh
cat > $SP/energy/screen.conf <<EOF
A|$BIN_STOCK|$RAMBLE||--interval 400 --window 20 --chunk 30
B1|$BIN_STOCK|$RAMBLE||--interval 800
B2|$BIN_STOCK|$RAMBLE||--window 10
B3|$BIN_STOCK|$RAMBLE||--chunk 15
B4|$BIN_STOCK|$RAMBLE||--interval 800 --window 10 --chunk 15
B5|$BIN_STOCK|$RAMBLE||--window 14 --chunk 6.5
EOF
OUT=$SP/energy/screen SRC_REV=$SRC_REV_STOCK $KIT/rotate.sh $SP/energy/screen.conf 1
# quality on the read-aloud recording, same configs (short; energy numbers here are rough):
sed "s#|$RAMBLE|#|$STRUCT|#; s#^\([A-Z0-9]*\)|#\1s|#" $SP/energy/screen.conf > $SP/energy/screen-struct.conf
OUT=$SP/energy/screen REF=$STRUCT_REF PRE=30 POST=20 $KIT/rotate.sh $SP/energy/screen-struct.conf 1
python3 -I $KIT/energy.py $SP/energy/screen
```
**Choosing B\*:** the B variant with the lowest J/min whose structured run (`B?s-r1`) has:
- WER ≤ A's + 0.3 points;
- `finalize_ms` ≤ 500;
- `first_partial_ms` ≤ A's + 450 ms, i.e. at most about one extra tick;
- `median_gap_ms` ≤ 1000 on the ramble.

Also note its `revision_rate` and `chunks`. How the preview feels (bigger jumps, later confirmations) is the user's call: report the numbers and say a hands-on check would follow if B\* is adopted.

### 6.7 Final rotation (quiet machine, ~2.5 h)
```sh
cat > $SP/energy/final.conf <<EOF
A|$BIN_STOCK|$RAMBLE||--interval 400 --window 20 --chunk 30
Bx|$BIN_STOCK|$RAMBLE||<flags of B*>
C0|$BIN_FORK|$RAMBLE||--window 14 --chunk 6.5
C|$BIN_FORK|$RAMBLE|TRANSCRIBE_PARAKEET_COREML_MODEL=$ENC_FP16|--window 14 --chunk 6.5
C2|$BIN_FORK|$RAMBLE|TRANSCRIBE_PARAKEET_COREML_MODEL=$ENC_FP16 TRANSCRIBE_BACKENDS=cpu|--window 14 --chunk 6.5
# C3|$BIN_FORK|$RAMBLE|TRANSCRIBE_PARAKEET_COREML_MODEL=$ENC_INT8MF TRANSCRIBE_BACKENDS=cpu|--window 14 --chunk 6.5
EOF
$KIT/rotate.sh $SP/energy/final.conf 3            # resumable: finished labels are skipped
python3 -I $KIT/energy.py $OUT && python3 -I $KIT/summary.py $OUT/energy.csv A Bx C0
```
- Re-run a flagged run under a new label. `summary.py` already leaves flagged runs out, and groups runs by the part before `-r`. For example:
  ```sh
  $KIT/quiet.sh && $KIT/run1.sh C-r4 $BIN_FORK $RAMBLE TRANSCRIBE_PARAKEET_COREML_MODEL=$ENC_FP16 -- --window 14 --chunk 6.5
  ```
- If the deciding ratio is "undecided", run rounds 4–5: `$KIT/rotate.sh $SP/energy/final.conf 5 4`.
- The ENC path contains no spaces, so it can live in the env field. If you move it somewhere with spaces, make a symlink.
- Use `SRC_REV` per configuration if you want it in the meta: `rotate.sh` passes the environment through.

### 6.8 Accuracy and "same transcripts" checks (quiet machine, ~30 min)
```sh
cat > $SP/energy/acc.conf <<EOF
A|$BIN_STOCK|$STRUCT||--interval 400 --window 20 --chunk 30
Bx|$BIN_STOCK|$STRUCT||<flags of B*>
C0|$BIN_FORK|$STRUCT||--window 14 --chunk 6.5
C|$BIN_FORK|$STRUCT|TRANSCRIBE_PARAKEET_COREML_MODEL=$ENC_FP16|--window 14 --chunk 6.5
C2|$BIN_FORK|$STRUCT|TRANSCRIBE_PARAKEET_COREML_MODEL=$ENC_FP16 TRANSCRIBE_BACKENDS=cpu|--window 14 --chunk 6.5
EOF
OUT=$SP/energy/acc REF=$STRUCT_REF PRE=20 POST=15 $KIT/rotate.sh $SP/energy/acc.conf 2
python3 -I $KIT/energy.py $SP/energy/acc | cut -f1,11,13,14        # label, finalize, WER, flags
# Word-level agreement without printing words (ramble has no reference):
python3 -I $KIT/agree.py $OUT/C0-r1.json $OUT/C-r1.json $OUT/C2-r1.json    # pure encoder effect: expect >= 99 % agreement
python3 -I $KIT/agree.py $OUT/A-r1.json $OUT/C0-r1.json $OUT/Bx-r1.json    # pipeline-shape + fork effect
python3 -I $KIT/agree.py $SP/energy/acc/C0-r1.json $SP/energy/acc/C-r1.json $SP/energy/acc/C2-r1.json
```
Report:
- structured WER per configuration, from the bench's own `--ref` WER;
- disagreement % on the ramble and on structured, C/C2 vs C0, and C0/B\* vs A;
- median `finalize_ms`;
- `first_partial_ms` (structured);
- `median_gap_ms` and `revision_rate` (ramble);
- `chunks`.

**ANE acceptance** (earlier research's rules):
- structured WER ≤ A's + 0.3 points;
- median finalize ≤ 0.5 s;
- the loaded-idle overhead (`mW_loaded_idle_soc`) no worse than C0's or A's.

Optional, if the decision is close or the user asks:
- **Long idle:** `IDLE=600 LOADED_S=580 $KIT/run1.sh A-idle …` and `C2-idle …` with `testdata/cmd_samantha.wav` as the WAV. Gives loaded-idle mW over ~10 min. Add `top -l 0 -s 5 -stats pid,idlew,cpu -pid <bench pid>` for wake-ups.
- **First dictation after 5 min idle:** `IDLE=300` on `structured.wav`. Look at `first_partial_ms` and `finalize_ms`, which show the ANE wake-up cost.

### 6.9 Compute and present
1. `python3 -I $KIT/summary.py $OUT/energy.csv A Bx C0` gives a table per configuration and ratios with 90 % CIs against A, against B\* and against C0.
2. **The decision line:**
   - Headline ratio: best ANE configuration (C2, or C/C3 if better) ÷ the baseline the user picks (section 8; recommended: the best GPU configuration of equal quality, min(A, B\*)).
   - "Halves" requires the whole 90 % CI ≤ 0.50 **and** the ANE acceptance checks in 6.8.
   - Also state C2 ÷ A (the literal rule) and C ÷ C0 (encoder placement alone).
3. **Table to publish (numbers only):**
   - per configuration: n, J/min (soc) mean ± sd, J/min (sys), net W by rail (CPU, GPU, ANE, DRAM), loaded-idle mW;
   - structured WER, ramble disagreement vs C0 and vs A, median finalize, first words, preview median gap, passes, chunks;
   - start and peak temperature;
   - the encoder's size on disk, its cold and warm load times, and the conversion time.

   A stacked bar of net W by rail per configuration reads well; follow the `dataviz` skill if you draw it.
4. **Publish** a styled HTML artifact for the user, as they prefer: numbers only, no transcript text, and a link to the ledger task.
5. **Write back:**
   - Append a numbers-only summary to the ledger task: read with `tuhdoo task <id>`, append, then `tuhdoo update <id> --desc -`. `--desc` replaces the whole description.
   - If the rule is met, create a follow-up for the pure-Rust `objc2-core-ml` adapter (~2 weeks, per the research). Otherwise create one for adopting B\* (defaults change, DESIGN.md, hands-on feel check, labelled `hands-on`).
   - Add an "Energy" section to `docs/model-bench.md` with the method and numbers. That's a repo edit, so it goes through the orchestrator.

### 6.10 Clean up
- Scrub transcript text from the result JSONs once `agree.py` is done. It keeps every number:
  ```sh
  python3 -I $KIT/scrub.py $SP/energy/*/*.json $SP/ane/*.json
  grep -l '"final_text"\|"hypothesis"' $SP/energy/*/*.json $SP/ane/*.json    # expect no output
  ```
- `git -C $REPO worktree remove $SP/wt-ane`. Keep the `spike/ane-energy` branch until the user decides, and ask before deleting it.
- Leave `recordings/` untouched.
- Make sure no `macmon` or `ps` sampler is left running: `pgrep -fl macmon`.

---

## 7. Risks and how the plan handles them

| Risk | Effect | Handling |
|---|---|---|
| Inputs over 15 s silently fall back to ggml on the ANE build | ANE energy looks worse, and on C2 the fallback encoder runs on the CPU (slow) | `--window 14 --chunk 6.5`, the overcap counter, a flag in `energy.py` |
| Core ML runs a stage on the CPU (ANE compiler rejection) | C/C2 show high `cpu_power`, `ane_power` ≈ 0 | Validation in 6.4 before any measurement; fallback `--stages 6` or int8 |
| The fork's decoder and upstream changes confound C vs A | ANE gets credit for decoder speed-ups | C0 control; C0 vs B5 isolates the fork |
| Shorter windows and chunks are a cheaper preview in themselves | C2 vs A over-credits the ANE | Report C ÷ C0 and C2 ÷ min(A, B\*); question 1 |
| CPU decoder too slow (C2) | Preview lags; latency fails | `pass_ms_mean`, `median_gap_ms`, `finalize_ms` checks; C (Metal decoder) as the alternative |
| Background builds or indexing (the other session's Xcode) | +10–20 W bursts | `quiet.sh` before each run; foreign-CPU flag; drop and re-run |
| Thermal drift | Leakage changes power by a few % | Cool-down to `TEMP_REF` ± 1.5 °C; rotation; per-run temperatures logged; `pmset -g therm` checks |
| fp16 vs Q5_K_M numerics | Transcript differences | WER on structured; agreement C vs C0 on both recordings; short-clip identity count |
| `gguf.dequantize` or coremltools 9.0 trouble on this setup | Can't convert | Q8_0 GGUF fallback (6.2); coremltools needs Python 3.11–3.13, via uv |
| `/private/tmp` cleared on reboot | Kit, clone and encoders lost | Appendix has the scripts; the conversion is reproducible from the pinned fork SHA |
| Measuring on a Mac mini, not a laptop | No battery numbers | SoC rails are the relevant comparison; `sys_power` as the whole-machine check |
| Privacy | Transcripts in JSON | Scratch only, `--quiet`, numbers-only scripts, scrub at the end |

**Not covered by this plan, on purpose:** the ledger task also lists P2 (FluidAudio CLI as a pure Core ML reference) and P3 (Apple SpeechAnalyzer). The user scoped this spike to three configurations, and SpeechAnalyzer is explicitly out. FluidAudio could still serve later as an upper bound for an all-Core-ML pipeline, if C2 lands near the threshold.

---

## 8. Questions for the user (with recommended answers)

1. **Which baseline does "halves" use?** Today's path (A), or the GPU configuration that would ship otherwise? The ANE forces a 14 s window and 6.5 s chunks, which on their own make the GPU cheaper (B5, C0).
   - **Recommended:** decide on best-ANE ÷ the best GPU configuration with equal quality (min of A and B\*), and show A alongside. Otherwise the ANE gets credit for savings a one-line defaults change gives for free.
   - The rule itself (≥ 2×) stays as set.
2. **OK to install `uv` with Homebrew** and let it download Python 3.11 plus coremltools 9.0 and friends (~0.3 GB) for the one-off encoder conversion?
   - **Recommended:** yes. It's the fork's documented route, and it keeps the system Python untouched.
   - The alternative is `brew install python@3.12` plus a venv.

---

## Appendix: the scripts (copies of `$SP/energy-kit/`)

Recreate them with a heredoc per file if `/private/tmp` was cleared, then `chmod +x $KIT/*.sh`. All Python runs on the system `python3` (3.9) with `-I`.

### `quiet.sh`

```bash
#!/bin/bash
# quiet.sh [MAX_WAIT_MIN]: wait until this Mac is quiet enough for an energy run.
# Exits 0 when quiet, 1 on timeout. One status line per check, on stdout.
#
# "Quiet" means all of:
#   - 1-minute load average under LOAD_MAX;
#   - no build/indexing/benchmark process using more than 5 % of a core, and nothing else
#     above 20 % of a core (ps);
#   - a 15 s macmon probe: mean CPU power under CPU_W_MAX, no sample over CPU_W_PEAK,
#     GPU and ANE essentially idle;
#   - CPU temperature back within TEMP_TOL of TEMP_REF (when TEMP_REF is set);
#   - pmset reports no thermal or performance warning.
# Calibrate CPU_W_MAX / CPU_W_PEAK from the idle calibration (runbook step 2.1).
set -u
MAX_MIN=${1:-30}
LOAD_MAX=${LOAD_MAX:-2.0}
CPU_W_MAX=${CPU_W_MAX:-0.35}
CPU_W_PEAK=${CPU_W_PEAK:-1.5}
GPU_W_MAX=${GPU_W_MAX:-0.05}
ANE_W_MAX=${ANE_W_MAX:-0.02}
TEMP_REF=${TEMP_REF:-}
TEMP_TOL=${TEMP_TOL:-1.5}
BUSY_RE=${BUSY_RE:-'xcodebuild|XCBBuildService|SWBBuildService|swift-frontend|swift-build|swiftc|clang|/ld$|ld64|cargo|rustc|cc1|cmake|ninja|/make$|ibtool|actool|coremlcompiler|ANECompilerService|easyvox-bench|EasyVox|mds_stores|mdworker|backupd|softwareupdated|bird$|photoanalysisd'}

deadline=$(( $(date +%s) + MAX_MIN * 60 ))
probe=$(mktemp -t quietprobe)
trap 'rm -f "$probe"' EXIT

while :; do
  why=""
  load=$(sysctl -n vm.loadavg | awk '{print $2}')
  awk -v l="$load" -v m="$LOAD_MAX" 'BEGIN{exit !(l < m)}' || why+="load=$load "

  busy=$(ps -A -o pcpu=,comm= | awk -v re="$BUSY_RE" '($1 > 5 && $0 ~ re) || $1 > 20 {printf "%s(%s%%) ", $2, $1}')
  [[ -n "$busy" ]] && why+="busy: $busy"

  therm=$(pmset -g therm)
  if ! grep -q "No thermal warning level" <<<"$therm" || ! grep -q "No performance warning level" <<<"$therm"; then
    why+="pmset-therm-warning "
  fi

  if [[ -z "$why" ]]; then
    macmon pipe -i 500 -s 30 > "$probe" 2>/dev/null
    verdict=$(python3 -I - "$probe" "$CPU_W_MAX" "$CPU_W_PEAK" "$GPU_W_MAX" "$ANE_W_MAX" "${TEMP_REF:-nan}" "$TEMP_TOL" <<'EOF'
import json, math, sys
path, cmax, cpeak, gmax, amax, tref, ttol = sys.argv[1], *map(float, sys.argv[2:])
rows = [json.loads(l) for l in open(path) if l.strip()][1:]   # first sample spans macmon's start-up
if not rows:
    print("no-macmon-samples"); sys.exit()
mean = lambda k: sum(r[k] for r in rows) / len(rows)
cpu, gpu, ane = mean("cpu_power"), mean("gpu_power"), mean("ane_power")
peak = max(r["cpu_power"] for r in rows)
temp = rows[-1]["temp"]["cpu_temp_avg"]
bad = []
if cpu >= cmax: bad.append("cpu=%.2fW" % cpu)
if peak >= cpeak: bad.append("cpu_peak=%.2fW" % peak)
if gpu >= gmax: bad.append("gpu=%.2fW" % gpu)
if ane >= amax: bad.append("ane=%.2fW" % ane)
if not math.isnan(tref) and abs(temp - tref) > ttol: bad.append("temp=%.1fC(ref %.1f)" % (temp, tref))
print(" ".join(bad) if bad else "OK cpu=%.2fW peak=%.2fW gpu=%.2fW ane=%.3fW temp=%.1fC" % (cpu, peak, gpu, ane, temp))
EOF
)
    if [[ "$verdict" == OK* ]]; then
      echo "$(date '+%H:%M:%S') quiet: load=$load $verdict"
      exit 0
    fi
    why+="$verdict"
  fi

  echo "$(date '+%H:%M:%S') waiting: $why"
  if (( $(date +%s) >= deadline )); then
    echo "gave up after $MAX_MIN min"
    exit 1
  fi
  sleep 30
done
```

### `run1.sh`

```bash
#!/bin/bash
# run1.sh LABEL BIN WAV [VAR=value ...] [-- extra bench args]
#
# One measured run, all logged under $OUT (default: $SP/energy/runs):
#   1. macmon pipe (500 ms) and a ps sampler (2 s) start;
#   2. PRE seconds of machine idle (no bench running): the baseline;
#   3. the bench: model load, warm-up decode, IDLE s with the model loaded and idle
#      (--idle-before), then the real-time replay of WAV through the session, to its Final;
#   4. POST seconds of machine idle: the drift check.
# Files: LABEL.jsonl (macmon), LABEL.ps (ps), LABEL.json (bench result: contains the final
# transcript, so it never leaves the scratchpad), LABEL.err (bench stderr), LABEL.meta.json.
#
# Example:
#   run1.sh A-r1 "$BIN_STOCK" "$RAMBLE" -- --interval 400 --window 20 --chunk 30
#   run1.sh C2-r1 "$BIN_FORK" "$RAMBLE" TRANSCRIBE_PARAKEET_COREML_MODEL="$ENC" TRANSCRIBE_BACKENDS=cpu -- --window 14 --chunk 6.5
set -u
# macOS /bin/bash is 3.2: empty arrays are expanded as ${a[@]+"${a[@]}"} under set -u.
SP=${SP:-/private/tmp/claude-501/-Users-agentbox-Development-easyvox/762f138d-0e08-4db4-a3c0-3333bd6b1846/scratchpad}
OUT=${OUT:-$SP/energy/runs}
MODEL=${MODEL:-parakeet-tdt-0.6b-v2-q5_k_m-gguf}
PRE=${PRE:-60}
IDLE=${IDLE:-20}
POST=${POST:-45}
REF=${REF:-}            # e.g. REF=/Users/agentbox/Development/easyvox/recordings/structured-reference.txt

L=$1; BIN=$2; WAV=$3; shift 3
ENVS=(); ARGS=()
while (($#)); do
  if [[ $1 == -- ]]; then shift; ARGS=("$@"); break; fi
  ENVS+=("$1"); shift
done
mkdir -p "$OUT"
for f in "$OUT/$L".{jsonl,ps,json,err,meta.json}; do
  [[ -e $f ]] && { echo "refusing to overwrite $f"; exit 2; }
done

now() { python3 -I -c 'import time; print("%.3f" % time.time())'; }
snap() { echo "\"load\": \"$(sysctl -n vm.loadavg)\", \"therm\": $(pmset -g therm | python3 -I -c 'import json,sys; print(json.dumps(sys.stdin.read()))')"; }

macmon pipe -i 500 > "$OUT/$L.jsonl" 2>/dev/null & MM=$!
( while :; do echo "T $(now)"; ps -A -o pid=,pcpu=,comm= | awk '$2 > 0.5'; sleep 2; done ) > "$OUT/$L.ps" 2>/dev/null & PSS=$!
trap 'kill $MM $PSS 2>/dev/null' EXIT

t_pre0=$(now); before=$(snap)
sleep "$PRE"
t_launch=$(now)
refargs=(); [[ -n $REF ]] && refargs=(--ref "$REF")
env ${ENVS[@]+"${ENVS[@]}"} "$BIN" session --model "$MODEL" --wav "$WAV" --idle-before "$IDLE" --quiet \
  --out "$OUT/$L.json" ${refargs[@]+"${refargs[@]}"} ${ARGS[@]+"${ARGS[@]}"} 2> "$OUT/$L.err" &
BP=$!
wait $BP; rc=$?
t_exit=$(now)
sleep "$POST"
t_post1=$(now); after=$(snap)
kill $MM $PSS 2>/dev/null; wait $MM $PSS 2>/dev/null

python3 -I - "$OUT/$L.meta.json" <<EOF
import json, sys
meta = {
  "label": "$L", "bin": "$BIN", "bin_sha256": "$(shasum -a 256 "$BIN" | cut -c1-16)",
  "wav": "$(basename "$WAV")", "model": "$MODEL", "env": $(printf '%s\n' ${ENVS[@]+"${ENVS[@]}"} | python3 -I -c 'import json,sys; print(json.dumps([l for l in sys.stdin.read().splitlines() if l]))'),
  "args": $(printf '%s\n' ${ARGS[@]+"${ARGS[@]}"} | python3 -I -c 'import json,sys; print(json.dumps([l for l in sys.stdin.read().splitlines() if l]))'),
  "ref": bool("$REF"), "pre_s": $PRE, "idle_s": $IDLE, "post_s": $POST,
  "t_pre0": $t_pre0, "t_launch": $t_launch, "t_exit": $t_exit, "t_post1": $t_post1,
  "bench_pid": $BP, "rc": $rc, "src_rev": "${SRC_REV:-}",
  "before": {$before}, "after": {$after},
  "lowpowermode": "$(pmset -g | awk '/lowpowermode/{print $2}')",
  "os": "$(sw_vers -productVersion) $(sw_vers -buildVersion)", "macmon": "$(macmon --version)",
}
json.dump(meta, open(sys.argv[1], "w"), indent=1)
EOF
echo "$L rc=$rc session json: $OUT/$L.json"
exit $rc
```

### `rotate.sh`

```bash
#!/bin/bash
# rotate.sh CONFIGS ROUNDS [FIRST_ROUND]: run every config ROUNDS times in rotated order
# (round r starts at config r), waiting for a quiet machine before each run.
#
# CONFIGS: one config per line, fields separated by "|":
#   NAME|BIN|WAV|VAR=value VAR=value|bench args
# Blank lines and lines starting with # are skipped. Paths must not contain "|" or spaces
# in the env/args fields (the WAV and BIN fields may contain spaces).
# Labels are NAME-rN; an existing label is skipped, so a stopped schedule can be resumed.
set -u
KIT=$(cd "$(dirname "$0")" && pwd)
SP=${SP:-/private/tmp/claude-501/-Users-agentbox-Development-easyvox/762f138d-0e08-4db4-a3c0-3333bd6b1846/scratchpad}
OUT=${OUT:-$SP/energy/runs}
CONF=$1; ROUNDS=$2; FIRST=${3:-1}

lines=()
while IFS= read -r line; do
  [[ -z "$line" || "$line" == \#* ]] && continue
  lines+=("$line")
done < "$CONF"
n=${#lines[@]}

for ((r = FIRST; r <= ROUNDS; r++)); do
  for ((k = 0; k < n; k++)); do
    line=${lines[$(( (k + r - 1) % n ))]}
    IFS='|' read -r name bin wav envs args <<<"$line"
    label="$name-r$r"
    if [[ -e "$OUT/$label.meta.json" ]]; then echo "skip $label (done)"; continue; fi
    "$KIT/quiet.sh" "${QUIET_MAX_MIN:-30}" || { echo "machine not quiet; stopping before $label"; exit 1; }
    # shellcheck disable=SC2086  # envs and args are deliberately word-split
    "$KIT/run1.sh" "$label" "$bin" "$wav" $envs -- $args
    python3 -I "$KIT/energy.py" "$OUT" "$label" | tail -1
  done
done
```

### `energy.py`

```python
"""energy.py RUN_DIR [LABEL ...]: energy per run from macmon samples. Prints numbers only.

For each LABEL (default: every *.meta.json in RUN_DIR) it reads LABEL.jsonl (macmon pipe),
LABEL.meta.json (wall-clock marks from run1.sh), LABEL.json (bench session result) and
LABEL.ps (ps samples), and writes RUN_DIR/energy.csv plus a table on stdout.

How the numbers are made:
- macmon's sampler keeps its IOReport baseline between samples, so each sample's power is
  the average over the whole interval since the previous one. Sample i therefore covers
  (t[i-1], t[i]]; energy in a window is the sum of power x overlap. Sample 0 is dropped.
- Windows (unix seconds):
    pre      [t_pre0 + 5, t_launch - 1]       machine idle, before the bench starts
    loaded   [s0 - LOADED_S, s0 - 1]          bench loaded and idle (--idle-before; LOADED_S=15)
    session  [s0, s1]                         s1 = t_exit - EXIT_PAD, s0 = s1 - cpu.wall_secs
    post     [t_exit + 10, t_post1 - 1]       machine idle again
  cpu.wall_secs is the bench's own session wall time (start of replay to Final + join).
- Rails: cpu, gpu, ane, dram (macmon ram_power), sys (SMC PSTR, whole machine, slow).
  soc = cpu + gpu + ane + dram is the headline rail.
- Baseline = mean of pre and post power. net = session energy - baseline x session time.
- J/min = net soc energy / (audio_secs / 60).
- Flags: drift (|post - pre| soc > 0.15 W), foreign CPU (other processes > 10 % of a core on
  average during the session), rc != 0, bench errors, Core ML over-capacity decodes.
"""
import csv, glob, json, os, re, sys
from datetime import datetime

EXIT_PAD = 0.15   # seconds between Final + join and process exit (WER calc, JSON write, exit)
LOADED_S = float(os.environ.get("LOADED_S", "15"))   # loaded-idle window length; IDLE=600 runs: LOADED_S=580
RAILS = {"cpu": "cpu_power", "gpu": "gpu_power", "ane": "ane_power", "dram": "ram_power", "sys": "sys_power"}
SELF = re.compile(r"(macmon|/ps$|/awk$|/sleep$|python3|Python|run1\.sh|bash$|/sh$)")


def ts(s):
    """RFC 3339 from chrono (3, 6 or 9 fraction digits) to unix seconds, Python 3.9-safe."""
    m = re.match(r"(.*T\d\d:\d\d:\d\d)(\.\d+)?(.*)$", s)
    frac = (m.group(2) or ".0")[1:7].ljust(6, "0")
    tz = m.group(3).replace("Z", "+00:00")
    return datetime.fromisoformat(m.group(1) + "." + frac + tz).timestamp()


def load_samples(path):
    rows = [json.loads(l) for l in open(path) if l.strip()]
    out = []
    for prev, cur in zip(rows, rows[1:]):
        a, b = ts(prev["timestamp"]), ts(cur["timestamp"])
        if b > a:
            out.append((a, b, cur))
    return out


def window(samples, lo, hi):
    """Energy (J) per rail and covered seconds in [lo, hi]."""
    e = {k: 0.0 for k in RAILS}
    covered = 0.0
    for a, b, r in samples:
        ov = min(b, hi) - max(a, lo)
        if ov <= 0:
            continue
        covered += ov
        for k, f in RAILS.items():
            e[k] += r[f] * ov
    e["soc"] = e["cpu"] + e["gpu"] + e["ane"] + e["dram"]
    return e, covered


def stats_in(samples, lo, hi, key):
    vals = [key(r) for a, b, r in samples if b > lo and a < hi]
    return vals


def foreign_cpu(ps_path, lo, hi, bench_pid):
    """Mean and max summed %CPU of other processes during [lo, hi], and the top three."""
    snaps, cur, t = [], None, None
    for line in open(ps_path):
        if line.startswith("T "):
            if cur is not None and lo <= t <= hi:
                snaps.append(cur)
            t, cur = float(line.split()[1]), {}
            continue
        parts = line.split(None, 2)
        if len(parts) < 3 or cur is None:
            continue
        pid, pcpu, comm = int(parts[0]), float(parts[1]), parts[2].strip()
        if pid == bench_pid or SELF.search(comm):
            continue
        name = os.path.basename(comm)
        cur[name] = cur.get(name, 0.0) + pcpu
    if cur is not None and t is not None and lo <= t <= hi:
        snaps.append(cur)
    if not snaps:
        return float("nan"), float("nan"), ""
    totals = [sum(s.values()) for s in snaps]
    per = {}
    for s in snaps:
        for k, v in s.items():
            per[k] = per.get(k, 0.0) + v / len(snaps)
    top = sorted(per.items(), key=lambda kv: -kv[1])[:3]
    return sum(totals) / len(totals), max(totals), " ".join("%s:%.0f" % kv for kv in top)


def analyse(run_dir, label):
    meta = json.load(open(os.path.join(run_dir, label + ".meta.json")))
    samples = load_samples(os.path.join(run_dir, label + ".jsonl"))
    bench_path = os.path.join(run_dir, label + ".json")
    bench = json.load(open(bench_path)) if os.path.exists(bench_path) else {}
    flags = []
    if meta.get("rc") != 0 or not bench:
        flags.append("bench-failed")
    if bench.get("errors"):
        flags.append("bench-errors=%d" % len(bench["errors"]))
    if bench.get("tcpp_overcap_decodes"):
        flags.append("overcap=%d" % bench["tcpp_overcap_decodes"])

    wall = bench.get("cpu", {}).get("wall_secs", float("nan"))
    s1 = meta["t_exit"] - EXIT_PAD
    s0 = s1 - wall
    win = {
        "pre": (meta["t_pre0"] + 5, meta["t_launch"] - 1),
        "loaded": (s0 - LOADED_S, s0 - 1),
        "session": (s0, s1),
        "post": (meta["t_exit"] + 10, meta["t_post1"] - 1),
    }
    e, cov, p = {}, {}, {}
    for name, (lo, hi) in win.items():
        e[name], cov[name] = window(samples, lo, hi)
        p[name] = {k: v / cov[name] if cov[name] > 0 else float("nan") for k, v in e[name].items()}
        if cov[name] < 0.9 * (hi - lo):
            flags.append("gap-%s" % name)
    base = {k: (p["pre"][k] + p["post"][k]) / 2 for k in p["pre"]}
    drift = p["post"]["soc"] - p["pre"]["soc"]
    if abs(drift) > 0.15:
        flags.append("drift=%.2fW" % drift)
    T = s1 - s0
    net = {k: e["session"][k] - base[k] * T for k in base}
    audio_min = bench.get("audio_secs", float("nan")) / 60.0
    fmean, fmax, ftop = foreign_cpu(os.path.join(run_dir, label + ".ps"), s0, s1, meta["bench_pid"])
    if fmean == fmean and fmean > 10:
        flags.append("foreign-cpu=%.0f%%" % fmean)
    temps = stats_in(samples, s0, s1, lambda r: r["temp"]["cpu_temp_avg"])
    gpu_act = stats_in(samples, s0, s1, lambda r: r["gpu_active_ratio"])
    fans = stats_in(samples, s0, s1, lambda r: max([f["rpm"] for f in r.get("fans", [])] or [0]))
    part = bench.get("partials", {})
    return {
        "label": label,
        "config": label.rsplit("-r", 1)[0],
        "audio_s": round(bench.get("audio_secs", float("nan")), 1),
        "session_s": round(T, 1),
        "J_per_min_soc": round(net["soc"] / audio_min, 1),
        "J_per_min_sys": round(net["sys"] / audio_min, 1),
        "J_per_min_gross_soc": round(e["session"]["soc"] / audio_min, 1),
        "W_cpu": round(p["session"]["cpu"] - base["cpu"], 3),
        "W_gpu": round(p["session"]["gpu"] - base["gpu"], 3),
        "W_ane": round(p["session"]["ane"] - base["ane"], 3),
        "W_dram": round(p["session"]["dram"] - base["dram"], 3),
        "W_base_soc": round(base["soc"], 3),
        "mW_loaded_idle_soc": round(1000 * (p["loaded"]["soc"] - base["soc"]), 0),
        "temp_start_C": round(temps[0], 1) if temps else float("nan"),
        "temp_max_C": round(max(temps), 1) if temps else float("nan"),
        "fan_max_rpm": max(fans) if fans else 0,
        "gpu_active_mean": round(sum(gpu_act) / len(gpu_act), 3) if gpu_act else float("nan"),
        "foreign_cpu_mean_pct": round(fmean, 1),
        "foreign_cpu_max_pct": round(fmax, 1),
        "foreign_top": ftop,
        "finalize_ms": bench.get("finalize_ms"),
        "first_partial_ms": part.get("first_partial_ms"),
        "passes": part.get("passes"),
        "skipped": part.get("skipped"),
        "median_gap_ms": part.get("median_gap_ms"),
        "revision_rate": part.get("revision_rate"),
        "chunks": bench.get("chunks", {}).get("polished"),
        "wer_pct": round(100 * bench["wer"]["wer"], 2) if "wer" in bench else "",
        "cpu_pct_one_core": round(bench.get("cpu", {}).get("cpu_pct_of_one_core", float("nan")), 1),
        "load_before": meta["before"]["load"],
        "flags": " ".join(flags),
    }


def main():
    run_dir = sys.argv[1]
    labels = sys.argv[2:] or sorted(os.path.basename(p)[: -len(".meta.json")] for p in glob.glob(os.path.join(run_dir, "*.meta.json")))
    rows = [analyse(run_dir, l) for l in labels]
    if not rows:
        sys.exit("no runs")
    with open(os.path.join(run_dir, "energy.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(rows)
    cols = ["label", "J_per_min_soc", "J_per_min_sys", "W_cpu", "W_gpu", "W_ane", "W_dram", "mW_loaded_idle_soc",
            "temp_start_C", "foreign_cpu_mean_pct", "finalize_ms", "passes", "wer_pct", "flags"]
    print("\t".join(cols))
    for r in rows:
        print("\t".join(str(r[c]) for c in cols))


if __name__ == "__main__":
    main()
```

### `summary.py`

```python
"""summary.py ENERGY_CSV BASELINE_CONFIG [OTHER_BASELINE ...]: per-config mean, sd, CV and
the ratio of each config's J/min to each baseline, with a 90 % confidence interval (Welch t on
log J/min). Runs with flags other than "gap-loaded" are left out and listed. Numbers only."""
import csv, math, sys
from collections import defaultdict

# Two-sided 90 % t quantiles (Python 3.9 stdlib has no t distribution).
T90 = {1: 6.314, 2: 2.920, 3: 2.353, 4: 2.132, 5: 2.015, 6: 1.943, 7: 1.895, 8: 1.860, 9: 1.833,
       10: 1.812, 12: 1.782, 15: 1.753, 20: 1.725, 30: 1.697}


def t90(df):
    keys = [k for k in sorted(T90) if k <= max(1, int(df))]
    return T90[keys[-1]]


def msd(xs):
    m = sum(xs) / len(xs)
    sd = math.sqrt(sum((x - m) ** 2 for x in xs) / (len(xs) - 1)) if len(xs) > 1 else float("nan")
    return m, sd


rows = list(csv.DictReader(open(sys.argv[1])))
baselines = sys.argv[2:]
by = defaultdict(list)
dropped = []
for r in rows:
    bad = [f for f in r["flags"].split() if not f.startswith("gap-loaded")]
    if bad:
        dropped.append((r["label"], " ".join(bad)))
        continue
    by[r["config"]].append(r)

print("config\tn\tJ/min soc mean\tsd\tCV%\tJ/min sys mean\tW cpu\tW gpu\tW ane\tW dram\tloaded-idle mW\tfinalize ms\tWER %")
for c, rs in sorted(by.items()):
    j = [float(r["J_per_min_soc"]) for r in rs]
    m, sd = msd(j)
    avg = lambda k: sum(float(r[k]) for r in rs) / len(rs)
    wer = [float(r["wer_pct"]) for r in rs if r["wer_pct"] not in ("", None)]
    fin = sorted(float(r["finalize_ms"]) for r in rs if r["finalize_ms"] not in ("", "None"))
    print("%s\t%d\t%.1f\t%.1f\t%.1f\t%.1f\t%.3f\t%.3f\t%.3f\t%.3f\t%.0f\t%s\t%s" % (
        c, len(rs), m, sd, 100 * sd / m if len(rs) > 1 else float("nan"), avg("J_per_min_sys"),
        avg("W_cpu"), avg("W_gpu"), avg("W_ane"), avg("W_dram"), avg("mW_loaded_idle_soc"),
        "%.0f" % fin[len(fin) // 2] if fin else "", "%.2f" % (sum(wer) / len(wer)) if wer else ""))

for b in baselines:
    if b not in by:
        print("baseline %s has no clean runs" % b)
        continue
    lb = [math.log(float(r["J_per_min_soc"])) for r in by[b]]
    print("\nratio of J/min (soc) to %s, 90 %% CI:" % b)
    for c, rs in sorted(by.items()):
        if c == b:
            continue
        lc = [math.log(float(r["J_per_min_soc"])) for r in rs]
        (mc, sc), (mb, sb) = msd(lc), msd(lb)
        ratio = math.exp(mc - mb)
        if len(lc) > 1 and len(lb) > 1:
            vc, vb = sc ** 2 / len(lc), sb ** 2 / len(lb)
            se = math.sqrt(vc + vb)
            df = (vc + vb) ** 2 / ((vc ** 2 / (len(lc) - 1)) + (vb ** 2 / (len(lb) - 1))) if se > 0 else 30
            lo, hi = math.exp(mc - mb - t90(df) * se), math.exp(mc - mb + t90(df) * se)
            verdict = "halves" if hi <= 0.5 else ("does not halve" if lo > 0.5 else "undecided: add runs")
            print("  %s / %s = %.3f  [%.3f, %.3f]  -> %s" % (c, b, ratio, lo, hi, verdict))
        else:
            print("  %s / %s = %.3f  (one run: no CI)" % (c, b, ratio))

if dropped:
    print("\nleft out (flagged):")
    for l, f in dropped:
        print("  %s: %s" % (l, f))
```

### `agree.py`

```python
"""agree.py A.json B.json [...]: how far apart the final transcripts of bench session results
are, without printing any of their words. For each B: word edit distance to A over A's word
count (lowercase alphanumerics, like the bench's WER), plus the word counts. Use it on the
private recordings, where there's no reference: it prints numbers only.

  python3 -I agree.py runs/C0-r1.json runs/C-r1.json runs/C2-r1.json
"""
import json, sys


def words(path):
    text = json.load(open(path)).get("final_text", "")
    out = []
    for w in text.split():
        w = "".join(ch for ch in w.lower() if ch.isalnum())
        if w:
            out.append(w)
    return out


def edits(a, b):
    prev = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        cur = [i] + [0] * len(b)
        for j, y in enumerate(b, 1):
            cur[j] = min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (x != y))
        prev = cur
    return prev[-1]


a = words(sys.argv[1])
for path in sys.argv[2:]:
    b = words(path)
    e = edits(a, b)
    print("%s vs %s: words %d / %d, edits %d, disagreement %.2f %%" % (
        sys.argv[1].rsplit("/", 1)[-1], path.rsplit("/", 1)[-1], len(a), len(b), e, 100.0 * e / max(len(a), 1)))
```

### `scrub.py`

```python
"""scrub.py FILE.json ...: remove transcript text from bench result files in place, once the
analysis (agree.py) no longer needs it. Keeps every number. Prints only file names and counts."""
import json, sys

TEXT_KEYS = {"final_text", "confirmed_at_stop", "text", "hypothesis", "hypotheses_distinct", "reference"}


def strip(x):
    if isinstance(x, dict):
        return {k: strip(v) for k, v in x.items() if k not in TEXT_KEYS}
    if isinstance(x, list):
        return [strip(v) for v in x]
    return x


n = 0
for path in sys.argv[1:]:
    if path.endswith(".meta.json"):
        continue
    data = json.load(open(path))
    json.dump(strip(data), open(path, "w"), indent=1)
    n += 1
print("scrubbed %d files" % n)
```

## History

### 2026-10-10 01:28 UTC — edit by `brandon`

description edited

### 2026-10-10 02:28 UTC — edit by `brandon`

description edited

### 2026-10-10 02:52 UTC — edit by `brandon`

depends_on +tuh-z7dr
