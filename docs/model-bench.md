# Model bench, October 2026

Run on 9 October 2026 to settle two questions from `DESIGN.md` ("Models"):

1. **The small-model slot.** Canary 180M Flash was to replace Whisper base.en only if it came back clean: across all test clips, repeated at least 3 times, no empty or cut-short transcripts, a word error rate no worse than Whisper base.en's, and no crash or hang.
2. **Parakeet TDT 0.6B v2's quantization.** Can Q5_K_M (539 MB) replace Q8_0 (730 MB) without losing accuracy or speed?

## Results

| Model (file) | Download | Word error rate | Empty or failed | 2 s of silence | First words | Text ready after release (3 / 11 / 30 s clip) | CPU while talking | Speed (RTF) | Memory, loaded / peak |
|---|---:|---:|---:|---|---:|---:|---:|---:|---:|
| **Parakeet Unified EN 0.6B Q5_K_M** (default) | 541 MB | 0.55 % | 0 of 84 | nothing | 1.35 s | 0.09 / 0.18 / 0.46 s | 7–9 % | 0.015 | 693 / 743 MB |
| Parakeet TDT 0.6B v2 Q8_0 (dropped) | 730 MB | 0.55 % | 0 of 84 | nothing | 0.45 s | 0.06 / 0.16 / 0.42 s | 18–37 % | 0.014 | 879 / 924 MB |
| **Parakeet TDT 0.6B v2 Q5_K_M** (alternative) | 539 MB | 0.55 % | 0 of 84 | nothing | 0.47 s | 0.07 / 0.17 / 0.44 s | 15–37 % | 0.015 | 691 / 735 MB |
| Canary 180M Flash Q5_K_M (bench only) | 159 MB | **7.12 %** | 0 of 84 | nothing | 0.45 s | 0.09 / 0.20 / 0.51 s | 5–14 % | 0.016 | 243 / 312 MB |
| **Whisper base.en Q8_0** (alternative) | 85 MB | 1.37 % | 0 of 84 | "you" | 0.51 s | 0.17 / 0.17 / 0.52 s | 31–36 % | 0.014 | 205 / 259 MB |

How to read it:

- **Word error rate:** the share of words that came out wrong, over the 14 speech clips, 6 runs each. Lower is better. It ignores punctuation and case, and counts "10" and "ten" as the same, but not "342" and "three hundred and forty two".
- **Empty or failed:** decodes of a speech clip that returned no text or an error (14 clips × 6 runs = 84).
- **2 s of silence:** what the model wrote for a clip with no speech. It should write nothing.
- **First words:** how long after you start talking the live preview shows its first words.
- **Text ready after release:** from the end of the speech (key released) to the final text.
- **CPU while talking:** share of one CPU core during the session, live preview included. The GPU does most of the work and isn't counted.
- **Speed (RTF):** decode time divided by audio length. 0.015 means a 10 s recording takes 0.15 s.
- **Memory:** the process's memory footprint once the model is loaded, and its peak after decoding every clip.

## What we decided

