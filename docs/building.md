# Build from source

You need an Apple silicon Mac running macOS 26 or later. That's also what EasyVox needs to run. Run the commands below from the root of a clone of the repository.

1. **Xcode** or the Command Line Tools (`xcode-select --install`). Apple's separate Metal toolchain isn't needed: GPUI and the speech runtime compile their Metal shaders when the app runs, not at build time. The cost moves to the first launch: the first model load takes about 14 s, once.
2. **cmake**, which builds the speech runtime: `brew install cmake`.
3. **Rust** through [rustup](https://rustup.rs). `rust-toolchain.toml` pins the version, and rustup installs it on the first build.
4. **The speech model**, Parakeet TDT 0.6B v2 (~539 MB). The app downloads it during onboarding; for development, fetch it yourself (the script works on a fresh clone, before the first build):

   ```sh
   scripts/fetch-model.sh              # the default model
   scripts/fetch-model.sh --list       # every model, its size, and whether the app offers it
   scripts/fetch-model.sh <model-id>   # another one, e.g. whisper-base.en-q8_0-gguf (85 MB)
   ```

   It downloads into the folder the app loads models from, `~/Library/Application Support/EasyVox/models/`, and checks the sha256. A model that's already there and checks out isn't downloaded again, and a failed or interrupted download leaves nothing behind. Set `EASYVOX_MODELS_DIR` (just the models) or `EASYVOX_DATA_DIR` (all of EasyVox's data) to use another folder; the app reads the same variables.

5. **Build and test:**

   ```sh
   cargo build
   cargo test

   # End-to-end engine tests. They need the model from step 4.
   cargo test -p easyvox-core --release -- --ignored

   # Live download checks against Hugging Face, when the catalog changes: a HEAD of every
   # offered model's URL, and a real 85 MB download into a temporary folder. They need the
   # internet, so they also need this variable; without it they do nothing.
   EASYVOX_LIVE_DOWNLOADS=1 cargo test -p easyvox-core --test download_live -- --ignored
   ```

   `cargo test` covers the in-app downloader offline, against a small HTTP server on 127.0.0.1.

6. **Make the app:**

   ```sh
   scripts/bundle.sh             # shipping build (thin LTO, about 1.5 minutes from clean)
   scripts/bundle.sh --release   # quicker, for day-to-day work
   open dist/EasyVox.app
   ```

   It writes `dist/EasyVox.app` and signs it ad hoc, which needs no certificate. macOS ties the Microphone and Accessibility permissions to the signature, so you may have to grant them again after each rebuild; onboarding opens to ask.

   Other arguments go to `cargo build`. `scripts/bundle.sh --features self-test` adds a self-check for a Mac nobody is watching: `open -W dist/EasyVox.app --args --self-test` replays a test clip through two dictations, the second right after the model was unloaded (no mic, no paste), checks that an edit to the settings file applies live (on a temporary settings file, unless `EASYVOX_CONFIG` names one), draws the Models page in every state and opens About from the menu, downloads a file from a server of its own on 127.0.0.1 into a temporary folder (never the internet), cancels and deletes it there, switches to a second downloaded model during a dictation and back, checks that picking a model that isn't downloaded shows it as missing, checks onboarding (that it opened at launch exactly when a permission or the model is missing, opens or stays shut for injected answers and draws every step, and downloads the chosen model from its own server into a temporary folder, through a cut connection it resumes from, then loads it), writes a JSON report to `$TMPDIR/easyvox-self-test.json` (or `--out <file>`) and quits. It never writes to or deletes from the models folder (onboarding's check only reads the chosen model from it, to serve it), never changes a permission and never brings up a macOS prompt; point `EASYVOX_MODELS_DIR` at a copy to keep it apart from yours.

CI ([`.github/workflows/ci.yml`](../.github/workflows/ci.yml)) runs rustfmt, clippy, the build and the tests on macOS 26 for pushes to `main` and for pull requests.

## Developer CLI

`easyvox-bench` measures speed, memory and word errors on the test clips in `testdata/`. It isn't part of the default build:

```sh
cargo run -p easyvox-bench --release -- help
cargo run -p easyvox-bench --release -- session --model parakeet-tdt-0.6b-v2-q5_k_m-gguf --wav testdata/punct_samantha.wav
```
