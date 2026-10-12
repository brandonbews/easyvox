# EasyVox: monorepo and a light website, planned from tuhdoo's

Ledger task: `tuh-01M4MJQZ9ZPPHC1MZY01T63XB8`. Written 10 October 2026. Nothing in either repo was changed for this plan.

## The recommendation in brief

- **Layout:** keep `crates/` where it is and add `site/` next to it, the way tuhdoo does. The Cargo workspace lists its members by name, so `site/` can't interfere with it.
- **Docs:** one copy of each doc. The site reads `../docs/*.md` when it builds and never copies them. A list in the site's own config says which files are published, so nothing moves.
- **Stack:** Astro 7, building plain static HTML. It reuses tuhdoo's Markdown setup (GitHub-flavoured Markdown (GFM) through unified/remark, the same link rewriting, nav kept in site config) and Biome. Pages ship no JavaScript, and the CSS is plain custom properties with no Tailwind. If you'd rather have a copy of tuhdoo's stack, Next 16 also works (question 1).
- **CI:** add `site/**` to `ci.yml`'s `paths-ignore` before any site file lands, so site work never starts a macOS run. The site gets its own Linux job, about 1 minute per run, which runs only when `site/**` or `docs/**` changes.
- **Hosting:** Vercel Hobby, like tuhdoo. It costs $0, works with a private personal repo, gives PR previews, needs no deploy workflow, and reuses your existing account and the tuhdoo.com DNS steps. Cloudflare Pages ($0, no bandwidth cap) is the runner-up. GitHub Pages also costs nothing extra, since GitHub Pro covers Pages from a private repo, but it has no previews and no custom headers.
- **Launch:** the site goes public when the repo does, at or before the first DMG. Until then it lives on an unlisted preview URL marked noindex.

---

## 1. How tuhdoo does it

### Repo layout

```
tuhdoo/
  cmd/ internal/ embed.go go.mod   Go app (module at the root)
  docs/                            published docs: the content root, the publish boundary
  internal-docs/                   design records, plans (never published)
  site/                            the website, self-contained (own package.json, lockfile, biome, tsconfig)
  npm/  harness/  bin/             distribution, test harness, local build output
  .github/workflows/test.yml       Go job + site job
  .github/workflows/release.yml    tag-triggered release + npm publish
  Makefile                         test = test-go + test-site; lint = lint-go + lint-site
```

