# EasyVox

## Private notes

`internal/` is a clone of the private repo `brandonbews/easyvox-internal`: the design decisions, the backlog, release steps, session hand-offs, and the rules agents follow when working for the maintainer. This repo ignores `/internal` and `/CLAUDE.local.md`. It may be missing, and that's fine: everything builds and tests without it.

This section is the canonical description of how the two repos fit together. To set up a computer:

1. Clone this repo, then the private one inside it: `git clone https://github.com/brandonbews/easyvox-internal.git internal`.
2. Run `sh internal/scripts/install-hooks.sh`, and again after moving either checkout. It's safe to rerun, and it refuses to replace a file or a hook it didn't write.
   - It links `CLAUDE.local.md` here to `internal/agent-rules.md`, so Claude Code loads the private rules in this checkout and in worktrees under `.claude/worktrees/`.
   - It installs a `post-checkout` hook that links `internal` into every new worktree (and `CLAUDE.local.md`, for a worktree outside this checkout).
3. Copy `internal/recordings/` from the old computer by hand: the real-voice clips for the local benchmarks. No repo carries them.

## Building and testing

- `docs/building.md` has every step: the tools, the speech model, the test clips, the build, the tests, the self-test and `scripts/bundle.sh`.
- Before a commit, run what CI runs: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` (and again with `-p easyvox-app --features self-test`), and `cargo test --workspace --locked`.
- The site in `site/` has its own commands and rules: `site/AGENTS.md`.

## Writing

- Prose, descriptions and comments: tight, clear technical language. No marketing tone, no filler, nothing that reads as generated. American spelling.
