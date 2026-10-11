# Shipping build: cargo-packager for Developer ID signing, notarization and the DMG

`tuh-01M4H2EGNVZ00YA6604MXABTDP`

- **Status:** open — blocked on dependencies
- **Priority:** 3
- **Labels:** `release`
- **Depends on:** [`tuh-2645`](tuh-01M4H2EGM7TG4S96QNRQHV2645.md) (on hold)
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: DESIGN.md "Signing and notarization (macOS)" and "Platform and distribution". A downloaded app opens without a warning only if it's signed with a Developer ID and notarized. cargo-packager is the planned tool, automated in GitHub Actions with the certificate and an App Store Connect API key as secrets. The DMG holds only the app (~25 MB).

The ask: cargo-packager config that builds EasyVox.app with the hardened runtime and the com.apple.security.device.audio-input entitlement, signs it with the Developer ID, notarizes and staples it, and wraps it in a DMG; a release workflow that does this on a version tag using the secrets; README steps for a local signed build.

Acceptance: a CI-built DMG opens on a clean Mac with no Gatekeeper prompt (`spctl -a -vv` reports a notarized Developer ID); `codesign --verify --deep --strict` passes; the Microphone prompt and dictation work under the hardened runtime.

Pointers: scripts/bundle.sh (today's ad-hoc bundle), packaging/Info.plist, .github/workflows/ci.yml.

Constraints: bundle id dev.easyvox.EasyVox never changes (macOS ties permissions and the shader cache to it); no secrets in the repo; ad-hoc dev builds keep working.

Design hand-off (11 October): the DMG background and Finder window settings are in design/assets/dmg/ (README.md). Ship the light background.

DMG background staged (11 October, lane C1): packaging/dmg/background.tiff, made with `tiffutil -cathidpicheck` from design/assets/dmg/background.png (660 × 400) and background@2x.png (1320 × 800). Not wired into any script yet. Finder window settings, from design/assets/dmg/README.md:
- Window content size 660 × 400 pt, the same as the picture. Some tools size the window including the title bar: if the bottom is cut off, add about 28 pt to the height.
- Icon view; toolbar, sidebar, path bar and status bar hidden. Icon size 128, text size 13, labels at the bottom, arrange by none.
- Icon centres: EasyVox.app at (170, 164), the Applications alias at (490, 164).
- Background: background.tiff in a hidden .background folder on the volume. Volume name: EasyVox.
- Name the disk image from the crate version (EasyVox-0.1.0.dmg today), not the hand-off's "EasyVox-1.0.dmg".
- create-dmg equivalent: --volname "EasyVox" --background background.tiff --window-pos 200 120 --window-size 660 400 --icon-size 128 --text-size 13 --icon "EasyVox.app" 170 164 --hide-extension "EasyVox.app" --app-drop-link 490 164.
- Only the light picture ships: a DMG background can't follow the appearance. The hand-off says Finder keeps labels black over a background picture even in Dark Mode; docs/branding.md said they turn white. Check by hand on macOS 26, light and dark, before release.

## History

### 2026-10-11 14:56 UTC — edit by `brandon`

description edited

### 2026-10-11 15:36 UTC — edit by `brandon`

description edited
