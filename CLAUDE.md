# EasyVox

## Private notes

- `internal/` is a gitignored clone of the private `brandonbews/easyvox-internal` repo (`git clone https://github.com/brandonbews/easyvox-internal.git internal`); its README and `scripts/install-hooks.sh` cover the rest of the setup. It may be missing, and that's fine.

## Building and testing

- `docs/building.md` has every step: the tools, the speech model, the test clips, the build, the tests, the self-test and `scripts/bundle.sh`.
- Before a commit, run what CI runs: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` (and again with `-p easyvox-app --features self-test`), and `cargo test --workspace --locked`.
- The site in `site/` has its own commands and rules: `site/AGENTS.md`.

## Writing

- Prose, descriptions and comments: tight, clear technical language. No marketing tone, no filler, nothing that reads as generated. American spelling.
