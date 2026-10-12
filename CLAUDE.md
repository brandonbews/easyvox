# EasyVox

## Working with tuhdoo

- The backlog is on the ledger: `tuhdoo backlog`. Agents: `tuhdoo protocol`, then `claim_next`.
- Ideas: `tuhdoo create "<title>" --status inbox`. Title-only is fine. Ideas do not go in markdown files.
- Open questions: on the ledger as `held` tasks labelled `decision` and `human`.
- Tasks that need Brandon carry the `human` label and stay `held`, so agents are never served them. Hands-on checks (a person at an unlocked screen) also carry `hands-on`.

## Private recordings

- `recordings/` holds Brandon's own voice recordings for local benchmarks. It's gitignored: never commit, upload, publish or delete these files, and don't paste their transcripts into committed files or artifacts. Bench results (WER, latency, energy) are fine to record.

## Writing

- Prose, descriptions and comments: tight, clear technical language. No marketing tone, no filler, nothing that reads as generated.
