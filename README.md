# EasyVox

A free, open source voice-to-text app for macOS.

Hold a shortcut and speak. EasyVox shows the words as it hears them, and when you let go it pastes the text where your cursor is. The speech model runs on your Mac, so no audio or text leaves the machine.

Website: [easyvox.dev](https://easyvox.dev)

## Status

🚧 Early development, not usable yet. There's nothing to install.

- The speech core (`crates/easyvox-core`) is in place and tested.
- The app shell (`crates/easyvox-app`, built with [GPUI Kit](https://github.com/longbridge/gpui-kit)) is ported from the prototype: menu bar icon, ⌥Space push-to-talk, the live-text overlay, Settings and History. Onboarding and model downloads inside the app come next.

The plan and the decisions behind it are in [DESIGN.md](DESIGN.md).

## Build from source

You need an Apple silicon Mac running macOS 26 or later. That's also what EasyVox needs to run.

1. **Xcode** or the Command Line Tools (`xcode-select --install`). Apple's separate Metal toolchain isn't needed: GPUI and the speech runtime compile their Metal shaders when the app runs, not at build time. The cost moves to the first launch: the first model load takes about 14 s, once.
2. **cmake**, which builds the speech runtime: `brew install cmake`.
3. **Rust** through [rustup](https://rustup.rs). `rust-toolchain.toml` pins the version, and rustup installs it on the first build.
4. **The speech model** (~541 MB). The finished app will download it during onboarding; for development, fetch it yourself:

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
   ```

6. **Make the app:**

   ```sh
   scripts/bundle.sh             # shipping build (thin LTO, about 1.5 minutes from clean)
   scripts/bundle.sh --release   # quicker, for day-to-day work
   open dist/EasyVox.app
   ```

   It writes `dist/EasyVox.app` and signs it ad hoc, which needs no certificate. macOS ties the Microphone and Accessibility permissions to the signature, so you may have to grant them again after each rebuild.

   Other arguments go to `cargo build`. `scripts/bundle.sh --features self-test` adds a self-check for a Mac nobody is watching: `open -W dist/EasyVox.app --args --self-test` replays a test clip through one dictation (no mic, no paste), writes a JSON report to `$TMPDIR/easyvox-self-test.json` (or `--out <file>`) and quits.

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs rustfmt, clippy, the build and the tests on macOS 26 for pushes to `main` and for pull requests.

### Developer CLI

`easyvox-bench` measures speed, memory and word errors on the test clips in `testdata/`. It isn't part of the default build:

```sh
cargo run -p easyvox-bench --release -- help
cargo run -p easyvox-bench --release -- session --model parakeet-unified-en-0.6b-q5_k_m-gguf --wav testdata/punct_samantha.wav --final-redecode
```

## License

[MIT](LICENSE)
