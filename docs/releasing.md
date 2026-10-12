# Releasing

A release is a disk image, `EasyVox-<version>.dmg`, on the repository's GitHub Releases page. It's signed with a Developer ID and notarized by Apple, so it opens without a Gatekeeper warning. GitHub Actions builds and signs it when the maintainer publishes a release for a new version tag ([`.github/workflows/release.yml`](../.github/workflows/release.yml)).

## How a release is built

Only maintainers can create `v*` tags. The `release` job waits in the `release` environment, which holds the signing secrets, until the maintainer approves the run. Then:

- `scripts/release.sh --check` checks that the tag matches the version in `Cargo.toml`, that the tree is clean and that `CHANGELOG.md` has the version's section, so a mistake fails in seconds.
- It fetches dmgbuild and builds the app with `scripts/bundle.sh`, before any secret is on the runner.
- It imports the Developer ID certificate into a temporary keychain.
- `scripts/release.sh --notarize` signs the app with the hardened runtime and a secure timestamp, then notarizes and staples it.
- It builds the DMG with [dmgbuild](https://github.com/dmgbuild/dmgbuild), then signs, notarizes and staples it.
- It checks both with `stapler` and `spctl`, and the app with `syspolicy_check`.
- The job deletes the keychain and the key files, and keeps the DMG, its notes and the notary service's logs as workflow artifacts.
- A second job, `publish`, which never sees the signing secrets, attaches the DMG to the release. It sets the notes to the version's changelog section and the DMG's SHA-256, and makes it the latest release.

`scripts/release.sh --help` lists the script's options, including an ad-hoc build on a Mac.

## Verifying a download

Check the DMG against the SHA-256 on the release page, then ask Gatekeeper:

```sh
shasum -a 256 ~/Downloads/EasyVox-X.Y.Z.dmg
spctl -a -vvv -t open --context context:primary-signature ~/Downloads/EasyVox-X.Y.Z.dmg
```

`spctl` should print `accepted` and `source=Notarized Developer ID`. After copying the app to Applications:

```sh
codesign --verify --deep --strict --verbose=2 /Applications/EasyVox.app
codesign -dvv /Applications/EasyVox.app 2>&1 | grep -E '^(Authority|TeamIdentifier|Timestamp)='
spctl -a -vvv -t exec /Applications/EasyVox.app
```

The first `Authority` line starts with `Developer ID Application:`, and `TeamIdentifier` is the same for every release.
