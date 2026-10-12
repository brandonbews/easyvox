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

12 Oct: agents use the GitHub account iamagentbox, a collaborator on brandonbews/easyvox with write access and no admin. So only Brandon can create the release environment, add its secrets, and approve a run. Also planned: a tag ruleset so only Brandon (admin) can create v* tags; restrict the environment to v* tags. Brandon gets a runbook with exact steps for the CSR, the certificate (G2 Sub-CA), the .p12, the API key and the secrets.

## History

### 2026-10-12 06:44 UTC — edit by `brandon`

description edited
