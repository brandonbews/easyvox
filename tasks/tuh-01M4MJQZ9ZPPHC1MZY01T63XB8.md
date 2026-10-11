# Monorepo: the app plus a light website (homepage and docs)

`tuh-01M4MJQZ9ZPPHC1MZY01T63XB8`

- **Status:** open — ready
- **Priority:** 3
- **Labels:** `website` `repo`
- **Depends on:** [`tuh-h1mn`](tuh-01M4MMD8EE6D7T5F8S7NFAH1MN.md) (done)
- **Created:** 2026-10-11 04:15 UTC by `brandon`

## Description

Brandon (10 Oct): turn the repo into a monorepo holding the app and a website for EasyVox. Keep the website very light: a basic homepage and some docs. Later it gets the condensed research page (inbox: "Website: a condensed, visual page of our benchmark and efficiency findings").

First step: study the tuhdoo monorepo's website for its architecture and structure (generator, layout, docs pipeline, hosting, build and deploy), and propose the same shape here, with open questions for Brandon. Then restructure and scaffold.

Constraints: lightness is a feature; macOS app first; repo docs stay Markdown; nothing outward-facing (hosting, DNS, a public deploy) without Brandon.

Plan (10 Oct, from studying tuhdoo; full plan in the session scratchpad, website/plan.md):
- tuhdoo: site/ beside the Go app; Next 16 + React 19 + Tailwind 4 + Biome, npm; fs-reads ../docs at build through a thin GFM-only unified/remark pipeline, nav in site config; Vercel (root site/, PR previews, no deploy workflow), tuhdoo.com at Namecheap. Lessons: add the CI gate with the scaffold; the tuhdoo data branch must never deploy; ~172 KB gzipped JS on a static homepage.
- Layout: keep crates/, add a self-contained site/. One copy of each doc: the site reads ../docs/*.md at build, and a list in site/src/lib/nav.ts is the publish boundary (no files move, no frontmatter).
- Stack (recommended): Astro 7 static, no JS, tuhdoo's remark pipeline and link rewriter ported, Biome, plain CSS tokens with light/dark following the system. Alternative: copy tuhdoo's Next stack.
- Public docs: settings.md, model-bench.md, bench/README.md, a new building.md (moved from the README). Not DESIGN.md, branding.md or spikes/.
- Results page: docs/bench/*.csv parsed at build, schema-checked, pinned to named studies and builds in site config, drawn as static SVG with tables and caveats.
- CI: add site/** to ci.yml paths-ignore FIRST (each macOS run costs 10-30 included minutes); a new Linux site.yml job path-filtered on site/** and docs/** (~1 min); npm Dependabot entry, monthly.
- Hosting (recommended): Vercel Hobby, $0, existing account; Cloudflare Pages ($0, no bandwidth cap) or GitHub Pages ($0 with Pro, no previews or headers) as alternatives. Stay on an unlisted noindex URL until the repo goes public; attach easyvox.dev (parked at Namecheap) at launch.
- Open questions for Brandon (11, each with a recommended answer, in the plan): stack, host, easyvox.dev ownership and apex vs www, launch timing, which docs are public, publish boundary, frontmatter, look, the building.md move, how the results page is built, the site CI job.

Scaffold (11 Oct, agent; uncommitted, for review): steps 2-6 and 8 done. ci.yml ignores site/** and site.yml. site/ is Astro 7.3.5, static, no JavaScript (CI fails on any <script> or .js). Docs: unified/remark pipeline ported from tuhdoo, nav.ts is the publish list (building, settings, model-bench, bench data); links to unpublished repo files are plain text while REPO_PUBLIC is false; the build fails on a broken link or #anchor (it found one: model-bench.md linked #the-eased-meter-and-the-softer-glide, now a heading). Homepage from plan section 5 with a CSS-only pill; noindex on every page and a disallow-all robots.txt behind NOINDEX in site/src/config.ts. site.yml (Linux, path-filtered) and an npm Dependabot entry. Build from source moved to docs/building.md, README keeps a pointer. Weight, gzipped HTML+CSS: home 5.2 KB, settings 12.5 KB, model bench 39.4 KB; 0 JS. Gates green: biome, astro check, build, cargo fmt, cargo test. Next (Brandon): review and commit; create the Vercel project with root directory site/, no domain.

Committed as 93fc164 (plan steps 2–6 and 8; site lint, check, build, cargo fmt and test passed). Next, Brandon: create the Vercel project with root directory site/, no domain (plan step 7). Then: the results page (step 9), and launch with the repo going public (step 10).

## History

### 2026-10-11 04:43 UTC — edit by `brandon`

description edited

### 2026-10-11 04:44 UTC — edit by `brandon`

depends_on +tuh-h1mn

### 2026-10-11 06:30 UTC — edit by `brandon`

description edited

### 2026-10-11 06:32 UTC — edit by `brandon`

description edited
