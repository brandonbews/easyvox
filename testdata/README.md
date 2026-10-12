# Test audio

- **The spoken clips** (`cmd_*`, `num_*`, `names_*`, `punct_*` and `long_*.wav`) are macOS text-to-speech. Apple's license doesn't allow redistributing its System Voices, so they aren't in the repo: `generate.sh` makes them on your Mac with `say`. If a voice is missing, it says which one to add. Their text, in `manifest.tsv`, is invented.
- **`silence_2s.wav`**, 2 s of digital silence, is generated, not spoken, so it's committed. `generate.sh` writes it too.
- **`longform/`** holds the long-dictation clips, made the same way from `longform/paragraphs.txt`, which is fictional. `longform/generate.py` makes them; see [`longform/README.md`](longform/README.md).

Without the spoken clips, `cargo test` skips the two tests that read them and says so. The model tests (`cargo test -- --ignored`), the bench and the app's self-test fail and ask for `generate.sh`.
