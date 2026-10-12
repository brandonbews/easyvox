# Bench data, 9–11 October 2026

These files hold the per-run numbers behind [`docs/model-bench.md`](../model-bench.md). That file has the summary tables and the methods. This folder has the runs those tables summarize, one row per run or per sample, so charts can be drawn from them.

- **When:** every run was made on 9, 10 or 11 October 2026. `studies.csv` gives the date of each study.
- **Machine:** one M4 Mac mini (16 GB), on mains power, macOS 26.6.2, transcribe-cpp 0.3.1, 4 threads.
- **Code:** each study ran a known commit, or a build on its way to one. See [Studies and builds](#studies-and-builds).
- **Extracted:** on 10 October 2026, by [`scripts/bench/extract.py`](../../scripts/bench/extract.py), from the raw runs in two session scratch folders. The 11 October rows (study `candidates`) were added on 11 October from a third.

## No transcript text

There is no transcript text in this folder. That means no final text, no preview text, no words, no reference text and no word lists.

Two of the recordings are the author's own voice (`structured.wav`, read aloud, and `ramble.wav`, five minutes of talking). They're private and not in the repo. The raw run files hold the text the models decoded from them, so none of that text may be published. The same rule covers the text-to-speech clips in `testdata/`, so there is only one rule.

Every cell is a number or a label from a fixed list: a model id, a clip file name, a run id, a study or build name, a setting and its value, or a phase. The extractor checks every cell before it writes a file. The only text it reads is the preview's words and the words of a one-pass decode of the same recording, to time the preview (the lag tables). It keeps none of it.

## How the files fit together

- `run_id` is the key. It is `study/label`, where the label is the raw run's file name.
- `runs.csv` has one row per dictation session. `energy.csv`, `power.csv`, `memory.csv`, `lag_*.csv`, `passes.csv`, `cuts.csv`, `starts.csv` and `stress.csv` add more about some of those runs, by `run_id`.
- `bench_runs.csv` and `clips.csv` hold the `bench` runs (every test clip decoded on its own). `app_runs.csv` holds runs of the app itself.
- `studies.csv` names the `docs/model-bench.md` section each study backs, and its date.

Units are in the column names: `_ms` milliseconds, `_s` seconds, `_pct` percent, `_w` watts, `_mw` milliwatts, `_mb` megabytes (10^6 bytes), `_dbfs` decibels relative to full scale, `_db` decibels. A CPU percentage is a share of one core. An empty cell means not measured.

## Files

### `studies.csv`

One row per study: `study`, `section` (the heading in `docs/model-bench.md`) and `date`.

### `runs.csv` (1,485 rows)

One row per `easyvox-bench session` run, or per clip of a `sessions` run: a WAV replayed in real time through the app's code path. It backs every session table in `docs/model-bench.md`, from "Results" to "Every candidate, one pipeline".

- **Who and what:** `study`, `build`, `model`, `clip`, and `source`:
  - `tts`: a text-to-speech clip from `testdata/`;
  - `silence`: `silence_2s.wav`;
  - `private`: one of the two private recordings;
  - `private-cut`: a piece cut from them (the 28 dictation starts `r…`/`s…`, the room-noise clips);
  - `synthetic-from-private` and `synthetic`: the stress copies (see `stress.csv`).
- **`settings`:** the settings a run changed, as `name=value;…`, where the run list recorded them. The `preview_*`, `chunk_ms`, `speech_*`, `agreement_reach_words` and `meter_interval_ms` columns give the values each run used, from the run's own record. Runs made before that record existed (before da36537, late on 9 October) give only `preview_interval_ms` (`sessions` runs) or `chunk_ms`, where they logged it.
- **Accuracy:** `wer_pct`, `wer_errors`, `ref_words` (only with a reference transcript), and `final_words`, the number of words in the final text.
- **Latency:**
  - `speech_onset_s`: where the speech detector found speech;
  - `first_words_ms`: first preview words, from the start of the replay;
  - `finalize_ms`: text ready after release, made of `final_wait_ms` (waiting for a preview pass still running) and `final_decode_ms`;
  - `model_wait_ms`: waiting for a model that was unloaded.
- **Work done:**
  - `cpu_pct` (whole session), `cpu_s`, `wall_s`;
  - `gpu_util_pct`: macOS's whole-GPU "Device Utilization %", averaged over the session;
  - `chunks` and their decode times;
  - `passes`, `skipped`, `pass_ms_mean`, `pass_ms_max`, `window_s_mean` (seconds of audio per preview pass).
- **The preview:**
  - `preview_events`: updates shown;
  - `revision_rate`: the share of updates that changed words already on screen;
  - `revisions`, `words_revised`, `updates_per_s`, `median_gap_ms`;
  - `confirmed_rewrites`: polished chunks swapped in;
  - `confirmed_vs_final_errors`, `confirmed_coverage`.
- **Conditions:** `metal_residency` and `unloaded_before` (0 or 1), `idle_before_s`, `loadavg_before` and `loadavg_after`. Some runs also have `other_cpu_pct`, other processes' CPU, and `build_overlap_s`, the seconds a build was running at the same time.

`gpu_util_pct` leaves out the model load. The energy runs use the samples inside the session. The 9 October `idle-*` runs drop the first 2 samples, as their scripts did. The others drop the first 3. `onepipe` drops none.

### `bench_runs.csv` (35 rows) and `clips.csv` (1,414 rows)

`easyvox-bench bench` runs: the 14 test clips decoded one at a time, 3 times each (once for `bench_fast`). They back "Results" (word error rate, empty or failed decodes, speed and memory), the `bench` check in "One pipeline" and the text-to-speech columns of "Parakeet TDT v3 against v2" and "Every candidate, one pipeline".

- `bench_runs.csv`: one row per run. `wer_pct` over all clips, `errors`, `ref_words`, `empty_speech_decodes`, `failed_speech_decodes`, `rtf_speech_clips` (decode time over audio length), `load_ms_first` and `load_ms_warm` (model load), `first_decode_ms`.
- `clips.csv`: one row per clip and repeat. `category`, `audio_s`, `ref_words`, `errors`, `wer_pct`, `rtf`, `decode_ms` (that repeat's decode), `hyp_words` (the number of words decoded) and `distinct_hyps` (1 if every repeat wrote the same text). The error counts are per clip, the same for each repeat.

The "Results" table pools `bench1-*` and `bench2-*`, the two rounds of 3 repeats.

### `memory.csv` (2,932 rows)

The process's memory footprint (`footprint_mb`) and its peak so far (`peak_footprint_mb`), by `phase`:

- for sessions: `loaded` (after the model load and a warm-up decode) and `end`;
- for `bench` runs: `start`, `loaded`, `after_decodes`, `idle_2s` and `unloaded`;
- `time_peak`: the peak footprint `/usr/bin/time -l` reported for the 9 October long-clip runs.

It backs the memory columns of every table. `sessions` runs (several clips in one process) have no rows: they load a second copy of the model, as `docs/model-bench.md` warns.

### `energy.csv` (130 rows)

One row per energy run. That's a `macmon` trace around the run, from 60 s of idle machine before it to 60 s after. It backs "Lightness", the energy rows of "First words and silence" and "Quiet mics and loud rooms", "Parakeet TDT v3 against v2", "Level meter", "Whisper's preview" and "Every candidate, one pipeline". The candidates' read-aloud runs (`candidates/s…`) had only 20 s of idle machine either side: their GPU power holds, but their energy totals are rough, and that section quotes energy from the ramble only.

- `j_per_min`: the headline figure, energy per minute of audio. It counts the SoC (CPU, GPU, Neural Engine and DRAM) on top of the idle machine, over the session (from the replay's start to the final text). GPU, Neural Engine and DRAM subtract their idle mean. The CPU rail, which jumps with other processes, subtracts the median of 5 s blocks.
- `w_added` (all rails) and `w_cpu`, `w_gpu`, `w_ane`, `w_dram`: the added power in watts.
- `j_per_min_plain` subtracts the idle mean from every rail. `j_per_min_sys` uses the whole-machine estimate (SMC) instead.
- `w_idle_soc`: the idle power. `gpu_active_pct`: macmon's GPU active ratio over the session. `temp_start_c`, `fan_max_rpm`, `session_s`, `audio_s`.

For `meter/*` (app runs), the session is the self-test's own start and end, and idle is the rest of the trace, as the meter's analysis did.

### `power.csv` (34,071 rows)

The 130 energy traces, as mean power in 1 s bins: `cpu_mw`, `gpu_mw`, `dram_mw` and `gpu_active_pct`. `t_s` counts seconds from the session's start, from 15 s before it to 15 s after its end. macmon samples about every 0.55 s. The Neural Engine read 0 W in every sample, so it has no column. These are the traces for a before and after chart of "Lightness".

### `traces.csv` (14,083 rows)

Per-second and per-2-second samples taken during the 9 October runs ("One pipeline", "Long dictation", "Idle and energy", "Real voice", "Chunk joins"):

- `gpu_util_pct`: whole-GPU "Device Utilization %" (`ioreg`), every second;
- `cpu_pct` and `rss_mb`: the bench's CPU over the last 2 s and its resident memory (`ps`), every 2 s;
- `other_cpu_pct`: every other process's CPU, every second.

`t_s` counts from the sampler's start, which is the bench's launch, so the model load comes first. A `sessions` command decodes several clips in a row, and its trace has the file's id (`onepipe/sessions_v2`), not a clip's.

### `cpu_minutes.csv` (221 rows)

CPU while talking, minute by minute (`cpu_pct` per `minute`, from 1). It backs the "CPU while talking" columns of "Long dictation" and "Whisper's preview".

- `ps`: the 2 s `ps` samples averaged per minute of the sampler, as the "Long dictation" analysis did. The first minute includes the model load.
- `cputime-stamps`: the process's CPU time logged with each preview pass, interpolated to whole minutes of the session ("Whisper's preview").

### `passes.csv` (3,975 rows)

Every preview pass and chunk decode of the 23 "Whisper's preview" runs, from a scratch copy of the bench that logged them:

- `phase`: `preview` or `chunk`;
- `t_s`: when it ended, in the session;
- `window_s`: seconds of audio decoded;
- `decode_ms`;
- `words`: the number of words the pass wrote;
- `new_confirmed`: words it confirmed;
- `cpu_s`: the process's CPU time so far.

### `cuts.csv` (370 rows)

Every chunk cut in the runs whose bench logged them ("Real voice", "Chunk joins", "Parakeet TDT v3 against v2"):

- `cut_at_s` and the chunk's span, `span_start_s` to `span_end_s`;
- `pending_audio_s` and `wall_s` at the cut (`wall_s` is empty where the bench didn't log it);
- `words_added` and `decode_ms` for the chunk;
- the pause the cut used, `pause_start_s` to `pause_end_s`, and its length `pause_s`.

### `lag_quantiles.csv` (640 rows) and `lag_hist.csv` (5,094 rows)

How far the preview trails the voice, per word, for 102 chunked sessions on `ramble.wav`, `lf_3m.wav` and `lf_10m.wav`. They back "Preview behind the voice" in "Real voice", "Lightness", "First words and silence", "Parakeet TDT v3 against v2", "Whisper's preview" and "Every candidate, one pipeline".

The method is the one in `docs/model-bench.md`. Each recording has a one-pass decode with word timings: the `timeline`. For each of its words, the lag is the time of the first preview update whose last three words match the timeline at that word, minus the time the word ended. The timelines:

- `ramble-v2-onepass`: Parakeet TDT v2's one-pass decode of the ramble from 9 October (`realvoice/r0-v2`). The `candidates` runs use it for every model, so a model that words things differently from v2 shows more lag than it has;
- `ramble-v3-onepass` and `ramble-v2-0910-onepass`: v3's and v2's own one-pass decodes from 10 October, for the v3 study;
- `lf_3m-v2-onepass` and `lf_10m-v2-onepass`: v2's one-pass decodes of the two long text-to-speech clips.

The two files:

- **`lag_quantiles.csv`:** per run, `minute` = `all` and then each minute of the recording. `words`, then the 5th to 95th percentiles in steps of 5 (`p05_s` … `p95_s`), `max_s` and `mean_s`. On the `all` row, `timeline_words` and `unmatched_updates` (updates the method couldn't place).
- **`lag_hist.csv`:** per run, the number of words in each 0.1 s bin (`lag_bin_s` is the bin's lower edge).

The method sometimes places a phrase early, so a few words get a large negative lag: 1.4 % of all words are below −1 s. The published medians and percentiles come from the same values. The 9 October "Long dictation" table measured lag another way (against the text-to-speech script's paragraph times), so these values for `onepipe/*` don't reproduce its "≤" figures.

### `starts.csv` (392 rows)

First words on the 28 dictation starts cut from the two private recordings, for "First words and silence". `start` is the start's name (its time in the recording: `r…` in the ramble, `s…` in the read-aloud clip). `onset_s` is where its speech starts, by the doc's rule: the first 120 ms above −40 dBFS. `first_after_onset_s` is `first_words_ms` minus that.

- `starts`: the published table, the build before against the final build (`new3`).
- `starts-20db`: the same with the first, 20 dB detector (`new2`).
- `firstpass-sweep-real`: `preview_first_ms` at 400 and 500 ms.

### `stress.csv` (243 rows)

The stress sessions of "Quiet mics and loud rooms", one per row. `case`:

- `quieter`: speech turned down by `atten_db` over pink noise at `noise_dbfs`;
- `edge`: the loudest 30 ms at `speech_peak_dbfs`, over a room at `room_dbfs`;
- `noisy`: the original speech under pink noise or babble (`noise`) at `noise_dbfs`;
- `soft`: "OK" or "Yes" with its loudest 30 ms at `speech_peak_dbfs`, over a room at `room_dbfs`, with `lead_s` of room before it;
- `noise`: room noise only.

Each row also has `source_clip`, the clip it was made from. `speech_found` and `text_kept` are 0 or 1. `wer_pct` is given for the read-aloud copies only. `onset_s` is where the speech starts in the clean source, by the same rule as `starts.csv`, and `first_after_onset_s` counts from it.

### `app_runs.csv` (19 rows)

Runs of the app itself (`EasyVox.app --self-test --replay`), for "Level meter" (`meter/*`, the 5-minute ramble) and "Overlay animation" (`overlay/*`, `lf_3m.wav`).

- `settings`: `meter_interval_ms`, `overlay_frame_ms`, `overlay_animations=0` (the four timings at 0) and `draw` (1 when each redraw was drawn).
- The app's own CPU: `cpu_pct`, `cpu_s`, `wall_s`.
- Counts: `levels_received`, `redraws_asked`, `renders` and `animation_frames`, plus the meter's, redraws' and draws' rates per second of audio.
- `drawing_pct` and `drawing_ms_per_render`: the forced draws' own time, from the builds that timed it.

The meter runs' energy is in `energy.csv` and `power.csv`. The overlay runs came in two rounds, `runs/` and `runs2/`. The published "Before" row matches both rounds pooled, and the other rows match `runs2/`.

### `idle.csv` (11 rows)

The idle app, measured from `proc_pid_rusage` over `window_s`: `wakeups_per_s`, `idle_exit_wakeups_per_s`, `cpu_pct`, `footprint_mb` and `resident_mb`. `state` is `loaded`, `loaded-1min`, `loaded-4.5min`, `unloaded` or `unloaded-after-edit`. It backs "The idle app" in "Idle and energy", and its paragraph on the settings file (`settings-idle`).

## Studies and builds

| Study | Section | Build labels and the code they ran |
|---|---|---|
| `results` | Results | `old-engine`: the engine before one pipeline, committed in 696b222 |
| `onepipe` | One pipeline; Long dictation | `onepipe`: 30a0c79 |
| `longform` | Long dictation, "one pass" | `old-engine`: the old engine, which decoded the whole recording on release |
| `longform-proto` | (prototype, not in a table) | `proto`: a first try at chunked final passes, before 30a0c79 |
| `idle-lat` | Dictation after idling | `d33ab36` |
| `idle-prev`, `idle-fw`, `idle-pn`, `idle-real`, `idle-tick` | The preview in pauses | `before`: 32b1af7, run with `GGML_METAL_NO_RESIDENCY=1` so only the speech gate differs. `after`: d33ab36 |
| `idle-app` | The idle app | the bundled app at `32b1af7` (also with `+no_residency`) and `d33ab36` |
| `realvoice` | Real voice | `1544b11`, plus log lines for cuts, pauses and final words |
| `joinfix` | Chunk joins | `before`: 9a2e921 with the same log lines. `after`: 39b51e0 |
| `tuning-check` | (a check, not in a table) | `before`: 39b51e0. `after`: da36537, which only gathered the knobs into `Tuning` |
| `lightness` | Lightness | `base`: da36537 (the "Before" rows). `new`: passes from the confirmed words, with intervals set per run. `final`: ffa49a5. `final-res`: ffa49a5 with ggml's residency sets kept |
| `settings-idle` | The idle app (the settings file) | `ffa49a5` before the settings file, `8f3f0b3` with it (`-warm`: relaunched) |
| `firstwords-*`, `firstpass-*`, `silence`, `starts-20db`, `starts`, `stress` | First words and silence; Quiet mics and loud rooms | `base`: 6e59ccc. `new`, `new2` and `new3`: the builds of f424519 in turn. `new` and `new2` have the first, 20 dB detector. `new3` has the final 6 dB detector, as committed |
| `v3` | Parakeet TDT v3 against v2 | `f424519` |
| `meter` | Level meter | `b9dd9fb` |
| `whisper` | Whisper's preview | `head`: cd78c42. `fix`: fc7c60a. `fix-cmp`: fc7c60a logging both agreement rules. All three add log lines for each pass |
| `overlay` | Overlay animation | `fc7c60a` (before) and `5bd95f9` |
| `candidates` | Every candidate, one pipeline | `93fc164+catalog`: 93fc164 with the nine bench-only catalog entries added, nothing else |

Run labels follow each study's own scheme:

- `-rN`: an energy run; `-sN`: a read-aloud run;
- `r0` and `s0`: one pass on release (`chunk_ms=0`);
- `B-` and `N-`: base and new builds;
- `a-` and `b-`: before and after;
- `candidates`: `s1-`…`s3-` the read-aloud rounds, `r1-` and `r2-` the ramble runs, `tts-` the clips, each followed by the model's alias (`extract.py`'s `MODELS`).

Every run is here, including a few that `docs/model-bench.md` left out. "The preview in pauses" dropped two read-aloud runs that overlapped a build on the same Mac: `idle-real/before3-structured` (`build_overlap_s` 26) and, judging by its 4.25 s release, `idle-real/after1-structured`. In the 5-minute ramble, Whisper's preview showed a word made up from room noise at 1.4 s. `first_words_ms` counts that word, but the published table gives 2.2 s, the first real word.

## Regenerating

The raw runs were in session scratch folders under `/private/tmp`. They held:

- 9 October: `mbench/`, `onepipe/`, `longform/` and `idle/`;
- 9 October evening to 10 October: `realvoice/`, `joinfix/`, `eq/`, `lightness/`, `idle/`, `engine2/`, `v3/`, `ab/`, `whisper/` and `overlay-anim/`;
- 11 October: `candidates/runs/`, in the second of those folders.

With the folders:

```sh
python3 -I scripts/bench/extract.py --s1 <9 Oct evening to 10 Oct folder> --s2 <9 Oct folder> \
    --s3 <9 Oct evening to 10 Oct folder>/candidates
```

It also reads the private `structured.wav` (from `--recordings`), if it's there, for where the speech starts in the read-aloud stress copies. The raw run files, and any audio cut from the private recordings, stay private.

The other scripts in `scripts/bench/` make and read new runs. All of them print numbers only.

- `run1.sh`: one energy run (macmon around a session).
- `rotate.sh`: a schedule of runs in rotated order.
- `quiet.sh`: waits until the Mac is quiet.
- `energy.py` and `hybrid.py`: energy per run. `hybrid.py` gives `j_per_min`.
- `summary.py`: means and confidence intervals per setting.
- `lag.py` (with `textutil.py`): the preview's lag.
- `agree.py`: how far two runs' final texts differ, without printing them.
- `scrub.py`: strips the text from result files.
- `wakes.py`: an idle process's wake-ups.
- `levels.py`, `cut.py` and `synth.py`: frame levels, and the stress clips.
