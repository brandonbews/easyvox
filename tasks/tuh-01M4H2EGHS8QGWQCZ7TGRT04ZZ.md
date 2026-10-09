# Check, and likely set, MACOSX_DEPLOYMENT_TARGET=26.0 in .cargo/config.toml

`tuh-01M4H2EGHS8QGWQCZ7TGRT04ZZ`

- **Status:** open — ready
- **Priority:** 3
- **Labels:** `build`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: EasyVox requires macOS 26.0 (packaging/Info.plist LSMinimumSystemVersion 26.0; DESIGN.md "Platform and distribution"). Without MACOSX_DEPLOYMENT_TARGET, rustc, cc and cmake (which builds transcribe.cpp) target an older default, so the binary's minimum OS can disagree with the Info.plist.

The ask: check the built binary's minos today (`vtool -show-build` or `otool -l | grep -A3 LC_BUILD_VERSION`) and any deployment-target warnings, then decide whether `[env] MACOSX_DEPLOYMENT_TARGET = "26.0"` in .cargo/config.toml helps (and what it does to cmake flags and build caching). Set it if it does.

Acceptance: the binary's minos matches LSMinimumSystemVersion, or the task records why it doesn't matter; no deployment-target warnings; CI green.

Pointers: packaging/Info.plist, scripts/bundle.sh, rust-toolchain.toml, .github/workflows/ci.yml.

## History

_No activity yet._
