<h1>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="design/assets/mark/png/easyvox-lockup-color-on-dark.png">
    <img src="design/assets/mark/png/easyvox-lockup-color-on-light.png" alt="EasyVox" width="280" height="50">
  </picture>
</h1>

Push-to-talk dictation for macOS: hold a shortcut and speak, and when you let go, EasyVox pastes the text at your cursor. The speech model runs on your Mac, so nothing you say leaves it.

[easyvox.dev](https://easyvox.dev) · macOS 26 or later, Apple silicon · English

## Install

A signed download and a Homebrew cask come with the first release, on the [releases page](https://github.com/brandonbews/easyvox/releases). Until then, build it from source.

## Build from source

You need Xcode or its Command Line Tools, cmake and Rust through [rustup](https://rustup.rs).
`scripts/fetch-model.sh` fetches the default speech model, `cargo build` and `cargo test` build and test, and `scripts/bundle.sh` makes `dist/EasyVox.app`.
[`docs/building.md`](docs/building.md) has every step, the end-to-end tests and the self-test.

## Settings

EasyVox reads `~/.config/easyvox/config.toml` and applies edits when you save; the Settings window writes the same file.
[Settings](docs/settings.md) covers the common keys, and [the reference](docs/settings-reference.md) lists every one.

## Benchmarks

[`docs/model-bench.md`](docs/model-bench.md) has the measurements behind the model list and the defaults.

## License

[MIT](LICENSE). Third-party notices: [`packaging/THIRD_PARTY_NOTICES.html`](packaging/THIRD_PARTY_NOTICES.html). The speech models keep their own licenses; Settings › About credits each one.
