# Dependabot for the GitHub Actions SHA pins

`tuh-01M4H2EGKDEZ7NPDTDRYRD66ZT`

- **Status:** open — ready
- **Priority:** 3
- **Labels:** `ci` `deps`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: .github/workflows/ci.yml pins each action to a commit SHA with the release in a comment. Nothing keeps those pins current.

The ask: add .github/dependabot.yml for the github-actions ecosystem, grouped into one PR per run, updating the SHA and its version comment together.

Acceptance: the config validates on GitHub (Insights → Dependency graph → Dependabot); a Dependabot PR bumps a pin and its comment.

Constraints: every Dependabot PR runs CI on macOS, which bills at 10× while the repo is private (tuh-01M4H2EG5RQG3TVM45KP1CVCQS). Keep the schedule monthly and grouped unless the repo goes public.

## History

_No activity yet._
