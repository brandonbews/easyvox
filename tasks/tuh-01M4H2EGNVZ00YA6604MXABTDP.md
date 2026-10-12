# Shipping build: hardened runtime, DMG (dmgbuild), release script and a GitHub Actions workflow that signs and notarizes on a tag

`tuh-01M4H2EGNVZ00YA6604MXABTDP`

- **Status:** open — blocked on dependencies
- **Priority:** 3
- **Labels:** `release`
- **Depends on:** [`tuh-2645`](tuh-01M4H2EGM7TG4S96QNRQHV2645.md) (on hold)
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: DESIGN.md "Signing and notarization (macOS)" and "Platform and distribution". A downloaded app opens without a warning only if it's signed with a Developer ID and notarized. Plan (12 Oct, replacing cargo-packager): extend scripts/bundle.sh and add scripts/release.sh; dmgbuild 1.6.7 via uvx for the DMG; GitHub Actions signs and notarizes on a v* tag in a `release` environment with Brandon as the required reviewer; the DMG goes on GitHub Releases (the repo goes public first). Release plan: the previous session's scratchpad release/plan.md.

The ask:
- packaging/EasyVox.entitlements with only com.apple.security.device.audio-input; bundle.sh signs dist and release builds with --options runtime and the entitlements (ad hoc locally), never --dev builds.
- packaging/dmg/settings.py (dmgbuild) with the Finder layout below.
- scripts/release.sh: ad hoc by default; --identity and --notarize for CI; notarize and staple the app, then sign, notarize and staple the DMG; prints the sha256 and the cask values; refuses a dirty tree or a tag that doesn't match the Cargo.toml version.
- .github/workflows/release.yml: temporary keychain, notarytool with an App Store Connect API key, uploads EasyVox-<version>.dmg to the GitHub Release. Workflow edits need Brandon's review.
- CHANGELOG.md; docs/releasing.md (short, public).
- Agents sign ad hoc only and never trigger or approve a signing run.

Acceptance: a CI-built DMG opens on a clean Mac with no Gatekeeper prompt (`spctl -a -vv` reports a notarized Developer ID); `codesign --verify --deep --strict` passes; the Microphone prompt and dictation work under the hardened runtime.

Constraints: bundle id dev.easyvox.EasyVox never changes (macOS ties permissions and the shader cache to it); no secrets in the repo or on the Mac; ad-hoc dev builds keep working.

Design hand-off (11 October): the DMG background and Finder window settings are in design/assets/dmg/ (README.md). Ship the light background.

DMG background staged (11 October, lane C1): packaging/dmg/background.tiff, made with `tiffutil -cathidpicheck` from design/assets/dmg/background.png (660 × 400) and background@2x.png (1320 × 800). Not wired into any script yet. Finder window settings, from design/assets/dmg/README.md:
- Window content size 660 × 400 pt, the same as the picture. Some tools size the window including the title bar: if the bottom is cut off, add about 28 pt to the height.
- Icon view; toolbar, sidebar, path bar and status bar hidden. Icon size 128, text size 13, labels at the bottom, arrange by none.
- Icon centres: EasyVox.app at (170, 164), the Applications alias at (490, 164).
- Background: background.tiff in a hidden .background folder on the volume. Volume name: EasyVox.
- Name the disk image from the crate version (EasyVox-0.1.0.dmg today), not the hand-off's "EasyVox-1.0.dmg".
- create-dmg equivalent: --volname "EasyVox" --background background.tiff --window-pos 200 120 --window-size 660 400 --icon-size 128 --text-size 13 --icon "EasyVox.app" 170 164 --hide-extension "EasyVox.app" --app-drop-link 490 164.
- Only the light picture ships: a DMG background can't follow the appearance. The hand-off says Finder keeps labels black over a background picture even in Dark Mode; docs/branding.md said they turn white. Check by hand on macOS 26, light and dark, before release.

Built and rehearsed ad hoc (12 October, agent, uncommitted in worktree agent-a5c0f787750044b9e):
- Added packaging/EasyVox.entitlements (audio-input only), packaging/dmg/settings.py (dmgbuild 1.6.7 through uvx, with ds_store 1.3.3 and mac_alias 2.2.3 pinned), scripts/release.sh, CHANGELOG.md, .github/workflows/release.yml and docs/releasing.md. bundle.sh signs the dist and --release builds with the hardened runtime; --dev builds stay as they were.
- `scripts/release.sh --allow-dirty` passes end to end: flags=0x10002(adhoc,runtime), the entitlement, no get-task-allow, arm64, minos 26.0. `syspolicy_check notary-submission` reports only the ad-hoc signature. EasyVox-0.1.0.dmg is 9.3 MB (ULFO). Its .DS_Store was read back: window {{200, 120}, {660, 400}}, icons at (170, 164) and (490, 164), size 128, text 13, picture background.
- Two changes from the hand-off. dmgbuild stores the background as the hidden file /.background.tiff, not in a .background folder. There's no hide-extension, because it puts com.apple.FinderInfo on the bundle and that fails `codesign --verify --strict`. Finder hides ".app" anyway.
- The workflow is dormant until the release environment and its five secrets exist. zizmor 1.30.1 and actionlint 1.7.12 (with shellcheck) report nothing.
- Still to do: the self-test under the hardened runtime (an EasyVox was running), the DMG window check (tuh-01M4NQE2KBMZ8WST33GWHJ82YA), the certificate, key and GitHub setup (Brandon's runbook), and the first CI run.

## History

### 2026-10-11 14:56 UTC — edit by `brandon`

description edited

### 2026-10-11 15:36 UTC — edit by `brandon`

description edited

### 2026-10-12 06:44 UTC — edit by `brandon`

retitled · description edited

### 2026-10-12 07:20 UTC — edit by `brandon`

description edited
