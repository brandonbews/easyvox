# Build from source

Requires an Apple silicon Mac on macOS 26 or later. Run the commands from the repository root.

## Tools

- **Xcode** or the Command Line Tools: `xcode-select --install`. The separate Metal toolchain isn't needed: shaders compile at run time, so the first model load takes about 14 s, once.
- **cmake**, for the speech runtime: `brew install cmake`.
- **Rust** through [rustup](https://rustup.rs). `rust-toolchain.toml` pins the version.

## The speech model

The app downloads it during setup. For development, fetch it yourself; this works before the first build:

```sh
scripts/fetch-model.sh              # the default, Parakeet TDT 0.6B v2 (539 MB)
scripts/fetch-model.sh --list       # every model, its size, and whether the app offers it
scripts/fetch-model.sh <model-id>   # another, e.g. whisper-base.en-q8_0-gguf (85 MB)
```

Models go in `~/Library/Application Support/EasyVox/models/` and are checked against their sha256. `EASYVOX_MODELS_DIR` (models only) or `EASYVOX_DATA_DIR` (all of EasyVox's data) selects another folder, for the script and the app.

## Build and test

```sh
cargo build
cargo test                                          # includes the downloader, against a local server
cargo test -p easyvox-core --release -- --ignored   # end-to-end engine tests; need the model

# Live checks against Hugging Face (HEAD of every offered URL, one 85 MB download):
EASYVOX_LIVE_DOWNLOADS=1 cargo test -p easyvox-core --test download_live -- --ignored
```

CI ([`.github/workflows/ci.yml`](../.github/workflows/ci.yml)) runs rustfmt, clippy, the build and the tests on macOS 26 for pushes to `main` and pull requests.

## The app

```sh
scripts/bundle.sh             # optimized build (thin LTO), about 1.5 min from clean
scripts/bundle.sh --release   # faster, for day-to-day work
open dist/EasyVox.app
```

This writes `dist/EasyVox.app`, signed ad hoc with the hardened runtime, as a release is. `--dev` makes a debug build without the hardened runtime, so a debugger can attach. macOS ties permissions to the signature, so after a rebuild it may ask for the Microphone and Accessibility again. Other arguments are passed to `cargo build`.

### Self-test

`scripts/bundle.sh --features self-test` adds an unattended check:

```sh
open -W dist/EasyVox.app --args --self-test --out report.json
```

It replays a test clip through two dictations, applies a settings edit live, downloads, cancels and switches models, and draws onboarding and every Settings page in each state. Then it writes a JSON report (`$TMPDIR/easyvox-self-test.json` by default) and quits. It works in temporary folders against a server on 127.0.0.1, only reads the models folder, and never changes a permission or shows a macOS prompt.

## Bench

`easyvox-bench` measures speed, memory and word errors on the clips in `testdata/`. It isn't part of the default build:

```sh
cargo run -p easyvox-bench --release -- help
cargo run -p easyvox-bench --release -- session --model parakeet-tdt-0.6b-v2-q5_k_m-gguf --wav testdata/punct_samantha.wav
```
