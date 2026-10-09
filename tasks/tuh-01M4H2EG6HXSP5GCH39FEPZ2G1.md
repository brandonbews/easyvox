# Push the 7 local commits on main (and let tuhdoo publish its data branch)

`tuh-01M4H2EG6HXSP5GCH39FEPZ2G1`

- **Status:** done
- **Priority:** 1
- **Labels:** `human` `decision`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Brandon's call; agents release it. Mark it done once pushed.

- main is 7 commits ahead of origin/main (5450cd3 "Add light/dark mode and overlay animation to design notes" through c599fcd "Document the stack decision, model list and build steps"). `git push origin main` triggers CI on macOS; see the public-repo/spending-limit decision first.
- The tuhdoo data branch (`tuhdoo`) exists only on Brandon's dev machine. The tuhdoo daemon pushes it to origin on every sync, so a pre-push hook (.git/hooks/pre-push, refusing only refs/heads/tuhdoo) holds it back. To publish the ledger: `rm .git/hooks/pre-push`; the daemon pushes within a minute. ci.yml only triggers on main and PRs, so the data branch starts no CI.

Done 2026-10-09: Brandon OK'd pushing; main pushed through 13618ce (8 commits, first CI run 37985576580). The pre-push hook was removed and the daemon published the tuhdoo branch.

## History

### 2026-10-09 20:14 UTC — edit by `brandon`

description edited · status held→done
