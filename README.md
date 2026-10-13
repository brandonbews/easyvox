<h1>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="design/assets/mark/png/easyvox-lockup-color-on-dark.png">
    <img src="design/assets/mark/png/easyvox-lockup-color-on-light.png" alt="EasyVox" width="280" height="50">
  </picture>
</h1>

Fast, local-only speech-to-text for macOS. Open source.

EasyVox goes online only when you ask: to download a speech model, and to check easyvox.dev for a newer version.

[easyvox.dev](https://easyvox.dev) · macOS 26 or later, Apple silicon · English

## Install

Download the signed, notarized DMG from the [latest release](https://github.com/brandonbews/easyvox/releases/latest), or install EasyVox with [Homebrew](https://brew.sh):

```sh
brew install --cask brandonbews/tap/easyvox
```

EasyVox needs macOS 26 or later on Apple silicon. Or [build it from source](#build-from-source).

## Build from source

You need Xcode or its Command Line Tools, cmake and Rust through [rustup](https://rustup.rs).
`scripts/fetch-model.sh` fetches the default speech model, `cargo build` and `cargo test` build and test, and `scripts/bundle.sh` makes `dist/EasyVox.app`.
[`docs/building.md`](docs/building.md) has every step, the end-to-end tests and the self-test.

## Settings

EasyVox reads `~/.config/easyvox/config.toml` and applies edits when you save; the Settings window writes the same file.
[Settings](docs/settings.md) covers the common keys, and [the reference](docs/settings-reference.md) lists every one.

## Benchmarks

[`docs/model-bench.md`](docs/model-bench.md) has the measurements behind the model list and the defaults.

## Private notes

The maintainer keeps the design decisions, the backlog, release steps and the rules agents follow for this project in a private repo, `brandonbews/easyvox-internal`, cloned into this checkout as `internal/`. This repo ignores `/internal` and `/CLAUDE.local.md`, and everything builds and tests the same without them. To set up a computer with it:

1. Clone this repo, then the private one inside it: `git clone https://github.com/brandonbews/easyvox-internal.git internal`.
2. Run `sh internal/scripts/install-hooks.sh`, and again after moving either checkout. It's safe to rerun, and it refuses to replace a file or a hook it didn't write.
   - It links `CLAUDE.local.md` here to `internal/agent-rules.md`, so Claude Code loads those rules in this checkout and in worktrees under `.claude/worktrees/`.
   - It installs a `post-checkout` hook that links `internal` into every new worktree (and `CLAUDE.local.md`, for a worktree outside this checkout).
3. Copy `internal/recordings/` from the old computer by hand: the real-voice clips for the local benchmarks. No repo carries them.

## License

[MIT](LICENSE). Third-party notices: [`packaging/THIRD_PARTY_NOTICES.html`](packaging/THIRD_PARTY_NOTICES.html). The speech models keep their own licenses; Settings › About credits each one.
