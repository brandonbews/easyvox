# Model bench, October 2026

Run on 9 October 2026 to settle two questions from `DESIGN.md` ("Models"):

1. **The small-model slot.** Canary 180M Flash was to replace Whisper base.en only if it came back clean: across all test clips, repeated at least 3 times, no empty or cut-short transcripts, a word error rate no worse than Whisper base.en's, and no crash or hang.
2. **Parakeet TDT 0.6B v2's quantization.** Can Q5_K_M (539 MB) replace Q8_0 (730 MB) without losing accuracy or speed?

Later the same day the engine moved to one pipeline for every model (no native streaming, long dictations polished chunk by chunk; see `DESIGN.md`, "Live text while speaking"). [One pipeline](#one-pipeline) re-runs the session numbers for the three offered models, and [Long dictation](#long-dictation) adds recordings of 3 and 9 minutes.

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
- **First words, text ready and CPU** in this table come from the old engine, where Parakeet Unified streamed natively. [One pipeline](#one-pipeline) has the current numbers.

## What we decided

**Small slot: Whisper base.en stays; Canary 180M Flash is bench only.** Canary was clean on two of the three counts: no empty transcripts (the [transcribe.cpp #140](https://github.com/handy-computer/transcribe.cpp/issues/140) bug never showed up), nothing written on silence, and no crash or hang. But on transcribe-cpp 0.3.1 it writes every number as words: "$342" came out as "three hundred and forty two dollars", and a phone number as "five five five zero one four two". The runtime has no number-formatting (ITN) switch for Canary. On the three number clips that's 57 % WER, against 0 % for every other model. On the other eleven clips Canary and Whisper tie (1.52 % each). So it fails the "no worse than Whisper base.en" bar (7.12 % vs 1.37 %), and digits matter for dictation. Its Q8_0 file wasn't tried: that test was only for empty transcripts, which didn't happen, and quantization can't add number formatting.

Whisper base.en's own flaw showed up too: it wrote "you" for 2 s of silence and added a stray "Mhm." after one long clip. The app should skip the final pass when a recording holds no speech.

**v2: Q5_K_M replaces Q8_0.** Both files wrote exactly the same text for every clip. First words appear just as fast (0.45–0.48 s for both). Q5_K_M decodes about 5 % slower, so its final text lands 10–20 ms later (medians of 3 sessions each), well inside the run-to-run spread and far below what anyone can notice. In return it's 191 MB smaller to download and uses about 190 MB less memory. The Q8_0 entry is gone from the catalog.

## One pipeline

Every model now takes the same path: the preview re-decodes the recent audio every 400 ms, and the final text comes from offline decodes, of chunks cut at pauses while you talk, then of the rest on release. Parakeet Unified no longer streams natively. Same clips and machine as above, one `sessions` replay each (two for the Parakeets' first words):

| Model | Word error rate | First words | Text ready after release (3 / 11 / 30 s clip) | CPU while talking (3 / 11 / 30 s) | GPU while talking | Preview revisions | Memory, loaded / peak |
|---|---:|---:|---:|---:|---:|---:|---:|
| **Parakeet Unified EN 0.6B Q5_K_M** | 0.55 % | 0.46–0.49 s | 0.11–0.13 / 0.18 / 0.47–0.48 s | 14–19 / 27–33 / 39–46 % | ~80 % | 14–40 % | 693 / 741 MB |
| **Parakeet TDT 0.6B v2 Q5_K_M** | 0.55 % | 0.45–0.49 s | 0.11 / 0.17–0.18 / 0.43–0.45 s | 15–23 / 27–34 / 37–43 % | ~85 % | 17–43 % | 691 / 737 MB |
| **Whisper base.en Q8_0** | 1.37 % | 0.50–0.51 s | 0.13 / 0.14 / 0.37 s | 30 / 29 / 27 % | ~73 % | 50–56 % | 205 / 264 MB |

- **Unified and v2 now cost the same.** Unified's preview used 7–9 % of one core when it streamed natively; re-decoding, it uses what v2 does. In exchange its first words come sooner (about 0.5 s instead of 1.35 s; one Unified run took 0.91 s on the 3 s clip). The two make the same word errors (two names), but **Unified often leaves out the last full stop**: 4 of the 13 speech clips end without one ("…I'll be 10 minutes late"), where v2 ends every clip with one. Appending silence doesn't fix it reliably (12 of 13 with 0.5 s, 10 of 13 with 1 s). It does the same at chunk cuts, which is why chunks are decoded with context (see "Long dictation").
- **GPU while talking:** macOS's "Device Utilization %" for the whole GPU, sampled every second during the replays with the preview on (`ioreg`). A coarse, system-wide figure; it drops to 3–9 % with the preview off. Re-decoding a window of up to 20 s every 400 ms keeps the GPU busy most of the time, which matters for battery.
- **Preview revisions:** the share of preview updates that changed words already on screen rather than only adding to them (lower is calmer). Whisper's preview flickers most: it has no word timings, so the preview aligns its words by spreading them evenly.
- **Text ready** is the same as before for these short clips: up to 30 s there's one chunk, decoded on release.
- Whisper base.en's word error rate is back to 1.37 % (the first run of this re-check had 1.92 %, from asking it for timestamps on short audio, which changed its guesses on the names clip; it now asks only past 30 s).

## Long dictation

Holding the key for minutes is normal use (`DESIGN.md`, "Requirements"). The old engine decoded the whole recording in one pass on release, and that pass grows quadratically: the text came 1.2 / 4.3 / 9.2 / 21.6 s after 1 / 3 / 5 / 9 minutes, and Whisper silently lost text past 30 s. Now chunks of about 30 s are polished while you talk and only the rest is decoded on release. Real-time replays, one run each, the same day:

| Model | Recording | Text ready after release (one pass → now) | Word error rate (one pass → now) | Peak memory (one pass → now) | CPU while talking, now (one pass) | GPU while talking | Preview behind the speech, median / 95th pct |
|---|---|---:|---:|---:|---:|---:|---:|
| Parakeet Unified | 3 min | 4.29 → **0.42 s** | 0.76 → 0.30 % | 923 → 801 MB | 47–50 % (8 %) | ~91 % | ≤ 0.6 / ≤ 1.3 s |
| Parakeet Unified | 3.5 min, long pauses | 4.75 → **0.44 s** | 0 → 0 % | 913 → 805 MB | 37–53 % (7 %) | ~91 % | ≤ 0.3 / ≤ 1.7 s |
| Parakeet Unified | 9 min | 21.58 → **0.38 s** | 1.15 → 0.86 % | 1191 → 841 MB | 54–58 % (8 %) | ~90 % | ≤ 0.6 / ≤ 1.7 s |
| Parakeet TDT v2 | 3 min | 4.06 → **0.37 s** | 0 → 0 % | 896 → 800 MB | 42–44 % (39 %) | ~91 % | ≤ 0.5 / ≤ 1.1 s |
| Parakeet TDT v2 | 9 min | 21.24 → **0.38 s** | 0.75 → **0.40 %** | 1133 → 833 MB | 47–51 % (37 %) | ~89 % | ≤ 0.5 / ≤ 1.6 s |
| Whisper base.en | 3 min | 1.66 → **0.37 s** | 16.3 → **0.61 %** | 286 → 287 MB | 40–56 % (41 %) | ~71 % | ≤ 0.6 / ≤ 1.4 s |

- **The recordings** are macOS `say` (Samantha) reading 32 paragraphs of rambling notes (a renovation, work, groceries, a book, a trip) with 0.5 s between paragraphs: 3 min is 195 s and 656 words, 9 min is 530 s and 1,735 words. "Long pauses" is Daniel with 2.5–8 s thinking pauses between and inside paragraphs (207 s). They're not in the repo (17 MB for 9 minutes); any long WAV and its transcript work with the commands below. Text-to-speech pauses are clean silence, so real speech will test the pause finder harder.
- **Text ready** no longer grows with length: 7 chunks were polished while talking in 3 minutes, 21 in 9, each in 0.3–0.45 s, and release only decodes the last one. The old numbers are from the same clips and machine, earlier that day.
- **Word errors** held or dropped, and no run lost or doubled a word at a chunk join (the longest run of missing words is one). Whisper's one pass had lost 85 words in a row. Unified's 9-minute errors are its own guesses ("lighthousekeeper's", "practices at"), the same kind v2 makes less often.
- **Peak memory** is bounded by the chunk instead of the whole recording.
- **CPU while talking** is each minute's share of one core (`ps` every 2 s). It stays flat over 9 minutes, since the preview window is capped at 20 s, but at about half a core it's the price of re-decoding: Unified streamed natively for 7–8 %. It doesn't drop in silence either: the long-pauses run kept re-decoding through 8 s pauses.
- **Preview behind the speech:** how far the preview's newest words trailed the voice, per minute. It kept up for the whole 9 minutes; the longest gap between preview updates was 1.3 s (3.5 s once), when a chunk was being polished. Polished text replaced preview text 6–7 times in 3 minutes and 18 in 9 (at most once per chunk, as the contract says). In the long-pauses run the 95th percentile reaches 4 s in the last minute only because the preview updates during a pause, long after the last word.

## Limits of this bench

- **The clips can't tell the top models apart.** They're 15 short clips made with macOS text-to-speech voices (`testdata/generate.sh`): clean, evenly paced, no background noise. All three Parakeet files made the same two mistakes, both on unusual names ("Priya Rahman", "Syabhan O'Connor"). They're good for speed, memory, empty or invented output, numbers and punctuation, but not for ranking accuracy. **Real-voice recordings are still wanted:** 5–10 clips of a real person, with numbers, names and a long 60–90 s ramble, run through every model.
- One machine: M4 (16 GB), macOS 26.6.2, transcribe-cpp 0.3.1, 4 threads, `easyvox-bench` release build. Load average 1.6–2.2 while it ran, with nothing else building.
- Sessions replay a WAV file at real-time speed through the same code path the app uses (live preview, chunks polished while talking, the rest on release). In the first run each model ran one session pass and the two v2 files three each; "One pipeline" and "Long dictation" ran one or two passes per model, load average 1.6–3.3.

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
for m in $MODELS; do $B sessions --model $m --out sessions-$m.json; done

# Long dictation: a real-time replay of a long recording, with its transcript in long.txt.
# /usr/bin/time -l adds the peak memory footprint; --chunk 0 gives the old one-pass final.
/usr/bin/time -l $B session --model parakeet-tdt-0.6b-v2-q5_k_m-gguf --wav long.wav --ref long.txt --out long.json
```

Where the table's numbers come from:

- `bench`: `wer_total`, `empty_speech_decodes` and `failed_speech_decodes`, `rtf_speech_clips`, `mem.after_load.footprint_mb` and `mem.after_decodes.peak_footprint_mb`. The silence clip's `hypothesis` is under `clips`. Progress lines on stderr flag `EMPTY`, `FAILED` and `UNSTABLE` (runs that disagree).
- `sessions`, the `partials_on` runs: `partials.first_partial_ms`, `finalize_ms`, `cpu.cpu_pct_of_one_core` and `partials.revision_rate`. Don't use its memory numbers: it loads a second copy of the model for the `partials_off` runs.
- `session`: `finalize_ms`, `wer` (with `--ref`), `chunks` and `mem_end.peak_footprint_mb`; `partials.confirmed_rewrites` counts polished chunks swapped into the preview. CPU per minute came from `ps -o time= -p <pid>` every 2 s, GPU from `ioreg -r -d 1 -c IOAccelerator` ("Device Utilization %") every second, and the preview's lag from the partial lines on stderr, matched against the transcript's timeline.

To re-run the v2 Q8_0 comparison, add its catalog entry back as `BenchOnly`: file `parakeet-tdt-0.6b-v2-Q8_0.gguf` from `handy-computer/parakeet-tdt-0.6b-v2-gguf` at commit `382796e405a9fe37145938dccbabfb25e93e4286`, sha256 `f0d0e99cebb6d3b83f1f7069b82b5d3c2e39a54545b0da039cb4bafd9c4e5caa`, 729,574,912 bytes.
