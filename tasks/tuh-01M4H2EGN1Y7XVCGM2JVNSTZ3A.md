# Decide: which auto-updater

`tuh-01M4H2EGN1Y7XVCGM2JVNSTZ3A`

- **Status:** done
- **Priority:** 3
- **Labels:** `human` `decision` `release`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Brandon's call; agents release it. Record the decision in this description and mark it done (DESIGN.md "Open questions").

Context: the DMG on GitHub Releases is meant to auto-update. GPUI Kit has no updater (Tauri had one built in). DESIGN.md "Requirements" says the only network use is downloading models, so an update check needs that line revised either way.

Options:
- Sparkle 2: the macOS standard, EdDSA-signed appcast; an Objective-C framework, so it needs bridging.
- cargo-packager's updater crate, already the planned packager, against GitHub Releases.
- No in-app updater at first: Homebrew cask upgrades plus a "new version" link.

Design hand-off (11 October): Check for Updates… in the menu and on About. If built, explicit and user-initiated (the only other network use is model downloads).

Decided (Brandon, 12 Oct): an explicit "Check for Updates…" that fetches https://dl.easyvox.dev/latest.json only when clicked and offers the download page (release plan §8, option A). No Sparkle, no background check. Ships in 0.1.0. Brew users use brew upgrade (no auto_updates in the cask).

## History

### 2026-10-11 14:56 UTC — edit by `brandon`

description edited

### 2026-10-12 06:26 UTC — edit by `brandon`

description edited

### 2026-10-12 06:26 UTC — edit by `brandon`

status held→done
