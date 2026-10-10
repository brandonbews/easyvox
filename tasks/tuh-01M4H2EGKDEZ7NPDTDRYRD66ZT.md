# Dependabot for the GitHub Actions SHA pins

`tuh-01M4H2EGKDEZ7NPDTDRYRD66ZT`

- **Status:** done
- **Priority:** 3
- **Labels:** `ci` `deps`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: .github/workflows/ci.yml pins each action to a commit SHA with the release in a comment. Nothing keeps those pins current.

The ask: add .github/dependabot.yml for the github-actions ecosystem, grouped into one PR per run, updating the SHA and its version comment together.

Acceptance: the config validates on GitHub (Insights → Dependency graph → Dependabot); a Dependabot PR bumps a pin and its comment.

Constraints: every Dependabot PR runs CI on macOS, which bills at 10× while the repo is private (tuh-01M4H2EG5RQG3TVM45KP1CVCQS). Keep the schedule monthly and grouped unless the repo goes public.

Finding (2026-10-10, agent): added .github/dependabot.yml: github-actions ecosystem, directory /, monthly, one group (patterns ["*"]) so each run opens one PR. Dependabot updates a SHA pin and its `# vX.Y.Z` comment together when the comment names the release, as ci.yml's do. Checked locally: it parses (Ruby's YAML) and passes the Dependabot schema (check-jsonschema 0.36.2, --builtin-schema vendor.dependabot), which does reject a bad interval. Still to check after it's pushed: Insights -> Dependency graph -> Dependabot shows it, and the first grouped PR bumps a pin and its comment.

Done in 9b8ebd3 (monthly, grouped; passes check-jsonschema). The first Dependabot PR will show the pin+comment bump.

## History

### 2026-10-10 20:38 UTC — edit by `brandon`

description edited

### 2026-10-10 20:46 UTC — edit by `brandon`

description edited · status open→done
