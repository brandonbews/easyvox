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

## History

_No activity yet._
