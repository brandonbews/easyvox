# Restyle the website to the design hand-off, plus the README lockup and DMG background (lanes B and C)

`tuh-01M4NR75KRQGBNJTMCA0Q44N9Y`

- **Status:** open — ready
- **Priority:** 1
- **Labels:** `design` `website`
- **Created:** 2026-10-11 15:10 UTC by `brandon`

## Description

Brandon accepted every recommendation of the design triage on 11 October. Lane B (B1 tokens, favicons, head; B2 social cards re-rendered with macOS 26 / Apple silicon; B3 homepage; B4 docs layout) and C1 (README lockup, packaging/dmg/background.tiff). Brandon: 'on any copy, especially the website, you can treat that as more or less placeholder unless any of it is genuinely good'.

Done (11 October, uncommitted in the main checkout). B1: brand tokens and recomputed AA contrast in global.css (dark background now #1C1C1E, theme-color agrees); favicons, manifest and og/twitter tags in Base.astro (relative until SITE_URL, a new launch switch in config.ts); the lockup (inline SVG mark + live text) in Header and Footer. B2: social cards re-rendered from board 20 with "macOS 26 or later · Apple silicon" and "Free, open source dictation that runs on your Mac.": site/public/og-image.png (dark), design/exports/ holds the GitHub previews (dark and light, for Brandon to upload at launch) and the light og card. B3: homepage in the boards' layout with true copy; Light, Models and Requirements kept; Pill.astro restyled (44 px, three vermilion strokes, provisional words faded, the app's pop, plays once); FAQ as <details>; new footer. B4: docs sidebar grouped (nav.ts gains `group`), 640 px prose, breadcrumbs, keycap chips, a phone nav in <details>, card prev/next. C1: README <picture> lockup; packaging/dmg/background.tiff (not wired in); Finder settings appended to tuh-01M4H2EGNVZ00YA6604MXABTDP.
Deviations: no menu button on the homepage header on phones (two links fit, and a CSS-only menu can't close after a same-page jump to #install); header CTA is "Install" → /#install until a DMG exists; no "On this page" rail, search, edit links or version number (Later items); links in running text are underlined (dark link vs text is 2.8:1).
Checks: npm ci, lint, check, build pass; no <script>, no .js. Gzipped HTML+CSS: home 5.3 → 11.6 KB, docs pages +4.2 KB each. Screenshots: scratchpad site-restyle/screenshots/.

## History

### 2026-10-11 15:38 UTC — edit by `brandon`

description edited
