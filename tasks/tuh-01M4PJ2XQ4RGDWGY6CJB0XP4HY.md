# Site: a _headers file for Cloudflare Pages (security headers; X-Robots-Tag noindex while NOINDEX is on)

`tuh-01M4PJ2XQ4RGDWGY6CJB0XP4HY`

- **Status:** open — ready
- **Priority:** none
- **Labels:** `website`
- **Created:** 2026-10-11 22:42 UTC by `brandon`

## Description

Cloudflare Pages reads site/public/_headers (copied to dist). Zero-JS site, so a strict CSP should hold: check the built HTML for inline styles, data: URIs and anything else it must allow before writing it. Also X-Content-Type-Options, Referrer-Policy, Permissions-Policy, frame-ancestors none. The noindex header must follow NOINDEX in site/src/config.ts (generate the file at build, or document the manual switch). Do it once Brandon has the Pages project (tuh-01M4MTMN0H5XNND5TMTCRT4ETD), so it can be checked on the preview URL.

## History

_No activity yet._
