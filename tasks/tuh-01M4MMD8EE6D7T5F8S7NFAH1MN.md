# Decide: website plan questions (stack, hosting, domain, public docs)

`tuh-01M4MMD8EE6D7T5F8S7NFAH1MN`

- **Status:** done
- **Priority:** 3
- **Labels:** `decision` `human` `website`
- **Created:** 2026-10-11 04:44 UTC by `brandon`

## Description

The website plan (docs/plans/website.md, section 10) has 11 questions. Each has a recommended answer; say "go with the recommendations" or change any:

1. Stack: static Astro with no JavaScript (recommended), or a copy of tuhdoo's Next 16? tuhdoo's homepage ships about 172 KB of gzipped JS for static text.
2. Hosting: Vercel Hobby, $0, same account as tuhdoo (recommended); or Cloudflare Pages or GitHub Pages, both $0.
3. Is easyvox.dev yours? It's parked at Namecheap. Recommended: use it, with the apex canonical and www redirecting.
4. When does the site go public? Recommended: the day the repo goes public, at or before the first DMG; until then an unlisted, noindex URL.
5. Which docs are public? Recommended: settings.md, model-bench.md, bench/README.md and a new building.md; not DESIGN.md, branding.md or the plans.
6. Publish boundary: a list in site config, so no files move (recommended), or a separate folder like tuhdoo's.
7. Frontmatter in docs: none (recommended); titles live in the site config.
8. Share tuhdoo's look? Recommended: its structure, not its identity; EasyVox uses the system font and neutral tokens until the brand lands.
9. Move "Build from source" from the README into docs/building.md: yes (recommended).
10. Results page: static SVG charts drawn at build time, each with a table, pinned to named studies (recommended).
11. A path-filtered site CI job (about 1 minute per run) plus site/** in ci.yml's paths-ignore: yes (recommended).

Answers so far (Brandon, 10 Oct): 3. He owns easyvox.dev. 1. Doesn't need Next; unsure about Astro only because he doesn't know it, and his other projects are on Vercel (Astro deploys to Vercel too, so hosting stays the same). Stack still open.

Decided (Brandon, 10 Oct): 1. Astro ("i dont plan to work on the site at all, so astro is fine if thats the best choice"). 2. Vercel, his existing account. 3. He owns easyvox.dev. 4–11: the recommendations, per the orchestrator, unless Brandon objects. The Vercel project, DNS and launch stay Brandon's steps.

## History

### 2026-10-11 04:57 UTC — edit by `brandon`

description edited

### 2026-10-11 06:10 UTC — edit by `brandon`

description edited · status held→done
