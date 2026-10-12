# Decide: move internal docs (DESIGN.md process notes, plans, branding, research notes, design hand-off, the tuhdoo ledger branch) to a private companion repo before EasyVox goes public

`tuh-01M4PG20BW9EH44AS97E1J3M33`

- **Status:** done
- **Priority:** none
- **Labels:** `human` `decision` `release` `repo`
- **Created:** 2026-10-11 22:06 UTC by `brandon`

## Description

Brandon, 11 Oct, asked about keeping design docs in a private repo beside public ones (he's formalizing his orchestrator elsewhere). Recommended: split by audience. Private companion repo (sibling checkout, added to agents' working dirs; no submodule): process and decision notes, docs/plans, docs/branding.md, the full model-bench lab notes, the design hand-off sources, handoffs, and the ledger. Public: code, README, LICENSE, building and settings docs, the site, brand assets the app and site use, the bench CSVs the results page reads, and a timeless ARCHITECTURE.md for contributors distilled from DESIGN.md. Timing: decide before the repo goes public, because flipping it publishes every commit and every pushed branch (origin/tuhdoo holds the ledger). Simplest safe path: publish a fresh one-commit history.

Decided (Brandon, 12 Oct): no private companion repo. The docs stay in the repo, as long as nothing personal or odd was ever committed; make them logical and tight before the repo goes public. Follow-up task created for the audit.

## History

### 2026-10-12 06:14 UTC — edit by `brandon`

description edited

### 2026-10-12 06:14 UTC — edit by `brandon`

status held→done
