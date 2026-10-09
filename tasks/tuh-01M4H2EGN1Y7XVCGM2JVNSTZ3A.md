# Decide: which auto-updater

`tuh-01M4H2EGN1Y7XVCGM2JVNSTZ3A`

- **Status:** on hold — deliberately paused
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

## History

_No activity yet._
