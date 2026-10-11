# Restyle the app to the design hand-off: brand, icon, menu bar, pill, notices, placement (lane A1-A6)

`tuh-01M4NR75GB4AD01K44V0W0P35Y`

- **Status:** open — ready
- **Priority:** 1
- **Labels:** `design` `app` `overlay`
- **Created:** 2026-10-11 15:10 UTC by `brandon`

## Description

Brandon accepted every recommendation of the design triage on 11 October (scratchpad design-triage.md section 5). Lane A steps A1-A6, one agent in order, in a git worktree so Brandon's test build in the main checkout is untouched. Copy is placeholder unless genuinely good.

Part 1 (A1, A2) done in worktree agent-a5c0f787750044b9e, uncommitted: brand.rs (tokens, branded GPUI Kit light/dark theme, brand::Assets), assets/brand/ (icon 128/256, mark and knot SVGs, menu bar glyphs as 1x/2x PNGs; Getting ready is a static whole ring), window sizes 560x540 / 760x540 / 760x520, packaging/AppIcon.icns via iconutil + CFBundleIconFile + bundle.sh copy, the new mic string, DESIGN.md Brand section. fmt, both clippys and tests pass; the bundle builds. Self-test not run: Brandon's EasyVox was running.

## History

### 2026-10-11 15:23 UTC — edit by `brandon`

description edited