**Small slot: Whisper base.en stays; Canary 180M Flash is bench only.** Canary was clean on two of the three counts: no empty transcripts (the [transcribe.cpp #140](https://github.com/handy-computer/transcribe.cpp/issues/140) bug never showed up), nothing written on silence, and no crash or hang. But on transcribe-cpp 0.3.1 it writes every number as words: "$342" came out as "three hundred and forty two dollars", and a phone number as "five five five zero one four two". The runtime has no number-formatting (ITN) switch for Canary. On the three number clips that's 57 % WER, against 0 % for every other model. On the other eleven clips Canary and Whisper tie (1.52 % each). So it fails the "no worse than Whisper base.en" bar (7.12 % vs 1.37 %), and digits matter for dictation. Its Q8_0 file wasn't tried: that test was only for empty transcripts, which didn't happen, and quantization can't add number formatting.

Whisper base.en's own flaw showed up too: it wrote "you" for 2 s of silence and added a stray "Mhm." after one long clip. The app should skip the final pass when a recording holds no speech.

**v2: Q5_K_M replaces Q8_0.** Both files wrote exactly the same text for every clip. First words appear just as fast (0.45–0.48 s for both). Q5_K_M decodes about 5 % slower, so its final text lands 10–20 ms later (medians of 3 sessions each), well inside the run-to-run spread and far below what anyone can notice. In return it's 191 MB smaller to download and uses about 190 MB less memory. The Q8_0 entry is gone from the catalog.

## Limits of this bench

- **The clips can't tell the top models apart.** They're 15 short clips made with macOS text-to-speech voices (`testdata/generate.sh`): clean, evenly paced, no background noise. All three Parakeet files made the same two mistakes, both on unusual names ("Priya Rahman", "Syabhan O'Connor"). They're good for speed, memory, empty or invented output, numbers and punctuation, but not for ranking accuracy. **Real-voice recordings are still wanted:** 5–10 clips of a real person, with numbers, names and a long 60–90 s ramble, run through every model.
- One machine: M4 (16 GB), macOS 26.6.2, transcribe-cpp 0.3.1, 4 threads, `easyvox-bench` release build. Load average 1.6–2.2 while it ran, with nothing else building.
- Sessions replay a WAV file at real-time speed through the same code path the app uses (live preview, then one final pass on release), with `--final-redecode` as the app sets it. Each model ran one session pass; the two v2 files ran three each.

## How to reproduce

Put the models in `~/Library/Application Support/EasyVox/models/gguf/` (or set `EASYVOX_MODELS_DIR`). Each catalog entry in `crates/easyvox-core/src/models.rs` has the pinned download URL and sha256. Then:

```sh
source "$HOME/.cargo/env"
cargo build -p easyvox-bench --release
B=target/release/easyvox-bench
MODELS="parakeet-unified-en-0.6b-q5_k_m-gguf parakeet-tdt-0.6b-v2-q5_k_m-gguf canary-180m-flash-q5_k_m-gguf whisper-base.en-q8_0-gguf"

uptime          # check nothing else is busy (load average around 2 or less)
$B info         # which models are on disk

# Warm-up: compiles the Metal shaders and loads the files into the page cache.
for m in $MODELS; do $B bench --model $m --fast > /dev/null; done

# Accuracy, empty output, speed and memory: every clip, 3 runs each. Run it twice,
# in reverse order the second time, to even out drift.
for m in $MODELS; do $B bench --model $m --reps 3 --out bench-$m.json; done

# Live preview and latency: real-time replays of the 3, 11 and 30 s clips.
for m in $MODELS; do $B sessions --model $m --final-redecode --out sessions-$m.json; done
```

Where the table's numbers come from:

- `bench`: `wer_total`, `empty_speech_decodes` and `failed_speech_decodes`, `rtf_speech_clips`, `mem.after_load.footprint_mb` and `mem.after_decodes.peak_footprint_mb`. The silence clip's `hypothesis` is under `clips`. Progress lines on stderr flag `EMPTY`, `FAILED` and `UNSTABLE` (runs that disagree).
- `sessions`, the `partials_on` runs: `partials.first_partial_ms`, `finalize_ms` and `cpu.cpu_pct_of_one_core`. Don't use its memory numbers for offline models: it loads a second copy of the model for the `partials_off` runs.

To re-run the v2 Q8_0 comparison, add its catalog entry back as `BenchOnly`: file `parakeet-tdt-0.6b-v2-Q8_0.gguf` from `handy-computer/parakeet-tdt-0.6b-v2-gguf` at commit `382796e405a9fe37145938dccbabfb25e93e4286`, sha256 `f0d0e99cebb6d3b83f1f7069b82b5d3c2e39a54545b0da039cb4bafd9c4e5caa`, 729,574,912 bytes.
