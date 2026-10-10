# Model bench, October 2026

Run on 9 October 2026 to settle two questions from `DESIGN.md` ("Models"):

1. **The small-model slot.** Canary 180M Flash was to replace Whisper base.en only if it came back clean: across all test clips, repeated at least 3 times, no empty or cut-short transcripts, a word error rate no worse than Whisper base.en's, and no crash or hang.
2. **Parakeet TDT 0.6B v2's quantization.** Can Q5_K_M (539 MB) replace Q8_0 (730 MB) without losing accuracy or speed?

Later the same day the engine moved to one pipeline for every model (no native streaming, long dictations polished chunk by chunk; see `DESIGN.md`, "Live text while speaking"). [One pipeline](#one-pipeline) re-runs the session numbers for the three offered models, and [Long dictation](#long-dictation) adds recordings of 3 and 9 minutes. Those re-runs made Parakeet TDT v2 the default in place of Parakeet Unified (see [What we decided](#what-we-decided)). [Idle and energy](#idle-and-energy) covers the app while it waits, and the preview while you pause. [Real voice](#real-voice) runs the three models on the author's own voice, and checks the chunk cuts and joins on a 5-minute ramble in an office. [Lightness](#lightness) then made the preview about six times lighter: it now re-decodes only the words that may still change, every 1.6 s instead of every 0.4 s. The sections before it ran the old preview. [First words and silence](#first-words-and-silence) brought the first words back to about half a second after you start speaking, and stopped decoding recordings that hold no speech. [Parakeet TDT v3 against v2](#parakeet-tdt-v3-against-v2) then ran v2's multilingual successor on the same two recordings, with those defaults.

## Results

| Model (file) | Download | Word error rate | Empty or failed | 2 s of silence | First words | Text ready after release (3 / 11 / 30 s clip) | CPU while talking | Speed (RTF) | Memory, loaded / peak |
|---|---:|---:|---:|---|---:|---:|---:|---:|---:|
| **Parakeet Unified EN 0.6B Q5_K_M** (alternative) | 541 MB | 0.55 % | 0 of 84 | nothing | 1.35 s | 0.09 / 0.18 / 0.46 s | 7–9 % | 0.015 | 693 / 743 MB |
| Parakeet TDT 0.6B v2 Q8_0 (dropped) | 730 MB | 0.55 % | 0 of 84 | nothing | 0.45 s | 0.06 / 0.16 / 0.42 s | 18–37 % | 0.014 | 879 / 924 MB |
| **Parakeet TDT 0.6B v2 Q5_K_M** (default) | 539 MB | 0.55 % | 0 of 84 | nothing | 0.47 s | 0.07 / 0.17 / 0.44 s | 15–37 % | 0.015 | 691 / 735 MB |
| Parakeet TDT 0.6B v3 Q5_K_M (bench only, 10 October) | 549 MB | 1.37 % | 0 of 84 | "Thank you." | – | – | – | 0.017 | 716 / 806 MB |
| Canary 180M Flash Q5_K_M (bench only) | 159 MB | **7.12 %** | 0 of 84 | nothing | 0.45 s | 0.09 / 0.20 / 0.51 s | 5–14 % | 0.016 | 243 / 312 MB |
| **Whisper base.en Q8_0** (alternative) | 85 MB | 1.37 % | 0 of 84 | "you" | 0.51 s | 0.17 / 0.17 / 0.52 s | 31–36 % | 0.014 | 205 / 259 MB |

How to read it:

- **Word error rate:** the share of words that came out wrong, over the 14 speech clips, 6 runs each. Lower is better. It ignores punctuation and case, and counts "10" and "ten" as the same, but not "342" and "three hundred and forty two".
- **Empty or failed:** decodes of a speech clip that returned no text or an error (14 clips × 6 runs = 84).
- **2 s of silence:** what the model wrote for a clip with no speech, decoding it on its own. It should write nothing. The app no longer decodes such a recording at all (see [First words and silence](#first-words-and-silence)).
- **First words:** how long after you start talking the live preview shows its first words.
- **Text ready after release:** from the end of the speech (key released) to the final text.
- **CPU while talking:** share of one CPU core during the session, live preview included. The GPU does most of the work and isn't counted.
- **Speed (RTF):** decode time divided by audio length. 0.015 means a 10 s recording takes 0.15 s.
- **Memory:** the process's memory footprint once the model is loaded, and its peak after decoding every clip.
- **First words, text ready and CPU** in this table come from the old engine, where Parakeet Unified streamed natively. [One pipeline](#one-pipeline) has the current numbers.
- **Parakeet TDT v3** was added on 10 October, after the old engine was gone, so it has no first-words, text-ready or CPU figures here: see [Parakeet TDT v3 against v2](#parakeet-tdt-v3-against-v2). The same 6 runs of each clip, with v2 run alongside (v2 repeated its 0.55 % and its text; 649 / 737 MB that day). v3 made 5 errors per run to v2's 2: two of the three names on `names_tessa` (v2 misses two as well, one of them the same), "Kowolski" for "Kowalski", "15th" for "15" and "the full copy" for "a full copy". Like v2 it wrote the numbers as digits. Decoded on its own, 2 s of silence came out as "Thank you."; the app no longer decodes a recording without speech (see [First words and silence](#first-words-and-silence)).

## What we decided

**Default: Parakeet TDT 0.6B v2 replaces Parakeet Unified** (decided after the [one pipeline](#one-pipeline) and [long dictation](#long-dictation) re-runs below). Unified had been the default because it streamed the preview natively; with one pipeline both models re-decode, and they cost the same and show first words as fast. v2 then made fewer errors on long dictations (0.40 % against 0.86 % over 9 minutes) and ended every clip with a full stop, where Unified left it out on 4 of 13. Unified stays as an alternative: it scores a little better on public benchmarks, which these text-to-speech clips can't confirm or rule out.

**Small slot: Whisper base.en stays; Canary 180M Flash is bench only.** Canary was clean on two of the three counts: no empty transcripts (the [transcribe.cpp #140](https://github.com/handy-computer/transcribe.cpp/issues/140) bug never showed up), nothing written on silence, and no crash or hang. But on transcribe-cpp 0.3.1 it writes every number as words: "$342" came out as "three hundred and forty two dollars", and a phone number as "five five five zero one four two". The runtime has no number-formatting (ITN) switch for Canary. On the three number clips that's 57 % WER, against 0 % for every other model. On the other eleven clips Canary and Whisper tie (1.52 % each). So it fails the "no worse than Whisper base.en" bar (7.12 % vs 1.37 %), and digits matter for dictation. Its Q8_0 file wasn't tried: that test was only for empty transcripts, which didn't happen, and quantization can't add number formatting.

Whisper base.en's own flaw showed up too: it wrote "you" for 2 s of silence and added a stray "Mhm." after one long clip. The app should skip the final pass when a recording holds no speech. (It does now, for every model: see [First words and silence](#first-words-and-silence).)

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
- **The settings file** (2026-10-10) is watched with FSEvents, which wakes the app only when its folder changes. Measured the same way, 60 s each, with the file watched: 0.3 wake-ups a second with the model loaded, after the unload, and after an outside edit to the file (one reload, then nothing); 0.002 % of a core; 665 MB loaded, as before.

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
- **A fix to try: widen `SLACK_SECS` from 0.3 to 0.5 s.** An offline replay of the join rules on these chunk decodes reproduces the session's text word for word at 0.3 s. At 0.5 s it recovers all three lost Parakeet words (v2's and both of Unified's), and adds nothing doubled on either recording. Unified's doubled word needs a different rule. Fixed since, with a join that matches the two decodes' words instead: see [Chunk joins](#chunk-joins).

### Chunk joins

The same evening, the join itself changed (`Polished::continuation` in `chunks.rs`). It used to go by timing alone: a word near the cut went to whichever side its middle fell on, with 0.3 s of slack and a check for a repeated first word. Now it first matches the new chunk's words with what the previous decode heard around the cut: the polished words in the new decode's left context, and the words the previous decode heard past the cut, which it used to throw away. Two words match if they have the same text and lie within 1 s of each other (`MATCH_SECS`). Words up to the last match with a polished word are dropped, a word matching the previous decode's words past the cut is kept, whichever side the new decode puts it, and any other word follows the old timing rule. Same rules for every model; Whisper's chunks have no context, so they're still simply joined.

Why not just widen the slack to 0.5 s: it recovers the three lost Parakeet words, but v2's sat 0.40 s before the cut (a 0.1 s margin), and it can't fix Unified's doubled word, since telling that apart from a word said twice needs what the previous decode heard after the cut. Matching works however far apart the two decodes put the word, up to 1 s, and needs no per-model setting.

Before → after, one run each after the change. The "before" ramble and read-aloud figures are the runs above; the text-to-speech clips were run fresh on the old code.

| Model | Ramble: words lost / doubled at the 13 joins | Read aloud: word error rate | `lf_3m` | `lf_3m_pauses` | `lf_10m` (9 min) | Text ready after release: ramble / read aloud |
|---|---:|---:|---:|---:|---:|---:|
| **Parakeet TDT v2** | 1 / 0 → **0 / 0** | 1.68 → 1.68 % | 0 → 0 % | 0 → 0 % | 0.40 → 0.40 % | 0.35 → 0.36 s / 0.37–0.42 → 0.42 s |
| Parakeet Unified | 2 / 1 → **0 / 0** | 2.10 → 2.10 % | 0.30 → 0.30 % | 0 → 0 % | 0.86 → 0.86 % | 0.33 → 0.51 s (0.37 s on a second run) / 0.38–0.41 → 0.41 s |
| Whisper base.en | 1 / 0 → 1 / 0 | 6.72 → 6.72 % | – | – | – | 0.19 → 0.19 s / 0.23–0.40 → 0.25 s |

- **Ramble:** all four join mistakes are gone, the only change to the text. Nothing else differs within 2.5 s of a cut, apart from one word v2 spells differently (see above). The cuts were the same 13.
- **Whisper's lost word stays.** It isn't a join mistake: the next chunk's decode, which starts in the pause and has no context, didn't write it. Fixing it needs context for Whisper, and so word timings.
- **Read aloud:** each model wrote exactly the same text as before.
- **Text-to-speech clips:** the same word error rates. On `lf_3m` and `lf_10m` each model wrote the same text as before, from the same cuts. On `lf_3m_pauses` a few cuts landed up to 0.11 s elsewhere (the ticks fall at slightly different moments in its 2.5–8 s pauses), which changed a few commas for v2. Replaying the old join on those decodes gives the same text, so that wasn't the join.
- **Text ready after release** doesn't depend on the join, which takes well under a millisecond. Where an after figure is higher (Unified's ramble at 0.51 s; 0.40–0.49 s on the text-to-speech clips, against 0.35–0.49 s before), most of it is a 68–156 ms wait for a preview pass still running at release: the text-to-speech clips end on their last word, so one often is. The decode after release took as long as before: 0.34–0.42 s on those clips, against 0.35–0.40 s. Run again, Unified's ramble took 0.37 s and v2's `lf_3m` 0.40 s, with no wait.
- **Cuts through words.** Real cuts land in pauses, so joins are rarely tested hard. An offline replay cut through the middle of a word every 10–20 s instead: 56 cuts per model over the ramble, the read-aloud clip and `lf_3m`, each chunk decoded with 1 s of context as in the app. The old join doubled 17 words for Unified and 1 for v2. The new one doubled 1 (a word Unified's next decode placed 1.3 s from where the previous one had it), and every other word it changed was one of those doubles.
- **How it was checked:** the join was replayed offline (in Python) on every session's chunk decodes, logged by the scratch bench copy. The replay reproduces each session's text word for word, with the old rules on the old runs and the new rules on the new runs. On the old runs, the new rules changed only the four words above. Any `MATCH_SECS` from 0.5 to 2 s gives the same text on all of it, except the one doubled word in the replay with cuts through words.

### Pause finder or VAD?

- **What's there.** `vad/silero_vad.onnx` (644 KB) runs only on sherpa-onnx's ONNX runtime, which the app doesn't build in (it would add 18.5 MB). In the bench (`--features sherpa`, then `--vad`), it only trims leading and trailing silence before the final decodes (`EngineConfig::vad_trim`). Nothing uses it for the cuts or the speech gate.
- **Would a VAD help?** Not for the cuts on this recording. A VAD tells speech from steady noise better than loudness does, but loudness found every pause here with 12 dB to spare. The one word v2 lost (see "Joins") was a soft word at the edge of a real pause. A VAD would still cut in that pause, and the word would still be lost: the fix for that is in the join rules. Voices or typing nearby fool both, since a VAD hears any speech, not just the speaker's.
- **Recommendation: keep the energy pause finder as it is.** If louder rooms start forcing cuts, the cheap fix is a noise-floor term in `quiet_level`: quiet = the louder of (speech − 20 dB) and (floor + 6 dB), where the floor is the 10th percentile of the window's frames. In the replay this gave the same 13 cuts on the real recording, and kept all 13 in pauses with up to −32 dBFS of added noise, where today's rule forces cuts from −38 dBFS. The speech gate uses the same threshold, so the preview would need checking too.
- **Whisper's "you" before the first word** comes from the speech gate, which decodes room noise until it has heard speech, so that quiet first words aren't missed. A plain level check would stop it. That's the same check `DESIGN.md` asks for before the final pass, and it needs no VAD. (Done: nothing is decoded before speech now; see [First words and silence](#first-words-and-silence).)

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

### Parakeet TDT v3 against v2

Run on 10 October 2026, same M4, at commit f424519: today's defaults, after [Lightness](#lightness) and [First words and silence](#first-words-and-silence), so these numbers compare with each other, not with the tables above. Parakeet TDT 0.6B v3 is v2's multilingual successor (25 European languages, the same size, CC-BY-4.0). It's a little behind v2 on public English benchmarks (Open ASR Leaderboard average 6.82 against 6.43) and had never been run on the author's voice. It's in the catalog as bench only, and v2 stays the default.

Same recordings and methods as above: real-time `session` replays, the models taking turns in a different order each round, each run started on a quiet machine (load average 1.3–2.2, nothing compiling). Energy as in [Lightness](#lightness) (macmon, CPU + GPU + Neural Engine + DRAM over the idle machine). Each model wrote the same final text in every run of the same recording.

**Read aloud** (81 s, 3 runs each; one pass on release, `--chunk 0`, one run each):

| Model | Word error rate (one pass) | First words, after speech starts | Text ready after release (one pass) | CPU while talking | Memory, loaded / peak |
|---|---:|---:|---:|---:|---:|
| **Parakeet TDT v2** | **1.68 %** (4 words; 3.36 %, 8) | 0.61–0.62 s | 0.36–0.42 s (1.31 s) | 9.4–9.6 % | 705–706 / 740–742 MB |
| Parakeet TDT v3 | 2.10 % (5 words; 3.78 %, 9) | 0.62–0.65 s | 0.41–0.46 s (1.44 s) | 13.1–13.4 % | 772–773 / 808–811 MB |

**Ramble** (5 minutes, 2 energy runs each; one pass, one run each):

| Model | Energy per minute | GPU power added | CPU while talking | Text ready after release (one pass) | Peak memory (one pass) | First words | Preview behind the voice: median / 95th pct | Words lost / doubled at the 13 joins | Hesitations written |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| **Parakeet TDT v2** | 41.5 J (37.8–45.2) | 0.52–0.53 W | 7.6–8.0 % | 0.33–0.35 s (7.92 s) | 763–783 MB (950) | 2.14–2.16 s | 0.76–0.78 / 1.96–2.05 s | 0 / 0 | 0 |
| Parakeet TDT v3 | 40.9 J (39.5–42.3) | 0.51–0.54 W | 10.6–11.0 % | 0.38–0.43 s (8.28 s) | 833–842 MB (1,018) | 2.13–2.14 s | 0.81–0.87 / 2.33–2.49 s | 0 / 0 | 25 |

- **Accuracy: about a tie, v2 a word ahead.** On the read-aloud clip they share three errors (two names and a sum of money). v2's fourth is a plural heard as a singular. v3 wrote another sum of money in words instead of digits, which the scorer counts as two errors. On the text-to-speech clips v2 was ahead too (0.55 % against 1.37 %; see [Results](#results)).
- **v3 writes hesitations.** On the ramble it wrote 25 hesitation sounds as words in the chunked text (27 in one pass), where v2 wrote none. That's most of its 30 extra words (742 against 712). Dictated text would need them taken out again. On the read-aloud clip it wrote none. Apart from them, the two models' ramble texts differ in 25 places, and with no reference there's no telling which is right.
- **v3 costs a little more.** About 40 % more CPU while talking (a few points of one core), about 70 MB more memory, and text ready 40–75 ms later after release: its decodes are about 5–10 % slower (preview passes 119–125 against 111–113 ms on the ramble). **Energy is a tie:** the GPU's power is the same, over the same 185 preview passes, and the totals differ by less than the CPU rail's noise.
- **The preview.** First words come as soon. The median lag is the same within 0.1 s; the 95th percentile is 0.3–0.4 s longer for v3. Each model's lag is measured against its own one-pass decode; against v2's, v3's is 0.76–0.79 / 2.28–2.47 s.
- **Joins.** Both cut at the same 13 places as on 9 October. Neither lost or doubled a word at a join. Near three of v3's cuts its chunked text has a short word its one-pass decode doesn't. One is a word repeated across a pause, which both chunk decodes, each decoded on its own, also heard twice: the one-pass decode heard it once. The other two lie 0.8 and 1.5 s before a cut, inside one chunk, and were simply heard differently. v2's only difference near a cut is one word written as two.
- **Language.** v3 picks the language itself. It stayed in English throughout: no accented letter and no foreign word in any final text or any preview update, on either recording or on the text-to-speech clips.
- **What it means:** on this voice v3 isn't more accurate than v2, writes hesitations, and costs a little more CPU and memory. v2 stays the default. v3's case would be the other languages, which this bench doesn't test.

```sh
B=target/release/easyvox-bench      # a scratch copy printing the cut and word lines described above
R=recordings                        # private, gitignored
V2=parakeet-tdt-0.6b-v2-q5_k_m-gguf; V3=parakeet-tdt-0.6b-v3-q5_k_m-gguf
scripts/fetch-model.sh $V3
# Read aloud: 3 rounds (v2 then v3, v3 then v2, v2 then v3), then one pass each.
$B session --model $V3 --wav $R/structured.wav --ref $R/structured-reference.txt --out s1-v3.json 2> s1-v3.err
$B session --model $V3 --wav $R/structured.wav --ref $R/structured-reference.txt --chunk 0 --out s0-v3.json
# Ramble: energy runs as in "Lightness" (macmon around the run), 2 rounds (v3 then v2, v2 then v3).
macmon pipe -i 500 > r1-v3.jsonl & sleep 60
$B session --model $V3 --wav $R/ramble.wav --idle-before 10 --out r1-v3.json 2> r1-v3.err
sleep 60; kill %1
# One pass, for the joins and the preview's lag.
$B session --model $V3 --wav $R/ramble.wav --chunk 0 --out r0-v3.json 2> r0-v3.err
# Text-to-speech clips: 3 runs of each, twice, in reverse model order the second time.
$B bench --model $V3 --reps 3 --out tts-v3.json
```

## Lightness

Run on the night of 9–10 October 2026, same M4 (a Mac mini, on AC), for the requirement "Light on the machine while you dictate" in `DESIGN.md`: make dictation as light as it can be, while the final text stays as accurate and as quick, and the preview's words still show up within about 2 s most of the time.

**What changed** (`Tuning` in `crates/easyvox-core/src/tuning.rs`; the same for every model):

- **Each preview pass starts at the confirmed words**: 1 s before the last word two passes agreed on (`preview_context_ms`, new), never before the last chunk cut. It used to start at the last cut and grow to 20 s, so every pass decoded again up to 20 s of words that were already settled. The preview's GPU energy follows the audio it decodes (about 0.13 J per second of audio here), so this alone cut a dictation's energy by three-quarters, at the same 400 ms.
- **A pass every 1.6 s instead of 0.4 s** (`preview_interval_ms`).
- Nothing else. The 20 s cap on a pass (`preview_window_ms`) now rarely matters (8 s changed nothing), and chunks stay at 30 s.

**The runs.** `ramble.wav` (5 minutes of the author rambling, see [Real voice](#real-voice)) replayed in real time through `easyvox-bench session` with Parakeet TDT v2: the app's code path. Each setting ran at least twice, in two rounds (the second in reverse order), each run on a quiet machine. Word errors come from the 81 s read-aloud clip and its reference, one run per setting (two for the new default). Figures are means, with the range over runs where it matters.

| Setting | Energy per minute of dictation | Added power: all / GPU | GPU active | CPU, share of one core | Preview behind the voice: median / 95th pct / max | Text ready after release: ramble / read aloud | Read aloud: word errors |
|---|---:|---:|---:|---:|---:|---:|---:|
| **Before:** a pass every 0.4 s, from the last cut (up to 20 s) | 254 J (250–259) | 4.23 / 3.56 W | 31 % | 36 % | 0.23 / 0.73 / 1.4 s | 0.34 / 0.41 s | 1.68 % |
| Before, a pass every 1.2 s | 98 J (90–105) | 1.62 / 1.38 W | 12 % | 14 % | 0.63 / 1.55 / 3.4 s | 0.34 / 0.39 s | 1.68 % |
| Before, passes of at most 8 s | 118 J | 1.96 / 1.65 W | 19 % | 26 % | 0.20 / 0.71 / 1.8 s | 0.36 / 0.39 s | 1.68 % |
| Before, 15 s chunks | 232 J | 3.87 / 3.31 W | 30 % | 36 % | 0.21 / 0.68 / 1.8 s | 0.20 / 0.23 s | 1.68 % |
| From the confirmed words, every 0.4 s | 58 J | 0.96 / 0.79 W | 13 % | 17 % | 0.11 / 0.72 / 1.9 s | 0.33 / 0.41 s | 1.68 % |
| … every 0.8 s | 44 J (38–50) | 0.73 / 0.64 W | 8 % | 11 % | 0.34 / 1.07 / 2.0 s | 0.37 / 0.39 s | 1.68 % |
| … every 1.2 s | 40 J (37–43) | 0.67 / 0.57 W | 7 % | 9 % | 0.61 / 1.55 / 3.9 s | 0.41 / 0.38 s | 1.68 % |
| **… every 1.6 s (the new default, 4 runs)** | **39 J (35–45)** | **0.65 / 0.52 W** | **6 %** | **8 %** | **0.81 / 1.94 / 4.2 s** | **0.35 / 0.38 s** | **1.68 %** |
| … every 2 s | 34 J | 0.56 / 0.49 W | 5.5 % | 7 % | 1.00 / 2.15 / 4.0 s | 0.36 / 0.39 s | 1.68 % |
| No preview at all (chunks only) | 16 J (14–17) | 0.26 / 0.15 W | 1.4 % | 1.3 % | – | 0.36 s / – | – |

Side checks at 1.2 s, two runs each: starting passes 2 s or 3 s before the confirmed words took 47 J and 53 J a minute (GPU 0.65 and 0.78 W); capping passes at 8 s changed nothing (44 J, GPU 0.58 W, the same lag); 20 s chunks took 43 J and made the final text quicker (0.30 / 0.23 s) but less accurate (see below).

- **Energy** is the SoC's (CPU + GPU + Neural Engine + DRAM) on top of the idle Mac, per minute of audio, from `macmon` (see "How it was measured"). The GPU column is the cleanest: the GPU is fully off at idle. The CPU rail swings by ±0.1 W with whatever else this shared Mac is doing (an iOS simulator, other agents, macOS's malware scans), so totals within about 5 J of each other are a tie; the GPU column shows the real order. For scale, this Mac mini draws about 3 W at idle by its own estimate (SMC), and that whole-machine estimate tells the same story: about 460 J a minute before, about 75 now. The fan never left its idle 1,000 rpm, before or after.
- **The knee is at 1.2–1.6 s.** From 0.4 to 1.2 s the GPU's power fell by more than a quarter. From 1.2 to 1.6 s it fell by a tenth, and on to 2 s by another 6 %, while the median lag grew by 0.2 s at each of those steps and its 95th percentile reached 2 s. Savings shrink because passes get longer as they get rarer (2.4 s of audio each at 0.4 s, 4.8 s at 1.6 s): words wait longer to be confirmed. 1.6 s is the lightest setting that keeps the preview within about 2 s most of the time. What's left is partly fixed: with no preview at all, polishing the chunks still costs about 16 J a minute.
- **"GPU active"** is macmon's `gpu_active_ratio`. macOS's "Device Utilization %" (`ioreg`), which earlier sections quote, barely moves (91 % before, 74 % now) although the GPU's power fell sevenfold, so it says little about cost.
- **The preview's lag** is measured per word, as in [Real voice](#real-voice). Its 95th percentile grows with the interval; the maximum (about 4 s from 1.2 s on) comes from one hard stretch 23–28 s into the ramble, where for a few passes the preview's newest words weren't the words of the reference decode: text was on screen, just not yet the right words. With a 1 s run-up the newest words have less of their sentence. Giving them 2 or 3 s instead barely changed the lag (95th percentile 1.65 and 1.49 s against 1.55 s, maximum 3.3 and 3.5 s against 3.9 s) and took 14 % and 37 % more GPU energy. The preview is a little rougher: of the confirmed preview words a polished chunk later replaced, 4.5 % differed (3.8 % before).
- **The final text is identical**: every preview-only setting wrote, word for word, the same ramble as before, and the same read-aloud text (1.68 %, 4 errors). Chunk length is the one knob that touches it: 15 s chunks changed the ramble in 17–19 places (24–25 joins instead of 13), and 20 s chunks made 6 errors on the read-aloud clip instead of 4, so chunks stay at 30 s.
- **Text ready after release** holds on long dictations and loses about 25 ms on short ones. Release latency over repeated runs, today's build and defaults against the new ones, alternating:

  | Recording | Before | Now |
  |---|---:|---:|
  | 11 s clip (`punct_samantha`, 8 runs each) | 0.18 s (0.15–0.20) | 0.20 s (0.17–0.22) |
  | 30 s clip (`long_daniel`, 8 runs each) | 0.46 s (0.42–0.50) | 0.46 s (0.45–0.47) |
  | 81 s read aloud | 0.41 s | 0.37–0.38 s |
  | 5-minute ramble (2 and 4 runs) | 0.34 s (0.34–0.35) | 0.35 s (0.32–0.37) |
  | `lf_3m_pauses` / `lf_10m` (one run each) | 0.40–0.49 / 0.37–0.44 s (earlier sections) | 0.38 / 0.38 s |

  The short clip's final decode took 0.21 s instead of 0.16 s. The old preview kept the GPU busy about 40 % of the time, so the final decode started on a GPU at full speed; after a light preview it starts on one that has slowed down. It isn't the windows or the memory: passes every 0.4 s with the new short windows gave 0.20 s, and keeping ggml's residency sets (`EASYVOX_METAL_RESIDENCY=1`) 0.19 s. The old preview paid some of it back in waiting: in 5 of 8 releases on the 11 s clip a pass was still running (up to 49 ms), and in none of the 16 new ones.
- **Release waiting for a preview pass.** transcribe.cpp checks its cancel flag only before a Parakeet decode starts (`parakeet/model.cpp`, "the single observation point on this path"), so a release can't stop a pass midway. Instead, passes are now rarely running: about 7 % of the time, for 0.11 s on average, against 40 % and 0.17 s before.
- **Other models:** Parakeet Unified and Whisper base.en wrote the same read-aloud text with the new defaults as before (2.10 % and 6.72 %), and their final text came as fast (Unified 0.40 → 0.41 s, Whisper 0.36 → 0.35 s). Whisper's preview starts its passes 1 s before the confirmed words too, although it only times segments; its preview words were, if anything, closer to its final text than before. Its CPU while talking fell from 52 % to 12 % of a core, and Unified's from 49 % to 10 %.
- **Long text-to-speech clips** (v2, new defaults): `lf_3m_pauses` 0 % and `lf_10m` 0.40 %, the same as in [Chunk joins](#chunk-joins).
- **The Neural Engine.** The rule for moving Parakeet's encoder to the Neural Engine is that it at least halves energy per minute against the best GPU setup. That's now about 39 J a minute, so the Neural Engine path would have to come in under about 20 J, while polishing the chunks with no preview at all already costs about 16 J on the GPU, CPU and memory. It could only take over the encoder's part of the remaining 0.65 W, and its encoder has a fixed 15 s shape that a 5 s preview pass would pay for in full. Halving looks unlikely, and the most it could save is about 0.3 W.

**How it was measured.** Each run: `macmon pipe -i 500` (CPU, GPU, Neural Engine and DRAM power, about every 0.55 s; no sudo) for 60 s of idle machine, then the bench (model load, a warm-up decode, 10 s idle, the real-time replay to its final text), then 60 s idle again. Before every run a script waited for a quiet machine: load average under 2.5, nothing building or indexing, a 15 s probe with the GPU off and the CPU under 1 W, and the CPU back near its idle temperature. Energy is each sample's power times its interval, summed over the session (from the replay's start to the final text). The idle level is subtracted: for the GPU, Neural Engine and DRAM, the mean of the idle minutes; for the CPU, whose idle level jumps with other processes, the median of 5 s blocks during the session minus the median during the idle minutes. The runs and scripts stayed in the session's scratch folder, since the bench's output holds the private recording's text.

```sh
B=target/release/easyvox-bench
R=recordings      # private, gitignored
M=parakeet-tdt-0.6b-v2-q5_k_m-gguf

# One energy run.
macmon pipe -i 500 > run.jsonl & sleep 60
$B session --model $M --wav $R/ramble.wav --idle-before 10 --out run.json 2> run.err
sleep 60; kill %1

# "Before" rows: the build from commit da36537, with its defaults or one of
#   --set preview_interval_ms=1200 | --set preview_window_ms=8000 | --set chunk_ms=15000
# New rows: this build, with its defaults (a pass every 1.6 s, 1 s of context) or one of
#   --set preview_interval_ms=400|800|1200|2000 | --set preview_interval_ms=0 (no preview)
#   --set preview_interval_ms=1200 plus --set preview_context_ms=2000|3000,
#   --set preview_window_ms=8000 or --set chunk_ms=20000

# Word errors, with any of the settings above.
$B session --model $M --wav $R/structured.wav --ref $R/structured-reference.txt --out read.json
# Release latency, alternating the two builds.
$B session --model $M --wav testdata/punct_samantha.wav --out lat.json
```

- **Where the numbers come from:** energy and "GPU active" (`gpu_active_ratio`) from `run.jsonl`; CPU from `cpu.cpu_pct_of_one_core`, release latency from `finalize_ms` (and `final_wait_ms`, `final_decode_ms`), passes from `partials.passes`, their mean length from `partials.window_secs_mean` (new), word errors from `wer.wer`.
- **The preview's lag:** for each word of v2's one-pass decode of the ramble (from [Real voice](#real-voice), with word timings), the time of the first preview update (the partial lines on stderr) whose last three words match the decode at that word, minus the time the word ended.

## First words and silence

Run on the morning of 10 October 2026, same M4, against the build before (commit 6e59ccc) with the same defaults otherwise. Two changes, the same for every model (`Tuning` in `crates/easyvox-core/src/tuning.rs`; `DESIGN.md`, "Live text while speaking"):

- **No speech, no decode.** Until a recording holds speech, nothing is decoded: no preview pass, no chunk, no final pass. A recording that never does ends with empty text. Speech is 120 ms in a row of 30 ms frames at least 6 dB louder than the room (the recording's 10th percentile, leaving out digital silence), or louder than -35 dBFS whatever the room, never at or under -55 dBFS (`speech_above_noise_db`, `speech_loud_dbfs`, `speech_min_ms`, `gate_silence_dbfs`; `SpeechDetector` in `pause.rs`).
- **The first preview pass runs 0.5 s after speech starts** (`preview_first_ms`), instead of 1.6 s after the key press. Until the preview shows words, the next passes come 0.5 s apart too, then every 1.6 s.

Real-time `session` replays on Parakeet TDT v2 unless a model is named, the two builds alternating, each run waiting for a quiet machine:

| | Before | After |
|---|---:|---:|
| First words, the 13 text-to-speech clips (speech starts at the key press; 3–8 runs each) | 1.70 s (1.66–1.73) | **0.58 s** (0.55–0.69) |
| First words after speech starts, 28 dictation starts cut from the two real recordings (median, slowest) | 0.98 s, 1.25 s | **0.58 s, 0.62 s** |
| … Parakeet Unified | 1.03 s, 2.21 s | **0.60 s, 1.63 s** |
| … Whisper base.en | 1.02 s, 1.27 s | **0.62 s, 0.72 s** |
| First words, 5-minute ramble (speech starts 1.5 s in; 3 runs each) | 3.39 s | **2.17 s** |
| Energy per minute, 5-minute ramble (3 runs each) | 37.7 J (34.6–41.2) | 39.0 J (36.8–41.0) |
| GPU power added / GPU active / CPU, share of one core | 0.53 W / 6.1 % / 7.8 % | 0.52 W / 6.1 % / 7.8 % |
| Preview behind the voice: median / 95th pct (ramble) | 0.80–0.86 / 1.77–1.95 s | 0.76–0.80 / 1.91–1.93 s |
| Read aloud, word errors: v2 / Unified / Whisper | 1.68 / 2.10 / 6.72 % | 1.68 / 2.10 / 6.72 %, the same text |
| `lf_3m_pauses`, word errors: v2 / Unified / Whisper | 0 / 0 / 0.52 % | 0 / 0 / 0.87 % (see below) |
| Text ready after release: 11 s / 30 s clip (8 runs each, median) | 0.21 / 0.46 s | 0.19 / 0.45 s |
| … ramble (3 runs) / read aloud, v2 (1 run) | 0.32–0.36 / 0.39 s | 0.34–0.36 / 0.38 s |

- **First words.** A first pass on 0.3 s of speech had nothing to show yet on one short clip in eleven; on 0.4 s and 0.5 s, v2's first pass showed words on all 28 real starts. Parakeet Unified needs more: at 0.5 s its first pass came back empty on 8 of the 28, and with one pass every 1.6 s after it, those first words took 2.2 s. So until the preview shows words, passes come every 0.5 s. That costs v2 and Whisper nothing (their first pass shows words) and brought Unified's slowest start under 1.2 s; with the final 6 dB detection one start, where a breath before the first word counted as speech, took 1.63 s. (The starts are counted from the first 120 ms above -40 dBFS.) The "before" figures depend on where speech starts against the 1.6 s ticks; counted from the key press, they were about 1.7 s at best. Before, on one of the 28 starts (the 2.7 s of room before the read-aloud clip's first word), v2's and Whisper's previews showed a word made up on the room noise.
- **Energy** is measured as in [Lightness](#lightness) ("energy per minute", the CPU rail taken robustly). The two builds run the same 185 preview passes over the ramble and write the same text; the GPU's power is the same, and the totals differ by the CPU rail's usual noise. Within 5 % of the 39 J a minute measured there. (The energy runs came before passes repeated every 0.5 s until words show; v2's first pass shows words, and the final build ran the same 185 passes over the ramble.)
- **Final text.** Every model wrote the same read-aloud text as before, and v2 the same ramble in all 7 runs. Over the 13 text-to-speech clips the text was the same in every run of both builds. On `lf_3m_pauses` the Parakeets made no errors either way. Whisper base.en made 3 or 5 errors in 570 words, in both builds: a second run of each flipped them (before 0.87 %, after 0.52 %), and so did starting the passes at other times (0.9, 1.3 and 1.6 s: 0.52, 0.87 and 0.87 %). Chunks are cut at ticks, so where they fall moves with the ticks' timing, and Whisper, decoded without context at cuts, is the model that notices.
- **Text ready after release** is unchanged within the spread. On `lf_3m_pauses` (one run each) it came 0.38 → 0.42 s for v2, 0.47 → 0.43 s for Unified and 0.42 → 0.40 s for Whisper, in the 0.38–0.49 s seen in earlier sections.

**Silence and room noise**, per model: what the final text held, before and after. The room noise is cut from the two private recordings: their first 1.45 s (1 s of digital silence, then the room), 5 s of the ramble's longest pauses, and the 2.35 s before the first word of the read-aloud clip.

| | Parakeet TDT v2 | Parakeet Unified | Whisper base.en |
|---|---|---|---|
| `silence_2s.wav`, before | nothing (one decode) | nothing (one decode) | "you" |
| Room noise (3 clips), before | 1 made-up word on one; preview passes on two | nothing, but preview passes on two (one showed a word) | 1 made-up word on two; preview passes on two |
| After | **nothing**; no decode on three, the breaths in the pauses decoded | **nothing**; the same | **nothing**; the same |

- **Short speech still comes through.** The four commands (`cmd_*`) wrote the same text with every model. So did the first word or two of each real recording, cut short (1.3 and 2 s), and copies of them 20 to 30 dB quieter, as from a microphone turned well down: the detector put the start of their speech where it did at full level.
- **The app** pastes nothing for empty text, leaves the clipboard and History alone, and shows "Didn't catch anything" for 2.5 s. The self-test now checks that with the silent clip, and `silence_and_room_noise_give_empty_text_on_every_offered_model` (an ignored engine test) runs the silent clip and a -50 dBFS noise clip through every offered model.
- **The levels.** Room noise in the two recordings sits around -50 to -60 dBFS, with peaks up to -40, and their speech around -17. The text-to-speech clips start speaking on their first frame, with no room to measure, which is what the -35 dBFS level is for. The first default asked for 20 dB over the room, which these recordings clear by far; quieter microphones don't (next section), so it's 6 dB. A steady room stays under that: the loudest 120 ms of the silent clip, the two recordings' quiet starts held 0, 3.7 and 5.1 dB over their 10th percentile. The ramble's pauses, with breaths in them, held 13 dB, so they're decoded now: every model wrote nothing.

### Quiet mics and loud rooms

A false "no speech" loses a whole dictation without a word, which is far worse than a stray "you", so the detector was stress-tested on speech much quieter or noisier than the author's recordings. Synthetic cases (scratch only): the read-aloud clip, five short clips (`cmd_*`, `num_rishi`) and the 28 real starts, turned down by 10, 20, 25 and 30 dB over pink noise at -62 dBFS (a laptop's own mic at arm's length); "edge" copies whose loudest 30 ms sits at -45 dBFS, 10 dB over a -55 dBFS room; the same clips under pink noise at -45, -40 and -35 dBFS or four voices babbling at -40; and "OK" and "Yes" from three voices, their loudest 30 ms at -40 or -45 dBFS, over a room at -62 or -55, with and without 0.4 s of room before them. 243 real-time sessions on the final build, v2 and Whisper:

| Case | Speech found, text kept | First words after speech starts (median, slowest) | Word errors, read aloud: v2 / Whisper |
|---|---|---:|---:|
| 10–30 dB quieter, over -62 dBFS | all 73 | 0.57–0.70 s, 0.84 s | 1.7–3.8 % / 14.3 % at -30 dB |
| Edge: loudest -45 dBFS, 10 dB over the room | all 40 | 0.65–0.96 s, 1.94 s | 26 % / 70 % |
| Pink noise at -45 / -40 / -35, babble at -40 | all 46 | 0.48–0.72 s, 0.72 s | 1.7–4.6 % / 9–11 % |
| "OK", "Yes" at -40 or -45 dBFS | all 72 | 0.57–0.68 s, 0.86 s | – |

- **At 20 dB** (simulated with the same rule), the detector missed every edge case and every "OK"/"Yes" at -45 dBFS, even 17 dB over a -62 room; at -30 dB it missed 24 of the 28 real starts and all five short clips. At 10 dB it still missed most edge cases, at 8 dB two; at 6 dB none, with 0.8 dB to spare on the weakest ("OK" at -45 over -55, no room before it).
- **Word errors** grow on the quietest and noisiest cases because the models hear less, not because anything was dropped: the short clips kept their clean word error rate, except Whisper at -25 dB and quieter, and on the edge copies.
- **Loud rooms** count as speech from the start once the room passes -35 dBFS, so there the preview may run on the noise before you speak (v2 showed a word on the -35 dBFS noise before the read-aloud clip's first word). That errs the safe way.
- **Energy** on the ramble didn't move: detection only decides when the first pass runs. Two macmon runs of the final build took 33.7 and 44.6 J a minute (mean 39.2; the CPU rail's noise again), with the GPU at 0.51–0.52 W, the same 185 passes and the same text. Speech is now found at 1.50 s instead of 1.53 s, and the first words came at 2.14 s.

```sh
B=target/release/easyvox-bench      # and a build of 6e59ccc for "before"
# First words and release latency.
$B session --model parakeet-tdt-0.6b-v2-q5_k_m-gguf --wav testdata/punct_samantha.wav --quiet --out lat.json
# A first-pass delay to try: --set preview_first_ms=400; the detection off: --set speech_min_ms=0.
# Silence: speech_onset_secs is null, partials.passes 0 and final_decode_ms about 0 when nothing was decoded.
$B session --model whisper-base.en-q8_0-gguf --wav testdata/silence_2s.wav --out silence.json
```

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

**Tuning.** The pipeline's numbers (preview interval and window, chunk length, joins, the speech gate, pauses) are the fields of `Tuning` in `crates/easyvox-core/src/tuning.rs`, each with its default, unit, range and trade-off. `--set key=value` changes any of them for one run and can be repeated, e.g. `--set preview_interval_ms=800 --set chunk_ms=15000`. An unknown key or an out-of-range value is an error. The older `--interval MS`, `--window SECS` and `--chunk SECS` still work, as aliases of `preview_interval_ms`, `preview_window_ms` and `chunk_ms`, and `--set` wins over them. `$B tuning` lists every key with its default, range and docs, and every result records the values it ran with under `tuning`.

Where the table's numbers come from:

- `bench`: `wer_total`, `empty_speech_decodes` and `failed_speech_decodes`, `rtf_speech_clips`, `mem.after_load.footprint_mb` and `mem.after_decodes.peak_footprint_mb`. The silence clip's `hypothesis` is under `clips`. Progress lines on stderr flag `EMPTY`, `FAILED` and `UNSTABLE` (runs that disagree).
- `sessions`, the `partials_on` runs: `partials.first_partial_ms`, `finalize_ms`, `cpu.cpu_pct_of_one_core` and `partials.revision_rate`. Don't use its memory numbers: it loads a second copy of the model for the `partials_off` runs.
- `session`: `finalize_ms`, `wer` (with `--ref`), `chunks` and `mem_end.peak_footprint_mb`; `partials.confirmed_rewrites` counts polished chunks swapped into the preview. CPU per minute came from `ps -o time= -p <pid>` every 2 s, GPU from `ioreg -r -d 1 -c IOAccelerator` ("Device Utilization %") every second, and the preview's lag from the partial lines on stderr, matched against the transcript's timeline.

To re-run the v2 Q8_0 comparison, add its catalog entry back as `BenchOnly`: file `parakeet-tdt-0.6b-v2-Q8_0.gguf` from `handy-computer/parakeet-tdt-0.6b-v2-gguf` at commit `382796e405a9fe37145938dccbabfb25e93e4286`, sha256 `f0d0e99cebb6d3b83f1f7069b82b5d3c2e39a54545b0da039cb4bafd9c4e5caa`, 729,574,912 bytes.
