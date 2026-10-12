# Homebrew cask for the notarized DMG

`tuh-01M4H2EGPP2HDY5Y1WD6ENDTTS`

- **Status:** open — blocked on dependencies
- **Priority:** 4
- **Labels:** `release`
- **Depends on:** [`tuh-btdp`](tuh-01M4H2EGNVZ00YA6604MXABTDP.md) (open), [`tuh-vcqs`](tuh-01M4H2EG5RQG3TVM45KP1CVCQS.md) (done)
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: DESIGN.md "Platform and distribution": one of the three install paths is a Homebrew cask pointing at the same DMG on GitHub Releases. Homebrew's main cask repo has been phasing out apps that aren't signed and notarized, so this waits on the shipping build, and on the repo/releases being public.

The ask: a cask (in Brandon's own tap first; homebrew/cask once eligible) for arm64 and macOS 26 or later, with the DMG URL and sha256, bumped by the release workflow on each tag.

Acceptance: `brew install --cask easyvox` installs and launches the app; `brew audit --cask --strict` is clean; a new tag updates the cask without hand edits.

Pointers: the release workflow from the shipping-build task; DESIGN.md "Signing and notarization (macOS)".

Note: the repo is private for now (decision tuh-01M4H2EG5RQG3TVM45KP1CVCQS). The cask needs a publicly downloadable DMG, so make the repo public (or host releases publicly) before this ships.

12 Oct: the DMG is hosted on GitHub Releases: https://github.com/brandonbews/easyvox/releases/download/v#{version}/EasyVox-#{version}.dmg. Brandon creates the public tap repo brandonbews/homebrew-tap. Cask: depends_on arch: :arm64, depends_on macos: :tahoe, uninstall quit:, zap, livecheck against GitHub releases; no auto_updates (brew upgrade). Check with brew audit --new --cask --strict --online and brew style. Install: brew install --cask brandonbews/tap/easyvox. Bumping the cask from the release workflow would need a token for the tap repo; decide then (an agent bump by hand is the default).

## History

### 2026-10-09 19:41 UTC — edit by `brandon`

description edited

### 2026-10-12 06:44 UTC — edit by `brandon`

description edited
