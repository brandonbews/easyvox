# Long-dictation clips

Long dictations are normal use: people hold the key for minutes. The clips in `testdata/` are 30 s or shorter, so these five clips of 1 to 9 minutes test long dictation: how soon the text is ready after release, peak memory, whether text is dropped or doubled where chunks join, and whether the live preview keeps up. The results are in the model bench's [Long dictation](../../docs/model-bench.md#long-dictation) section.

They're macOS text-to-speech (`say`) reading `paragraphs.txt`: 22 paragraphs of fictional notes, emails and lists, written for this project, with numbers, times, money and punctuation. TTS speech is far cleaner than real dictation, and its pauses are pure silence, so treat the numbers as a best case.

| Clip | Voice | Length | Words | Content |
|---|---|---:|---:|---|
| `lf_1m` | Samantha | 74 s | 250 | paragraphs 1–3, 0.5 s apart |
| `lf_3m` | Samantha | 195 s | 656 | paragraphs 1–8 |
| `lf_5m` | Samantha | 314 s | 1,036 | paragraphs 1–13 |
| `lf_10m` | Samantha | 530 s | 1,728 | all 22 paragraphs (8.8 minutes) |
| `lf_3m_pauses` | Daniel | 207 s | 571 | paragraphs 1–7, with 2.5–8 s thinking pauses between and inside paragraphs |

The Samantha clips are nested, so a paragraph can be compared across lengths. Lengths are from the October 2026 run and may shift a little with other macOS voice versions. The words don't change.

## Generating them

```sh
testdata/longform/generate.py
```

This takes about 25 s and writes 62 MB to `testdata/longform/clips/`, which is gitignored:

- `<id>.wav`: the clip, 16 kHz mono s16le like the other test clips.
- `<id>.txt`: its reference transcript.
- `manifest.json`: each clip's text, plus every paragraph's start and end time in the WAV, for finding where errors happen or how far the preview lags.
- `para/`: each paragraph spoken on its own. They're cached, and a paragraph is spoken again only when its text changes. Delete `clips/` to start from scratch.

Options: `--out DIR` writes the clips elsewhere, and `--text-only` writes only the references without running `say`.

The reference text depends only on `paragraphs.txt` and the clip lists at the top of `generate.py`, never on the audio. `manifest.tsv` (id, voice, category, text, as in `testdata/manifest.tsv`) is committed and rewritten on every run. If you edit `paragraphs.txt`, run `generate.py --text-only` and commit `manifest.tsv` with it.

## Running the bench on one

`session` replays the WAV in real time with live partials. With `--ref`, it also reports the word error rate against the reference. `/usr/bin/time -l` adds the peak memory footprint.

```sh
cargo build -p easyvox-bench --release
B=target/release/easyvox-bench
uptime   # timings drift when the machine is busy: aim for a load average around 2 or less
/usr/bin/time -l $B session --model parakeet-tdt-0.6b-v2-q5_k_m-gguf \
  --wav testdata/longform/clips/lf_3m.wav --ref testdata/longform/clips/lf_3m.txt --out lf_3m.json
```

- A replay takes as long as the clip: about 9 minutes for `lf_10m`.
- The numbers to read are `finalize_ms` (text ready after release), `wer`, `chunks.polished` (chunks polished while talking) and `mem_end.peak_footprint_mb`.
- `--chunk 0` decodes the whole recording in one pass on release, the old behavior, for comparison.
- `--quiet` hides the partial lines on stderr. Keep them if you want to measure preview lag against `clips/manifest.json`.
- `$B help` lists the other options.
- For a short-clip baseline, run the same command on `clips/para/Samantha_03.wav` with `--ref clips/para/Samantha_03.txt`.
