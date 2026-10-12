# Releasing

A release is a disk image, `EasyVox-<version>.dmg`, on the repository's GitHub Releases page. It's signed with a Developer ID and notarized by Apple, so it opens without a Gatekeeper warning. GitHub Actions builds and signs it when the maintainer publishes a release for a new version tag ([`.github/workflows/release.yml`](../.github/workflows/release.yml)).

## Making a release

1. **The release commit.**
   - Set the version in `Cargo.toml` (`[workspace.package] version`). It's `X.Y.Z`, with no pre-release suffix: macOS bundle versions are numbers only.
   - Run `cargo update --workspace`, so `Cargo.lock` has the new version.
   - In `CHANGELOG.md`, rename `## Unreleased` to `## X.Y.Z - <date>` and add an empty `## Unreleased` above it.
   - Commit as "Release X.Y.Z" and push it to `main`.
2. **The release.** The maintainer publishes it on GitHub, which needs no git setup:
   - **Releases → Draft a new release**;
   - under **Choose a tag**, type `vX.Y.Z` and click **Create new tag**, with `main` as the target;
   - tick **This is a pre-release**, so it isn't the latest release while it has no DMG;
   - click **Publish release**.

   Publishing creates the tag and starts the workflow. A draft starts nothing, since its tag doesn't exist yet. A ruleset lets only maintainers create, move or delete `v*` tags.
3. **The approval.** The `release` job waits in the `release` environment, which holds the signing secrets, until the maintainer approves it. Before approving, read the changes since the last release: `.github/`, `scripts/`, any `build.rs` and `Cargo.lock` above all. The run signs whatever the tag holds.
4. **The run.**
   - `scripts/release.sh --check` checks that the tag matches the version, that the tree is clean and that `CHANGELOG.md` has the version's section, so a mistake fails in seconds.
   - It fetches dmgbuild and builds the app with `scripts/bundle.sh`, before any secret is on the runner.
   - It imports the Developer ID certificate into a temporary keychain.
   - `scripts/release.sh --notarize` signs the app with the hardened runtime and a secure timestamp, then notarizes and staples it.
   - It builds the DMG with [dmgbuild](https://github.com/dmgbuild/dmgbuild), then signs, notarizes and staples it.
   - It checks both with `stapler` and `spctl`, and the app with `syspolicy_check`.
   - The job deletes the keychain and the key files, and keeps the DMG, its notes and the notary service's logs as workflow artifacts.
   - A second job, `publish`, which never sees the signing secrets, attaches the DMG to the release. It sets the notes to the version's changelog section and the DMG's SHA-256, and makes it the latest release.

If a run fails, the release stays a pre-release with no DMG. Re-run it once the cause is fixed. If the fix needs a commit, delete the release and its tag, and publish a new release once the fix is on `main`.

## Rehearsing

```sh
scripts/release.sh --allow-dirty
```

This runs the same script on a Mac, signed ad hoc: the checks, the build, the hardened-runtime signature and its checks, and the DMG. It skips notarization and the Gatekeeper checks, which need the Developer ID. It needs [uv](https://docs.astral.sh/uv/) for dmgbuild. Gatekeeper blocks an ad-hoc DMG on any other Mac, so a rehearsal's DMG is never published, and only a notarized run prints the values for the Homebrew cask. `--check` runs the checks alone, without the build. `scripts/release.sh --help` lists the options.

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
