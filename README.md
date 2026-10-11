<h1>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="design/assets/mark/png/easyvox-lockup-color-on-dark.png">
    <img src="design/assets/mark/png/easyvox-lockup-color-on-light.png" alt="EasyVox" width="280" height="50">
  </picture>
</h1>

A free, open source voice-to-text app for macOS.

Hold a shortcut and speak. EasyVox shows the words as it hears them, and when you let go it pastes the text where your cursor is. The speech model runs on your Mac, so no audio or text leaves the machine.

Website: [easyvox.dev](https://easyvox.dev)

## Status

🚧 Early development, not usable yet. There's nothing to install.

- The speech core (`crates/easyvox-core`) is in place and tested.
- The app shell (`crates/easyvox-app`, built with [GPUI Kit](https://github.com/longbridge/gpui-kit)) is ported from the prototype: menu bar icon, ⌥Space push-to-talk, the live-text overlay, Settings and History.
- Settings → Models downloads, switches and deletes the offered models: Parakeet TDT v2 (the default) and Whisper base.en (the small one). A download is checked against its sha256 before it's used, goes through macOS's own networking (so system proxy settings apply), and is saved with a LICENSE.txt that credits the model.
- On the first launch, onboarding asks for the Microphone and Accessibility, downloads the default model and loads it once, so the first dictation doesn't wait for the one-time shader compile. It opens again at launch whenever one of those is missing; Settings → Permissions opens it any time.

The plan and the decisions behind it are in [DESIGN.md](DESIGN.md).

## Build from source

You need an Apple silicon Mac on macOS 26 or later, with Xcode (or its Command Line Tools), cmake and Rust through rustup.
`scripts/fetch-model.sh` fetches the speech model, `cargo build` and `cargo test` build and test, and `scripts/bundle.sh` makes `dist/EasyVox.app`.

Every step, the end-to-end tests, the self-test, CI and the developer CLI are in [`docs/building.md`](docs/building.md).

## Settings

EasyVox reads its settings from `~/.config/easyvox/config.toml` (or `$XDG_CONFIG_HOME/easyvox/config.toml`; `EASYVOX_CONFIG` points at another file) and applies edits as soon as you save. The Settings window writes the same file. The file only needs the keys you change; [`docs/settings.md`](docs/settings.md) lists every key with its default, its unit and what it trades off, including the advanced ones for the live preview, the chunks and the pauses. `dist/EasyVox.app/Contents/MacOS/easyvox --print-default-config` prints the same list.

## License

[MIT](LICENSE). The app's third-party notices are in [`packaging/THIRD_PARTY_NOTICES.html`](packaging/THIRD_PARTY_NOTICES.html), generated from `Cargo.lock` by `scripts/notices.sh` (needs `cargo install --locked cargo-about --features cli`); run it after a dependency update, or `cargo test` says so. The speech models keep their own licenses: Settings → About credits each one.
