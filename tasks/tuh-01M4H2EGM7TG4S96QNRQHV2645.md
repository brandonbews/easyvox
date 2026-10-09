# Create a Developer ID Application certificate and a notarization key

`tuh-01M4H2EGM7TG4S96QNRQHV2645`

- **Status:** on hold — deliberately paused
- **Priority:** 3
- **Labels:** `human` `release`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Brandon's task; agents release it. Mark it done when both exist; the shipping-build task waits on it.

- Developer ID Application certificate: only the Apple Developer account holder can create one. The existing Apple Distribution certificate can't sign apps distributed outside the App Store.
- An App Store Connect API key for notarization.
- Store the exported certificate (.p12 and password) and the API key as GitHub Actions secrets; agree the names with the shipping-build task.

Pointers: DESIGN.md "Signing and notarization (macOS)".

## History

_No activity yet._