The site was built in one PR (#54, 7 August 2026) after a grill session that decided its stack (`tuh-01KZEPBEE8HFDQVK96AQNCQF4G`). It went into the same repo because tuhdoo's ledger is per repo: a separate site repo would have split the backlog. Its rules were that the site owns its own tooling, and the Go module never references `site/`.

### Site generator and toolchain

- **Next.js 16.3** (App Router, TypeScript), **React 19.2**, **Tailwind v4.3** (used only for its theme tokens and utilities, with Preflight off), **Biome 2.5.7** for lint and format, and `tsc --noEmit`. Package manager: **npm** (`site/package-lock.json`), on Node 24 in CI.
- Docs dependencies: `unified`, `remark-parse`, `remark-frontmatter`, `remark-gfm`, `remark-rehype`, `rehype-slug`, `rehype-react`, `unist-util-visit`, `yaml`. That's 12 runtime and 7 dev dependencies, 217 packages in the lockfile, and **425 MB** of `node_modules`.
- Every page is generated at build time. `output: 'export'` is left off on purpose: the pages are static anyway, and leaving it off keeps the security headers in `next.config.ts`, which only work on Vercel.
- **Page weight**, measured on the last local build: the homepage is 39 KB of HTML (8 KB gzipped) and 11 KB of CSS, and it loads **8 scripts, 573 KB raw (about 172 KB gzipped)**: React plus Next's router. The only client component is the docs sidebar, which uses `usePathname`. The site is light to build on, but not light to download.
- Extras built from the same nav config: `llms.txt`, `llms-full.txt`, `sitemap.xml`, `robots.txt`, a manifest, and OpenGraph images.

### Docs: one source, rendered

- The site reads the repo-root `docs/` with `fs` at build time (`site/src/lib/docs.ts`, `DOCS_ROOT = ../docs`). Docs are never copied into `site/`. This works on Vercel because a project with root directory `site/` still checks out the whole repo.
- **The content rules:** GFM, never MDX. Frontmatter holds only `title` and `description`, which feed `<title>` and the meta description and are never rendered. Links are relative paths to real `.md` files, so they work on GitHub first. Heading IDs use GitHub's slug rules (`rehype-slug`, which uses github-slugger), so `file.md#anchor` links work the same on GitHub and on the site.
- **The pipeline** (`site/src/lib/markdown.tsx`) is remark-parse → remark-frontmatter → remark-gfm → one custom visitor that rewrites `.md` links into routes → remark-rehype → rehype-slug → rehype-react. Links that leave `docs/` are left untouched.
- **Components:** every custom element is in one file (`markdown-map.tsx`): heading anchors, internal links, code blocks with a language badge, scrolling tables, images and task checkboxes.
- **Nav and sidebar** come from a hand-written list in `site/src/lib/nav.ts` (slug, file, title), which also sets the order and the prev/next links. The content never knows about the site.
- **The publish boundary** is a directory: everything in `docs/` is public, and everything in `internal-docs/` isn't.

### Styling and theming

- The CSS is semantic component classes on top of design tokens in `globals.css`. Light values sit in Tailwind's `@theme`, and dark mode overrides the same custom properties under `@media (prefers-color-scheme: dark)`. There is no toggle.
- Contrast was computed for every text pair (AA in both themes) and recorded in the CSS comments. Colours come from the logo (emerald on a near-black green). Headings use Sora through `next/font`, and body text uses the system font stack. The logo SVG switches its fills per theme.

### Build, preview, deploy

- **Hosting:** Vercel through its GitHub App. The project's root directory is `site/`, production deploys come from `main`, every PR gets a preview, and no `vercel.json` is needed. A Vercel preview on PR #63 was how the restyle was checked by eye.
- **Domain:** tuhdoo.com was registered at Namecheap and pointed at Vercel. The apex 308-redirects to `www`, which is canonical (`metadataBase`, sitemap, canonical URLs). This was a task for Brandon (`tuh-01KZF2D3MA0P24WKAWK89J0Q0X`).
- **Secrets:** none for the site. The Vercel app needs no repo secrets. `site/.env.local` (gitignored) holds `NEXT_DEV_ORIGINS`, a list of hosts per machine, not a secret. The release workflow uses `github.token` and npm OIDC publishing, so no token secrets exist.
- **CI:** `test.yml` has a `test` job (Go) and a `site` job (`npm ci`, `biome check`, `tsc --noEmit`, `next build`), both on ubuntu-latest. Both are required checks in ruleset 20155938. **It has no path filters:** every push except to the data branch runs both jobs. That's fine on a public repo, where Actions minutes are free.

### What worked

- **The content rules.** One link and anchor check over the first build found zero broken links. The docs read the same on GitHub as on the site, and keeping the nav in site config kept the docs free of anything site-specific.
- **Building on Vercel with root directory `site/`.** It needed no workflow files and gave previews for free. Pushes that don't touch `site/` are skipped, so the daemon's frequent data-branch pushes never start a deploy (checked live, `docs/recipes/vercel.md`).
- **Building `llms.txt` and the sitemap from the nav config,** so they can't drift from the docs.

### What didn't, or cost extra

- **The site shipped without a gate.** It had no lint, no typecheck and no CI job, so a broken site build could merge with green checks. A follow-up task three days later (#63, `tuh-01KZF992N0AM2TZGHMH2T5WYQN`) added Biome, tsc, the CI job, make targets and a required check. **Lesson: add the gate in the same PR as the scaffold.**
- **Vercel tried to deploy the data branch.** Before the root-directory skip was understood, Vercel built failed deployments from the `tuhdoo` data branch on every daemon push. Two fixes failed. The Ignored Build Step never runs, because Vercel's commit-author check for private repos comes first and the daemon isn't a person. A `vercel.json` on the default branch doesn't apply either, because Vercel reads config from the pushed commit. What worked: a `vercel.json` committed on the data branch itself, or a root-directory project, which never sees those pushes. **EasyVox has a `tuhdoo` data branch on origin too.**
- **The framework moves faster than the model's training data.** Next 16 is newer than the model's training data, and `next dev` rewrites `site/AGENTS.md`. PR #60 committed that file so the tree stays clean. Astro 7 (June 2026) has the same issue.
- **Previewing from another machine.** Brandon views the dev server over Tailscale, which needed `allowedDevOrigins`, read from a gitignored env file (#58, #59). EasyVox's dev server will need the same.
- **Copy took more rounds than code:** #62, #71 and #92 were copy passes against a writing bar.
- **Weight:** 172 KB of gzipped JavaScript to show static text, and 425 MB of dev install.
- **Pins:** `test.yml` uses floating tags (`@v4`), while `release.yml` pins SHAs. EasyVox already pins SHAs, with Dependabot to update them.
- **One backlog for both** (`tuh-01KZA0VT234XJYVZWT8YFV8XE2`, held): labels kept site and core work apart well enough. No friction was recorded.

---

## 2. EasyVox today: facts that shape the plan

- **The repo is private** (`gh repo view`: PRIVATE). Pages aren't enabled. It has no rulesets and no branch protection, and **no PRs have ever been opened**: commits go straight to `main`, so "what runs on a pull request" mostly means Dependabot.
- **The Actions budget:** Brandon has GitHub Pro, which includes 3,000 minutes a month, with macOS billed at 10×. That decision is on `tuh-01M4H2EG5RQG3TVM45KP1CVCQS`. Recent CI runs: rustfmt on Linux takes about 15 s (1 billed minute), and the macOS job takes 0:55 to 2:44 (1–3 billed minutes, so 10–30 included minutes). A main push costs about 11–31 included minutes.
- **`ci.yml`** runs on pushes to `main` and on PRs, with `paths-ignore: ["**.md", "docs/**", "LICENSE"]`. **A new `site/` directory isn't in that list**, so until it's added, every site commit starts the macOS job.
- **A small gap that already exists:** the test in `crates/easyvox-app/src/config.rs:582` checks that `docs/settings.md` contains `--print-default-config` word for word. A commit that changes only `docs/settings.md` skips CI, so a stale hand edit is only caught by the next code push.
- **A `tuhdoo` data branch exists on origin.** CI's push trigger is `branches: [main]`, so it's safe. Any host that deploys every branch has to exclude it.
- **The domain:** `easyvox.dev` resolves to Namecheap's parking nameservers (`dns1/dns2.registrar-servers.com`, a parking page, mail forwarding). That's the same setup tuhdoo.com had before launch. The README, `Cargo.toml` (`homepage`) and the bundle id (`dev.easyvox.EasyVox`) already use it. Whether it's Brandon's isn't confirmed here (question 3). `.dev` is on the HSTS preload list, so the site must be HTTPS-only. Every host listed below does that.
- **The bench CSVs have already landed:** 18 files in `docs/bench/`, about 2.5 MB, in commit 25f64d6. They hold numbers and fixed labels only, with no transcript text, as their README says. `studies.csv` has quoted fields, so the CSVs need a real parser, not `split(",")`.
- **The branding brief** (`docs/branding.md`, committed in 9579a09) already outlines the homepage (V9) and the docs (V10). It asks whether the site and a public repo launch with the first DMG (`tuh-01M4MKSK7707E6K6SZ4AYX4R77`, item 6).

---

## 3. Target layout

```
easyvox/
  crates/                 unchanged: easyvox-core, easyvox-app, easyvox-bench
  docs/                   unchanged: the one copy of every doc and of docs/bench/*.csv
  scripts/ packaging/ testdata/ Cargo.toml Cargo.lock rust-toolchain.toml   unchanged
  site/                   NEW, self-contained
    package.json  package-lock.json  astro.config.mjs  tsconfig.json  biome.json
    .gitignore            node_modules/ dist/ .astro/ .env*.local
    AGENTS.md             "Astro 7 is newer than your training data: read its docs first"; the content rules
    public/               favicon, og image (from the brand work)
    src/
      lib/nav.ts          the published-docs list: slug, file, title, description
      lib/docs.ts         fs-reads ../docs; ../README.md never (see §4)
      lib/markdown.ts     unified pipeline + link rewriter (ported from tuhdoo)
      lib/bench.ts        later: CSV loading, schema checks, aggregation (§6)
      lib/results.config.ts  later: which study/build each chart shows
      components/         Header, Footer, Sidebar, PrevNext, Pill demo, chart primitives (.astro)
      layouts/            Base, Docs
      pages/index.astro   homepage
      pages/docs/[...slug].astro   every published doc, from nav.ts
      pages/results.astro later
      pages/robots.txt.ts, sitemap (later), llms.txt.ts (optional)
      styles/global.css   tokens + components, light/dark
  .github/workflows/ci.yml    edited: paths-ignore gains "site/**"
  .github/workflows/site.yml  NEW: Linux, path-filtered
  .github/dependabot.yml      edited: an npm entry for /site, monthly, grouped
```

The crates don't move. tuhdoo's layout gives no reason to move them: its Go module sits at the repo root with `site/` beside it, the same shape as `crates/` plus `site/`. `target/` and `site/node_modules` stay out of each other's way. Root `CLAUDE.md` gets one line pointing at `site/AGENTS.md`.

---

## 4. Docs as a single source

**How:** `site/src/lib/docs.ts` reads files from `../docs` (resolved from `site/`) when the site builds. Vercel, Cloudflare Pages and Actions all check out the whole repo, which tuhdoo confirmed on Vercel. Each page renders through tuhdoo's pipeline, ported almost line for line: remark-parse → remark-gfm → the link rewriter → remark-rehype → rehype-slug → small rehype visitors (heading anchors, scrolling tables, code-block wrappers) → rehype-stringify. The output is used as HTML in an `.astro` page. It's GFM, never MDX, and GitHub's heading IDs keep `model-bench.md`'s many `#anchor` links working.

**Publish boundary:** the list in `nav.ts`. A file is public only if it's listed there, so no files move. tuhdoo's alternative, a directory boundary that moves internal docs to `internal-docs/`, is cleaner but breaks links in `DESIGN.md`, the README, `config.rs` and `scripts/bench/`. It's worth revisiting if `docs/` grows (question 6).

**Frontmatter:** none. Titles and descriptions live next to each entry in `nav.ts`, and the page heading comes from the doc's own `# H1`. GitHub shows YAML frontmatter as a table at the top of a file, and the docs don't need it. Adding frontmatter wouldn't break the `settings.md` test either, since it only checks that the doc *contains* the block (question 7).

**Link rewriting**, extending tuhdoo's rules:

| Link in a published doc | On the site |
|---|---|
| A relative `.md` link to a published doc, with or without `#anchor` | That doc's route, anchor kept |
| A relative link to a repo file that isn't published (`DESIGN.md`, `../../scripts/bench/extract.py`, `LICENSE`) | `https://github.com/brandonbews/easyvox/blob/main/<path>` once the repo is public. While it's private, plain text (no link that would 404), controlled by one flag, `REPO_PUBLIC`, in site config |
| A link to a file that doesn't exist | The build fails |
| `#anchor`, `https:`, `mailto:` | Left as is |

tuhdoo leaves links that point outside `docs/` untouched, which makes them broken relative URLs on the site. EasyVox's docs have several of those.

**What's public:**

| Source | On the site? | Route | Why |
|---|---|---|---|
| `docs/settings.md` | **Yes** | `/docs/settings` | The user reference for every setting. It's generated by the app and checked by a test, and the site renders it as is. |
| `docs/model-bench.md` | **Yes**, as the full report behind the results page | `/docs/model-bench` | Honest numbers with their methods. It's dense, so the results page is the summary. |
| `docs/bench/README.md` | **Yes**, as "Bench data" | `/docs/bench` | The data dictionary for the CSVs. Optionally the build also exposes the CSVs as downloads, generated at build time, so no committed copies. |
| `README.md` | **No**: it's the repo's GitHub front page | — | Its words feed the homepage, rewritten by hand as tuhdoo did. The "Build from source" steps move to `docs/building.md` and the README keeps a short pointer, so one copy shows on both (question 9). |
| new `docs/building.md` | Yes | `/docs/building` | It's the only way to install until the DMG exists. |
| new `docs/README.md` (optional) | Yes | `/docs` | A short docs index, as in tuhdoo. Without it, `/docs` redirects to the first entry. |
| `DESIGN.md` | **No** | Linked on GitHub once public | 77 KB of internal working notes and decisions. Good for contributors, not for users. |
| `docs/branding.md`, `docs/ui-stack-spike.md`, `docs/spikes/` | No | — | Internal briefs and history. |
| `CLAUDE.md`, `site/AGENTS.md` | No | — | Agent instructions. |
| `LICENSE`, `packaging/THIRD_PARTY_NOTICES.html` | Linked | GitHub, or copied into the build output at build time | Credits and the license. |

Later docs from the branding brief's V10 (Install, First launch, Dictating, Models, Privacy, Troubleshooting, Uninstall) are new `docs/*.md` files added to `nav.ts` when the app is ready for them.

Afterwards, once the repo is public: `config.rs:53` prints a GitHub link to `docs/settings.md` inside the default config. It could point at `https://easyvox.dev/docs/settings` instead. That's one change to the generated block and the doc together, which the test keeps in step.

---

## 5. Homepage outline

> **As built (Brandon, 11 October):** the outline below was trimmed. The page is now the pitch ("Simple, local, fast speech-to-text."), the buttons and the pill demo; Fast, Preview as you talk, Local, and Free and open source, a line each; three numbers on lightness; the two models; the questions; and install. The footer is the logo, Results and Docs (plus GitHub, License and Third-party notices once the repo is public), and "Made by Brandon Bews · Free and open source (MIT)". `site/src/pages/index.astro` is the source of truth.

This follows `docs/branding.md` V9, with a few additions from this plan:

1. **Hero:** the wordmark. "A free, open source voice-to-text app for macOS." Then "Hold a shortcut and speak. EasyVox shows the words as it hears them, and when you let go it pastes the text where your cursor is." Below that, a **pill demo** in CSS only: words fade in once, with a static frame under `prefers-reduced-motion`. It uses invented sample text, never text from recordings. The buttons say "Coming soon" or "Build from source" until a DMG exists, then Download and GitHub.
2. **Private:** the model runs on your Mac. No audio or text leaves it, the only network use is downloading a model (checked against its sha256), and History keeps text, never audio.
3. **Light:** idle, it does nothing (it wakes about once every 3 s). While you dictate it adds about 0.65 W, and the model is freed after 10 minutes unused (about 90 MB idle). Under it: "One Mac (M4 mini), one speaker" and a link to `/results` once that page exists.
4. **How it works:** three steps. Hold ⌥Space → watch the words, confirmed and provisional → let go, and the text is pasted and stays on the clipboard.
5. **Models:** Parakeet TDT 0.6B v2 (the default, 539 MB) and Whisper base.en (85 MB). Both are downloaded in the app and credited, with their licenses.
6. **For tinkerers:** one TOML settings file, applied as soon as you save → `/docs/settings`.
7. **Requirements:** Apple silicon, macOS 26 or later, English for now.
8. **Install:** DMG, Homebrew cask, or build from source, marked "coming soon" until a release exists. Keep the early-development banner from the README until then.
9. **Open source:** MIT, the GitHub link (hidden while the repo is private), and credit to transcribe.cpp and Handy.
10. **Footer:** license, third-party notices, model credits, GitHub.

Header nav: Docs, Results (added when the page lands), GitHub (once public).

---

## 6. The results page: data path

```
docs/bench/*.csv  (committed by scripts/bench/extract.py; numbers and fixed labels only)
   │  read with fs at build time, never fetched by the browser
   ▼
site/src/lib/bench.ts
   - parse with d3-dsv (build-time only) or a ~30-line RFC 4180 parser (studies.csv has quoted fields)
   - schema check: required columns per file; the build fails loudly if extract.py renames one
   - select rows by site/src/lib/results.config.ts: each chart names its study, build(s), model(s)
     e.g. idle  → idle.csv, study "idle-app", build "d33ab36", states loaded/unloaded
          accuracy → bench_runs.csv + clips.csv, study "results"/"v3"
          energy → energy.csv (j_per_min, w_added) for the lightness study
          latency → runs.csv first_words_ms / finalize_ms; lag_quantiles.csv for the preview's lag
          silence → stress.csv speech_found / text_kept
   - aggregate (medians, ranges) and downsample time series (power.csv has 22k rows) to the points drawn
   ▼
src/components/charts/*.astro: bar, dot-range, small line; static inline SVG drawn at build time
   - light/dark via CSS custom properties; an accessible <table> of the same numbers under each chart
   - caveats printed with the chart, not in a footnote: one M4 Mac mini, one speaker, dates from studies.csv
   ▼
/results  (no JavaScript) → links to /docs/model-bench for methods and /docs/bench for the data
```

- **Pinned choices, not "latest":** the CSVs mix studies and older builds. Pinning each chart to named studies and builds means a new bench run never silently changes a published number. Updating the page is a one-line config change, made on purpose.
- **Privacy:** the site reads only `docs/` (and never `recordings/`). The CSVs already contain no text. As a cheap guard, the schema check can reject any cell outside the expected numeric or label shape.
- **CI:** `docs/**` is in the site job's path filter, so a new extraction rebuilds and rechecks the page.
- **Charts:** hand-written SVG components with no chart library. The `dataviz` skill should guide the colours and marks. Interactivity, if it's ever wanted, goes in an Astro island on this page only.

---

## 7. CI

**First, before any site file exists**, edit `ci.yml`:

```yaml
paths-ignore: ["**.md", "docs/**", "site/**", "LICENSE"]   # push and pull_request
```

This is the main cost risk. Each site commit that reaches macOS CI costs 10–30 included minutes, so 50 commits while scaffolding would use half a month of Pro's 3,000.

**Optional, fixes the settings.md gap:** switch to `paths:` with negations (later patterns win):

```yaml
paths: ["**", "!**.md", "!docs/**", "!site/**", "!LICENSE", "docs/settings.md"]
```

**New `site.yml`:** ubuntu-latest, `timeout-minutes: 10`, the same SHA-pinned style as `ci.yml`.

```yaml
on:
  push:          { branches: [main], paths: ["site/**", "docs/**", ".github/workflows/site.yml"] }
  pull_request:  { paths: ["site/**", "docs/**", ".github/workflows/site.yml"] }
  workflow_dispatch:
jobs:
  site:  checkout (persist-credentials: false) → setup-node 24 (cache npm, site/package-lock.json)
         → npm ci → npm run lint (biome check) → npm run check (astro check) → npm run build
```

- **Cost:** about 1 billed Linux minute per run, against 10–30 for a macOS run. It also runs on `main` pushes, since there are no PRs. The host's own build does the deploy, so this job is only the gate.
- **On a PR (in practice, Dependabot):** each workflow runs only if its paths changed. A site-only PR never starts macOS. A crates-only PR never starts the site job.
- **On main:** the same filters. The host deploys after its own build. A failed build leaves the last good deploy live.
- **Required checks:** none today, since there are no rulesets. If any are added later, note that a path-filtered required check never reports on PRs that skip it, and those PRs block. Use an always-run gate job, or leave the check optional.
- **Dependabot:** add `npm` for `/site`, monthly, grouped like the Actions entry. Its PRs run only the Linux site job.
- **If GitHub Pages is chosen:** add `upload-pages-artifact` and `deploy-pages` steps on `main`, with `pages: write` and `id-token: write`, and no secrets.

---

## 8. Hosting options

| | Vercel Hobby (recommended) | Cloudflare Pages | GitHub Pages (Actions) | Netlify Free |
|---|---|---|---|---|
| **Cost** | $0. Personal, non-commercial use only. Pro is $20/month per member. | $0. 500 builds a month, 1 concurrent, unlimited bandwidth. | $0 extra: Pro (already paid) covers Pages from private repos. Builds use Actions Linux minutes, about 1 per deploy. | $0 for 300 credits a month. A production deploy costs about 15, so about 20 deploys a month. The site pauses when credits run out. |
| **Private repo** | Yes (personal account). The commit author must be the owner: Brandon's commits deploy, Dependabot's show a warning. | Yes (Cloudflare GitHub app) | Yes, with Pro. The site itself is public. A private site needs Enterprise Cloud. | Yes |
| **Builds on** | Vercel | Cloudflare | Actions | Netlify |
| **PR / branch previews** | Yes | Yes | No | Yes (free of credits) |
| **Security headers** | `vercel.json` | `_headers` file | **None** | `_headers` |
| **The `tuhdoo` data branch** | Root directory `site/` skips it, as checked live in tuhdoo | Exclude `tuhdoo` in branch deploy controls, plus build watch paths. A push of 20 or more commits bypasses watch paths, so the exclusion matters. | Not touched: the workflow runs on `main` only | Needs a branch setting |
| **DNS for easyvox.dev at Namecheap** | A + CNAME records (the tuhdoo.com steps) | The apex needs Cloudflare nameservers. A `www`-only CNAME works without them. | 4 A records + a `www` CNAME. HTTPS is provided. | A/ALIAS + CNAME |
| **New accounts** | None (already used for tuhdoo) | A Cloudflare account | None | A Netlify account |
| **Limits that matter** | 100 GB transfer a month; over that, the site pauses rather than billing | None in practice | 1 GB site, 100 GB/month bandwidth (soft limit), 10-minute deploy timeout | Credits |

**Why Vercel:** you already have the account and the DNS steps from tuhdoo.com. Deploys need no workflow file, it gives previews you can open from any machine (useful since you work remotely), and it costs nothing. A free open source app fits the non-commercial terms. The site is static either way, so moving to Cloudflare or GitHub Pages later is a settings change, not a migration. **Pick Cloudflare Pages instead** if you want no bandwidth cap or no non-commercial clause. **Pick GitHub Pages** if you want no third party at all and can live without previews or headers. Netlify's credit plan is the weakest fit for a site that changes whenever the docs do.

**While the repo is private:** connect the host but **don't attach easyvox.dev yet**. Serve from the host's URL with `robots.txt` set to disallow everything and `noindex` on every page, and hide the GitHub links (`REPO_PUBLIC = false`). An "open source" homepage whose GitHub link 404s would break its own promise.

---

## 9. Steps, smallest first

1. **Answer the questions below** (about 10 minutes).
2. **Guard CI** (one line, before any site file): add `site/**` to `ci.yml`'s `paths-ignore`. Optionally fix the settings.md gap at the same time. Workflow edits need your review.
3. **Scaffold `site/`:** Astro 7 pinned to an exact version, Biome, `astro check`, `.gitignore`, `site/AGENTS.md`, a placeholder homepage, and the base layout with light/dark tokens. Build locally only. The dev server listens on the network, with allowed hosts read from `site/.env.local`, so you can preview over Tailscale.
4. **Add the gate in the same PR:** `site.yml` (Linux, path-filtered) and the npm entry in `dependabot.yml`. This is tuhdoo's lesson.
5. **The docs pipeline:** port `nav.ts` and `rewriteMdLink` with the private-repo rule, add the docs layout (sidebar, prev/next), and publish `docs/settings.md` first. Check its tables, the TOML block and its anchors. Then add `model-bench.md` and `bench/README.md`.
6. **Homepage copy and look:** write the copy from §5 and pick neutral, Apple-like tokens until the brand directions land. The favicon and og image come from the brand work.
7. **Hosting, done by you:** create the Vercel project with root directory `site/` (Astro is detected automatically) and confirm it serves a preview URL with noindex. Attach no domain.
8. **Move "Build from source" into `docs/building.md`**, leaving a pointer in the README, if you say yes to question 9.
9. **The results page:** `bench.ts` with the schema check, `results.config.ts`, 4–6 SVG charts with their tables and caveats, and a link to the full report. Its inbox task (`tuh-01M4MGCF0JZW3A508YWHTS4140`) becomes the ledger task for this.
10. **Launch, together with the repo going public and the first DMG:** attach easyvox.dev (and decide apex or `www`), add the DNS records at Namecheap, remove noindex, set `REPO_PUBLIC = true`, and add the sitemap, og cards and, optionally, `llms.txt`. Point `config.rs`'s settings link at the site.

---

## 10. Questions for Brandon

1. **Stack: Astro (static, no JavaScript) or a copy of tuhdoo's Next 16?** *Recommended: Astro 7.* It keeps tuhdoo's content rules, its link rewriter (ported), its nav-in-config pattern and Biome, but ships HTML and CSS only. tuhdoo's homepage loads about 172 KB of gzipped JavaScript to show static text, which doesn't fit a site selling lightness. Next would be quicker to stand up, since about 10 files would copy over, and it's one framework across both projects. If you choose Next, use `output: 'export'` only if you leave Vercel.
2. **Hosting?** *Recommended: Vercel Hobby, $0*, the same account and DNS steps as tuhdoo. The alternatives are Cloudflare Pages ($0, no bandwidth cap, needs Cloudflare nameservers for the apex) or GitHub Pages ($0 extra with your Pro plan, Actions minutes, no previews or headers).
3. **Is easyvox.dev yours, and should the apex or `www` be canonical?** It's parked on Namecheap's nameservers, like tuhdoo.com was. *Recommended: yes, use it, with the apex `easyvox.dev` canonical and `www` redirecting to it*, since the README, `Cargo.toml` and the bundle id all say `easyvox.dev`. (tuhdoo went the other way, with `www` canonical.)
4. **When does the site go public?** *Recommended: on the same day the repo goes public, at or before the first DMG.* Until then it stays on the host's unlisted URL with noindex. This also answers item 6 of the branding decision: the favicon and social cards become v1 if you agree.
5. **Which docs are public?** *Recommended: `settings.md`, `model-bench.md` (as the full bench report), `bench/README.md` (as bench data), and a new `building.md`.* Not `DESIGN.md`, `branding.md`, `ui-stack-spike.md` or `spikes/`. `DESIGN.md` gets linked on GitHub once public.
6. **How is the publish boundary set: a list in site config, or a directory like tuhdoo's?** *Recommended: a list (`nav.ts`) for now*, so no files move and no links break. Move internal docs to `internal-docs/` only if `docs/` grows.
7. **Frontmatter in the docs?** *Recommended: no.* Titles and descriptions live in `nav.ts`, and the docs stay plain GitHub Markdown, with no frontmatter table showing on GitHub.
8. **Should the site share tuhdoo's look?** *Recommended: share its structure, not its identity:* the layout, docs shell, token CSS, light/dark following the system, and computed AA contrast. Its green and Sora stay tuhdoo's. EasyVox uses the system font (SF Pro), as the brief's "native" personality suggests, and neutral tokens until the brand directions land.
9. **Should "Build from source" move from the README into `docs/building.md`?** *Recommended: yes, when the docs pages land.* The README keeps a three-line summary and a link, so one copy shows on GitHub and the site.
10. **How should the results page be built?** *Recommended: static SVG charts drawn at build time, each with a table, no JavaScript, each pinned to named studies and builds in site config.* A new bench run then changes the page only on purpose.
11. **Should there be a site CI job in Actions on top of the host's build?** *Recommended: yes, a path-filtered Linux job* (about 1 billed minute per run), not required, since there are no rulesets today. Also add `site/**` to `ci.yml`'s `paths-ignore` first, in any case.

---

## Sources for hosting facts (checked 10 October 2026)

- GitHub plans, which list Pages for private repos under Pro (a private site needs Enterprise Cloud): https://docs.github.com/en/get-started/learning-about-github/githubs-plans
- GitHub Pages limits (1 GB site, 100 GB/month soft limit, 10 builds an hour unless built by a custom workflow, no commercial-business use): https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits
- Cloudflare Pages limits (500 builds a month, 1 concurrent build, 20,000 files): https://developers.cloudflare.com/pages/platform/limits/
- Cloudflare Pages build watch paths (pushes with 20 or more commits or 3,000 or more files bypass them): https://developers.cloudflare.com/pages/configuration/build-watch-paths/
- The Vercel Hobby non-commercial clause and limits come from third-party 2026 summaries (for example https://www.fencode.dev/en/blog/vercel-free-vs-pro-2026-official-limits-pricing). Confirm on vercel.com/pricing.
- Netlify's credit plan (300 credits a month, about 15 per production deploy) also comes from third-party 2026 summaries (for example https://netli.fyi/blog/netlify-free-plan-limits-2026). Confirm on netlify.com/pricing.
- tuhdoo's Vercel and data-branch behaviour: `tuhdoo/docs/recipes/vercel.md` and tasks `tuh-01KZF2D3MA0P24WKAWK89J0Q0X` and `tuh-01KZPW8CZPKF2KTWMA5B8QYVN0`.
