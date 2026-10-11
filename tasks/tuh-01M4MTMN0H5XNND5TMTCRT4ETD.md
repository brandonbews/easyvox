# Create the Cloudflare Pages project for the website (root site/, no domain)

`tuh-01M4MTMN0H5XNND5TMTCRT4ETD`

- **Status:** on hold — deliberately paused
- **Priority:** 3
- **Labels:** `human` `website`
- **Created:** 2026-10-11 06:33 UTC by `brandon`

## Description

Plan step 7 (docs/plans/website.md). About 5 minutes in the Vercel dashboard:

1. Add New → Project → import brandonbews/easyvox. If it isn't listed, give Vercel's GitHub app access to the repo.
2. Root Directory: site. Framework: Astro (detected). Leave the build command (npm run build) and output directory (dist) as they are.
3. Keep "Skip deployments when there are no changes to the root directory" on. That's what keeps Vercel off the tuhdoo data branch, which has no site/ (tuhdoo learned this the hard way).
4. Deploy. Attach no domain yet. Every page carries noindex until launch.
5. Tell Claude the preview URL. The site's base URL goes in site/src/config.ts for canonical links later.

The domain (easyvox.dev, apex canonical) and removing noindex happen at launch, with the repo going public (plan step 10).

Brandon, 11 Oct: Cloudflare Pages instead of Vercel (he's moving tuhdoo there too). Settings given to him: Git-connected project on the private repo; framework preset Astro or none; root directory site; build command npm run build; output directory dist; env NODE_VERSION=24 (package.json engines >=24); production branch main; preview branches exclude tuhdoo (the ledger's data branch has no site/) and dependabot/*; build watch paths site/* and docs/*. No domain until launch. When he sends the pages.dev link: update .github/workflows/site.yml's comment and docs/plans/website.md's hosting notes, and add site/public/_headers (tuh: security headers task).

## History

### 2026-10-11 22:42 UTC — edit by `brandon`

description edited

### 2026-10-11 22:42 UTC — edit by `brandon`

retitled
