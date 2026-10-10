# Model bench, October 2026

Run on 9 October 2026 to settle two questions from `DESIGN.md` ("Models"):

1. **The small-model slot.** Canary 180M Flash was to replace Whisper base.en only if it came back clean: across all test clips, repeated at least 3 times, no empty or cut-short transcripts, a word error rate no worse than Whisper base.en's, and no crash or hang.
2. **Parakeet TDT 0.6B v2's quantization.** Can Q5_K_M (539 MB) replace Q8_0 (730 MB) without losing accuracy or speed?

Later the same day the engine moved to one pipeline for every model (no native streaming, long dictations polished chunk by chunk; see `DESIGN.md`, "Live text while speaking"). [One pipeline](#one-pipeline) re-runs the session numbers for the three offered models, and [Long dictation](#long-dictation) adds recordings of 3 and 9 minutes. Those re-runs made Parakeet TDT v2 the default in place of Parakeet Unified (see [What we decided](#what-we-decided)). [Idle and energy](#idle-and-energy) covers the app while it waits, and the preview while you pause. [Real voice](#real-voice) runs the three models on the author's own voice, and checks the chunk cuts and joins on a 5-minute ramble in an office.

## Results

| Model (file) | Download | Word error rate | Empty or failed | 2 s of silence | First words | Text ready after release (3 / 11 / 30 s clip) | CPU while talking | Speed (RTF) | Memory, loaded / peak |
|---|---:|---:|---:|---|---:|---:|---:|---:|---:|
| **Parakeet Unified EN 0.6B Q5_K_M** (alternative) | 541 MB | 0.55 % | 0 of 84 | nothing | 1.35 s | 0.09 / 0.18 / 0.46 s | 7–9 % | 0.015 | 693 / 743 MB |
| Parakeet TDT 0.6B v2 Q8_0 (dropped) | 730 MB | 0.55 % | 0 of 84 | nothing | 0.45 s | 0.06 / 0.16 / 0.42 s | 18–37 % | 0.014 | 879 / 924 MB |
| **Parakeet TDT 0.6B v2 Q5_K_M** (default) | 539 MB | 0.55 % | 0 of 84 | nothing | 0.47 s | 0.07 / 0.17 / 0.44 s | 15–37 % | 0.015 | 691 / 735 MB |
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

**Default: Parakeet TDT 0.6B v2 replaces Parakeet Unified** (decided after the [one pipeline](#one-pipeline) and [long dictation](#long-dictation) re-runs below). Unified had been the default because it streamed the preview natively; with one pipeline both models re-decode, and they cost the same and show first words as fast. v2 then made fewer errors on long dictations (0.40 % against 0.86 % over 9 minutes) and ended every clip with a full stop, where Unified left it out on 4 of 13. Unified stays as an alternative: it scores a little better on public benchmarks, which these text-to-speech clips can't confirm or rule out.

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

- **The recordings** are macOS `say` (Samantha) reading 22 paragraphs of rambling notes (a renovation, work, groceries, a book, a trip) with 0.5 s between paragraphs: 3 min is 195 s and 656 words, 9 min is 530 s and 1,735 words. "Long pauses" is Daniel with 2.5–8 s thinking pauses between and inside paragraphs (207 s). The WAVs aren't committed: `testdata/longform/generate.py` regenerates them in about 25 s, and the reference transcripts are committed in `testdata/longform/manifest.tsv` (see `testdata/longform/README.md`). Any long WAV and its transcript work with the commands below. Text-to-speech pauses are clean silence, so real speech will test the pause finder harder.
- **Text ready** no longer grows with length: 7 chunks were polished while talking in 3 minutes, 21 in 9, each in 0.3–0.45 s, and release only decodes the last one. The old numbers are from the same clips and machine, earlier that day.
- **Word errors** held or dropped, and no run lost or doubled a word at a chunk join (the longest run of missing words is one). Whisper's one pass had lost 85 words in a row. Unified's 9-minute errors are its own guesses ("lighthousekeeper's", "practices at"), the same kind v2 makes less often.
- **Peak memory** is bounded by the chunk instead of the whole recording.
- **CPU while talking** is each minute's share of one core (`ps` every 2 s). It stays flat over 9 minutes, since the preview window is capped at 20 s, but at about half a core it's the price of re-decoding: Unified streamed natively for 7–8 %. It didn't drop in silence either: the long-pauses run kept re-decoding through 8 s pauses. The preview now rests in long pauses (see [Idle and energy](#idle-and-energy)).
- **Preview behind the speech:** how far the preview's newest words trailed the voice, per minute. It kept up for the whole 9 minutes; the longest gap between preview updates was 1.3 s (3.5 s once), when a chunk was being polished. Polished text replaced preview text 6–7 times in 3 minutes and 18 in 9 (at most once per chunk, as the contract says). In the long-pauses run the 95th percentile reaches 4 s in the last minute only because the preview updates during a pause, long after the last word.

## Idle and energy

Three changes for the time EasyVox spends waiting, measured the same day on the same M4 with Parakeet Unified (release builds; `DESIGN.md`, "Keeping it light and fast"):

1. **No timer while idle.** Every binary sets `GGML_METAL_NO_RESIDENCY=1` first thing in `main`, so ggml-metal's residency thread never starts (`DESIGN.md`, "Engine and macOS gotchas").
2. **The model is freed after 10 minutes without use**, and reloaded on the next key press while the session already records.
3. **The preview rests in pauses.** A preview pass runs only if something louder than the pause threshold arrived since the last one, plus two more after the speech stops (`SpeechGate` in `crates/easyvox-core/src/pause.rs`).

### The idle app

`dist/EasyVox.app` (`scripts/bundle.sh --release`), launched with `open` and left alone. Rates over 30 s from `proc_pid_rusage`, the counters behind `top`'s wake-up column:

| The app, nobody dictating | Wake-ups a second | CPU, share of one core | Memory footprint |
|---|---:|---:|---:|
| Before, model loaded | 145 | 0.15–0.24 % | 687 MB |
| **Now, model loaded** | **0.3** | **0.003 %** | 667 MB |
| **Now, after the 10-minute unload** | **0.3** | **0.002 %** | **92 MB** |

- **The wake-ups** came from a ggml-metal thread that wakes every 5 ms to keep its "residency sets" (GPU memory held ready between decodes) alive. It runs for as long as the Metal device exists, so unloading the model alone wouldn't have stopped it. "Before" stayed at 145 a second for minutes after its last decode.
- **Memory:** unloading frees about 575 MB (630 MB right after a dictation, in the self-test). It takes 25–35 ms, off the UI thread. What's left is the app itself plus ggml's Metal device and compiled shaders, which live until the app quits.
- **Quitting** left no crash report, with the model loaded or unloaded.

### Dictation after idling

`easyvox-bench session` on the 3 s clip, three runs warm and two after 5 minutes idle (`--idle-before 300`) for each line. `--unload` frees the model first, so the session reloads it while it records:

| | First words | Text ready after release | Reloading the model |
|---|---:|---:|---:|
| Residency sets on (ggml's default), warm | 0.46 s | 0.08–0.09 s | |
| Residency sets off (now), warm | 0.46–0.47 s | 0.08–0.09 s | |
| Residency sets on, after 5 min idle | 0.47–0.48 s | 0.08–0.11 s | |
| Residency sets off, after 5 min idle | 0.48 s | 0.07–0.09 s | |
| Model unloaded, warm | 0.48–0.49 s | 0.07–0.09 s | 0.11–0.12 s |
| Model unloaded, after 5 min idle | 0.49 s | 0.08 s | 0.14–0.15 s |

- **The residency sets don't help** these decodes, warm or after an idle, so turning them off costs nothing.
- **A reload costs about 20 ms of first words**, not its 0.11–0.15 s: it overlaps the first 0.4 s of audio that the first preview pass waits for anyway. Nothing said meanwhile is lost; the final text covers the whole recording. In the app's self-test the reload took 0.15 s, and the first words came 0.50 s after the key press, against 0.54 s with the model loaded. (The first load under a new app signature still compiles the Metal shaders, about 14 s, once.)

### The preview in pauses

Real-time replays with the key held, before and after the speech gate. "Before" is the build from before these changes, run with `GGML_METAL_NO_RESIDENCY=1` so that only the gate differs. CPU is the bench's share of one core; GPU is the whole GPU's "Device Utilization %", sampled every second:

| Recording | Preview passes run (skipped) | CPU | GPU | First words | Text ready after release | Word error rate |
|---|---:|---:|---:|---:|---:|---:|
| 3 / 11 / 30 s clips, no pauses (6 runs each) | 7 / 26 / 71–73 (none) | unchanged | unchanged | unchanged | unchanged | 0 % |
| 44 s, 2–8 s pauses over room noise (5 runs) | 106–108 → 68–69 (37–38) | 40–41 → **26–28 %** | 89 → **70 %** (1 run) | unchanged | 0.26–0.31 → 0.30–0.32 s | 0 % |
| 3.5 min, 2.5–8 s pauses (1 run) | 497 → 436 (59) | 46 → **40 %** | 91 → **83 %** | unchanged | 0.40 → 0.43 s | 0 % |
| Real voice, 81 s read aloud (3 and 2 runs) | 193–194 → 188–191 (3) | 45–48 → 44–46 % | 90–92 → 87–91 % | unchanged | 0.38–0.46 → 0.39–0.44 s | 2.1 → 2.1 % |
| Real voice, 5 min ramble (2 runs) | 719 → 705–709 (10–13) | 41–44 → 40–43 % | 88–91 → 88–91 % | unchanged | 0.36 → 0.35–0.36 s | |

- **The clips:** the first three rows are text-to-speech: the test clips, the 44 s clip, and the long-pauses recording from "Long dictation". The 44 s clip strings four of the short clips together with 2–8 s of silence, all over white noise at −50 dBFS. The two real recordings are the author's own voice, on cheap AirPods in an office for the ramble; they're private and not in the repo. Their room noise sits around −50 dBFS against speech at about −17 dBFS. Two runs that overlapped another session's Xcode build on this Mac were dropped.
- **Long pauses get cheap; real speech barely pauses that long.** Skipping starts only once a quiet stretch outlasts the two settle passes, about 1.2 s. In the 5-minute ramble the gate saw 87 quiet stretches, and 82 of them lasted 0.4–0.8 s (a breath between phrases); only 5 reached 1.2 s. Room noise wasn't the problem: the gate's threshold sat near −37 dBFS, well above the noise. Even with no settle passes it would skip only 17 % of the ramble's passes (offline replay of the gate on the recording). So on real dictation the preview still costs about half a core; a cheaper preview is a job for the energy spike.
- **First words** don't move: the first check that hears speech always decodes. In both builds a few runs (4 of 18 before, 2 of 18 after, on the short clips) showed the first words one pass (0.4 s) later, from where the 400 ms ticks happened to fall against the first word.
- **Releasing after a long pause** costs about 25 ms: on the 44 s clip, which ends with 3 s of quiet, the final pass took a median 0.31 s instead of 0.28 s, most likely because the GPU had gone idle and slowed down meanwhile. The real read-aloud recording ends with 2.6 s of quiet and showed no difference, and in the ramble the final text came slightly sooner, since no preview pass was still running at release.
- **Word errors** are the same, as expected: the gate only touches the preview, and the final text decodes all of the audio.

To reproduce: `easyvox-bench session … --idle-before 300` and `--unload` for the latency table (`metal_residency`, `model_wait_ms`, `partials.first_partial_ms`, `finalize_ms` in its JSON; `EASYVOX_METAL_RESIDENCY=1` turns the residency sets back on), and `partials.passes` and `partials.skipped` for the gate. For the idle app, any tool that reads a process's wake-ups works (`top -stats pid,cpu,idlew,mem -pid <pid>`, or `proc_pid_rusage`).

## Real voice

Run on the evening of 9 October 2026, same M4, on two recordings of the author's own voice. They're private and not in the repo (`recordings/` is gitignored), so this section gives numbers only, no transcript text. Both are 16 kHz mono WAVs:

- **Read aloud** (`structured.wav`): 81 s, a 238-word script with names, times, sums of money and technical terms, read at a normal pace. Its reference transcript gives the word error rate.
- **Ramble** (`ramble.wav`): 5:00 of unscripted talking, on cheap AirPods in an office. There's no reference. Room noise sits around −50 dBFS and speech around −17 dBFS (90th percentile of 30 ms frames). The file starts with 1 s of digital silence, and the speech starts at 1.5 s.

Every run is a real-time `session` replay, the app's code path: a preview pass every 400 ms, chunks polished while talking, the rest decoded on release. Each run started only when the 1-minute load average was under 2.5 and nothing was compiling (1.4–2.4 during the runs). Other processes' CPU was logged every second, and no run had to be dropped. The models took turns, in a different order each round. Each model wrote the same final text in every run of the same recording.

### Read aloud, 3 runs each

| Model | Word error rate | First words | Text ready after release | CPU while talking | GPU while talking | Memory, loaded / peak |
|---|---:|---:|---:|---:|---:|---:|
| **Parakeet TDT 0.6B v2 Q5_K_M** (default) | **1.68 %** (4 words) | 0.49–0.50 s | 0.37–0.42 s | 39 % | 79–92 % | 706–724 / 754–781 MB |
| Parakeet Unified EN 0.6B Q5_K_M | 2.10 % (5) | 0.49–0.50 s | 0.38–0.41 s | 43–44 % | 88–91 % | 710 / 761–765 MB |
| Whisper base.en Q8_0 | 6.72 % (16) | 0.51–0.53 s | 0.23–0.40 s | 43–45 % | 74–76 % | 233–235 / 270–274 MB |
| Canary 180M Flash Q5_K_M (bench only, see below) | 11.76 % (28) | 3.39–3.41 s | 0.38–0.41 s | 12–14 % | 77–79 % | 272–273 / 326–330 MB |

- **v2 and Unified are one word apart.** Most of their errors are names, or a time or a sum written another way. On this clip that's close to a tie, with v2 ahead.
- **Whisper base.en is much weaker on a real voice.** It made 1.37 % of errors on the text-to-speech clips, and 6.72 % here.
- **Chunks didn't cost accuracy.** The same clip decoded in one pass on release (`--chunk 0`, one run each) gave v2 3.36 %, Unified 2.10 % and Whisper 9.24 %. So for v2 and Whisper the chunked text was better here. One pass made the text wait 1.28 / 1.36 / 0.79 s after release.
- **CPU:** v2 used less than Unified in every run, here and on the ramble.
- **GPU:** v2's first run read 79 %; its other runs read 91–92 %.
- **Canary 180M Flash** still writes numbers as words: that's 8 of its 28 errors. Without those it still made more errors than Whisper (20 against 16), and its preview showed nothing for the first 3.4 s. It stays bench only.

### Ramble, 5 minutes

Two runs for v2 and one each for the others. The figures in brackets come from the one-pass decode (`--chunk 0`, one run each):

| Model | Text ready after release (one pass) | CPU while talking | GPU while talking | Peak memory (one pass) | First words | Preview behind the voice: median / 95th pct / max | Words lost / doubled at the 13 joins |
|---|---:|---:|---:|---:|---:|---:|---:|
| **Parakeet TDT v2** | 0.35 s (7.92 s) | 33–34 % | 89 % | 807–811 MB (1,001) | 1.75–2.13 s | 0.20–0.21 / 0.69–0.73 / 1.41–1.75 s | 1 / 0 |
| Parakeet Unified | 0.33 s (8.16 s) | 39 % | 90 % | 799 MB (1,004) | 2.61 s | 0.26 / 0.74 / 2.04 s | 2 / 1 |
| Whisper base.en | 0.19 s (2.64 s) | 40 % | 75 % | 279 MB (308) | 2.20 s | 0.21 / 0.77 / 2.10 s | 1 / 0 |

- **Text ready** stays at about a third of a second, against 8 s for one pass over 5 minutes. Each model polished 13 chunks while talking, each in 0.14–0.45 s.
- **First words** count from the start of the file; the speech starts at 1.5 s.
- **The preview keeps up.** It trails the voice by about 0.2 s (median), and every minute looks the same (medians 0.17–0.30 s). No word took longer than 2.1 s to show up. A pass ran about every 0.42 s (709–713 passes), and the speech gate skipped only 6–9. The slowest pass took 0.30–0.31 s (once 0.42 s, for Whisper), inside the 0.4 s interval. How it was measured: for each word of v2's one-pass decode, which has word timings, the time of the first preview update that reached that word, minus the time the word ended.
- **Words on silence:** in both runs, Whisper's preview showed "you" at 1.4 s, on the room noise before the first word. It never reached the final text. Whisper's real first words came at 2.2 s, which is the figure in the table. Its final text had no words on silence: the few words it has that v2's one pass doesn't all sit in speech or between two close words, and are mostly one word written as two. The Parakeets wrote nothing on silence.

### Where the chunks were cut

The cuts landed at 20.5, 44.7, 71.1, 87.0, 109.4, 126.0, 150.7, 167.8, 193.9, 212.8, 228.3, 246.2 and 275.3 s, the same for every model and run. The cutter only looks at loudness, and the ticks fall inside the same pauses. That makes chunks of 15.5–29 s. The read-aloud clip was cut at 21.1, 39.2 and 55.8 s.

- **Every cut was in a real pause.** The pauses found were 0.72–1.92 s long (0.78–1.11 s on the read-aloud clip). Within 50 ms of each cut, the loudest 10 ms of audio was −40 to −54 dBFS, against −17 dBFS for speech. Around each cut the audio stayed at least 20 dB under the speech (under −37 dBFS) for 0.75–1.9 s on the ramble, and 0.5–1.0 s on the read-aloud clip. No cut was forced (that only happens after 60 s without a pause), and none fell inside a word. The one-pass word timings do put four cuts inside a word, but each of those words runs 0.7–0.9 s into the silence after it: Parakeet stretches a word over the pause that follows it.
- **The noise margin.** The quiet threshold sits 20 dB under the speech, at −37.6 dBFS here. That's 12 dB above this office's noise. An offline replay of the cutter (`pause.rs` and `Chunker::cut`, which reproduces these 13 cuts) shows how much louder the noise could get, with white noise mixed in:
  - up to −42 dBFS of added noise: the same cuts;
  - −40 dBFS: still 13 cuts, some moved, all in pauses;
  - −38 dBFS: 9 cuts, 1 of them forced;
  - −35 dBFS and up: every cut forced (6 cuts, chunks up to 60 s).

  Even the forced cuts landed in pauses, since the quietest moment usually is one. So louder rooms would mostly mean longer chunks and a slower release, not broken words.

### Joins

Each model's chunked final text was compared, by reading, with its own one-pass decode, within 2.5 s of each cut:

- **v2 lost one word in 13 joins**, the same in both runs, and doubled none. The word was at 20.5 s: a short, soft word said as the voice trailed off into the pause, at −36 to −47 dBFS. The two decodes placed it on opposite sides of the cut. The first chunk's decode put it after the cut (20.64–20.88 s), so it counted as the next chunk's word. The next chunk's decode put it 0.4 s before the cut (19.94–20.18 s), outside the 0.3 s allowed for a word the first chunk didn't keep (`SLACK_SECS` in `chunks.rs`). The only other difference near a join is a spelling. That's 1 word in 711. Across the 5 minutes, the chunked and one-pass texts differ in 19 places, 16 of them far from any join. There the two decodes heard a few words differently, and with no reference there's no telling which one is right.
- **Unified lost two words and doubled one**: at 126.0 and 275.3 s, and at 150.7 s. Each was the word just before a pause, which Unified stretches over the pause (as `chunks.rs` notes). In the doubled case, the next chunk's copy started after the first copy ended, and its middle fell past the cut, so `without_repeat` kept it.
- **Whisper lost one word, at 212.8 s:** the first word after the cut. Whisper's chunks are decoded without context, and this decode started in the pause and dropped it. The other differences near its joins are word forms.
- **The read-aloud clip:** nothing lost or doubled at its 3 joins, by any model.
- **A fix to try: widen `SLACK_SECS` from 0.3 to 0.5 s.** An offline replay of the join rules on these chunk decodes reproduces the session's text word for word at 0.3 s. At 0.5 s it recovers all three lost Parakeet words (v2's and both of Unified's), and adds nothing doubled on either recording. Unified's doubled word needs a different rule. The code is unchanged: the long text-to-speech clips should be checked first.

### Pause finder or VAD?

- **What's there.** `vad/silero_vad.onnx` (644 KB) runs only on sherpa-onnx's ONNX runtime, which the app doesn't build in (it would add 18.5 MB). In the bench (`--features sherpa`, then `--vad`), it only trims leading and trailing silence before the final decodes (`EngineConfig::vad_trim`). Nothing uses it for the cuts or the speech gate.
- **Would a VAD help?** Not for the cuts on this recording. A VAD tells speech from steady noise better than loudness does, but loudness found every pause here with 12 dB to spare. The one word v2 lost (see "Joins") was a soft word at the edge of a real pause. A VAD would still cut in that pause, and the word would still be lost: the fix for that is in the join rules. Voices or typing nearby fool both, since a VAD hears any speech, not just the speaker's.
- **Recommendation: keep the energy pause finder as it is.** If louder rooms start forcing cuts, the cheap fix is a noise-floor term in `quiet_level`: quiet = the louder of (speech − 20 dB) and (floor + 6 dB), where the floor is the 10th percentile of the window's frames. In the replay this gave the same 13 cuts on the real recording, and kept all 13 in pauses with up to −32 dBFS of added noise, where today's rule forces cuts from −38 dBFS. The speech gate uses the same threshold, so the preview would need checking too.
- **Whisper's "you" before the first word** comes from the speech gate, which decodes room noise until it has heard speech, so that quiet first words aren't missed. A plain level check would stop it. That's the same check `DESIGN.md` asks for before the final pass, and it needs no VAD.

### What it means for the model list

- **v2 stays the default.** On a real voice it was the most accurate of the three (by one word over Unified), used the least CPU, was as fast as Unified, and had fewer join problems on the ramble. Unified's small edge on public benchmarks didn't show on this voice. One speaker and two recordings can't rank v2 and Unified firmly, but nothing here argues against v2.
- **Whisper base.en** is the small-memory pick, not an accurate one: 6.7 % of errors on a real voice. Its "pick this if" line in `DESIGN.md` could say so.
- **The ledger task also asked for Parakeet Unified at 160 ms chunks** (`EASYVOX_PK_CHUNK_MS`). That setting went away with native streaming, so there was nothing to run.

### Commands

```sh
B=target/release/easyvox-bench
R=recordings      # private, gitignored
V2=parakeet-tdt-0.6b-v2-q5_k_m-gguf; UNI=parakeet-unified-en-0.6b-q5_k_m-gguf; WH=whisper-base.en-q8_0-gguf

# Read aloud: 3 rounds over the three models, in a different order each round.
$B session --model $V2 --wav $R/structured.wav --ref $R/structured-reference.txt --out s1-v2.json 2> s1-v2.err
# Ramble: 2 runs for v2, 1 for each of the others.
$B session --model $V2 --wav $R/ramble.wav --out r1-v2.json 2> r1-v2.err
# One pass on release, to compare joins and latency against.
$B session --model $V2 --wav $R/ramble.wav --chunk 0 --out r0-v2.json 2> r0-v2.err
# Canary, from a scratch models folder.
EASYVOX_MODELS_DIR=/tmp/m scripts/fetch-model.sh canary-180m-flash-q5_k_m-gguf
$B session --model canary-180m-flash-q5_k_m-gguf --models /tmp/m --wav $R/structured.wav --ref $R/structured-reference.txt
```

- **Where the numbers come from:**
  - `wer.wer`, `partials.first_partial_ms`, `finalize_ms`, `cpu.cpu_pct_of_one_core` and `chunks.polished` in the JSON;
  - memory: `mem_before_session.footprint_mb` (loaded, after the warm-up decode) and `mem_end.peak_footprint_mb`;
  - GPU: `ioreg -r -d 1 -c IOAccelerator` ("Device Utilization %") every second, leaving out the first 3 s of model load;
  - the preview's lag: the partial lines on stderr.
- **Cut times and word timings** came from a scratch copy of the bench with three `eprintln!` lines added, not committed: the pause each cut used (in `Chunker::cut`), each cut (in `polish_chunk`), and every final word with its timing. The same copy ran every session above. Each chunk's span was then decoded on its own with `transcribe --words`, to replay the join rules with other `SLACK_SECS` values.
- **The loudness checks and the noise replay** read the WAV's 30 ms and 10 ms frame levels. Noise is mixed in by adding its power to each frame.

## Limits of this bench

- **The clips can't tell the top models apart.** They're 15 short clips made with macOS text-to-speech voices (`testdata/generate.sh`): clean, evenly paced, no background noise. All three Parakeet files made the same two mistakes, both on unusual names ("Priya Rahman", "Syabhan O'Connor"). They're good for speed, memory, empty or invented output, numbers and punctuation, but not for ranking accuracy. [Real voice](#real-voice) adds two recordings of one real speaker; more speakers and rooms would still help.
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
