#!/usr/bin/env bash
# Build EasyVox.app into dist/ and sign it ad hoc. GPUI has no bundler, so this is the bundler.
#
#   scripts/bundle.sh                        shipping build (`dist` profile: thin LTO), ~1.5 min clean
#   scripts/bundle.sh --release              faster day-to-day build (`release` profile, no LTO)
#   scripts/bundle.sh --dev                  debug build, with the "Test dictation" menu item
#   scripts/bundle.sh --features self-test   any other arguments go to `cargo build`
#
# Then: open dist/EasyVox.app
#
# The default and --release builds are signed with the hardened runtime and
# packaging/EasyVox.entitlements, as a shipping build is, so a missing entitlement shows up in
# day-to-day use. --dev builds aren't: the hardened runtime blocks attaching a debugger.
# scripts/release.sh builds on this script for shipping: a Developer ID signature, notarization
# and the DMG (see docs/releasing.md).
#
# MACOSX_DEPLOYMENT_TARGET (26.0, matching Info.plist's LSMinimumSystemVersion) is set in
# .cargo/config.toml, not here, so `cargo build`, the tests, CI and the model bench build the
# same code this script ships.
set -euo pipefail

usage() {
  sed -n '2,9s/^# \{0,1\}//p' "$0"
}

profile=dist
cargo_args=()
for arg in "$@"; do
  case $arg in
    --release) profile=release ;;
    --dev) profile=dev ;;
    -h | --help)
      usage
      exit 0
      ;;
    *) cargo_args+=("$arg") ;;
  esac
done
# Cargo puts the `dev` profile in target/debug and every other profile under its own name.
out_dir=$profile
[[ $profile == dev ]] && out_dir=debug

caller_dir=$PWD
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
if ! command -v cargo >/dev/null && [[ -f $HOME/.cargo/env ]]; then
  # shellcheck source=/dev/null
  . "$HOME/.cargo/env"
fi

app=dist/EasyVox.app
staging=dist/.staging/EasyVox.app

echo "==> cargo build --profile $profile -p easyvox-app ${cargo_args[*]-}"
# `${a[@]+"${a[@]}"}`: an empty array is "unbound" to `set -u` in macOS's bash 3.2.
cargo build --locked --profile "$profile" -p easyvox-app ${cargo_args[@]+"${cargo_args[@]}"}

# Version from the app crate (`cargo pkgid` ends in `#0.1.0` or `#easyvox-app@0.1.0`). macOS
# wants up to three dot-separated numbers, so a pre-release suffix like `-beta.1` is dropped.
pkgid=$(cargo pkgid -p easyvox-app)
version=${pkgid##*[#@]}
version=${version%%[-+]*}
if [[ ! $version =~ ^[0-9]+(\.[0-9]+){0,2}$ ]]; then
  echo "bundle: can't turn the crate version '${pkgid##*[#@]}' into a bundle version" >&2
  exit 1
fi

echo "==> assembling $app (version $version)"
rm -rf dist/.staging
mkdir -p "$staging/Contents/MacOS" "$staging/Contents/Resources"
cp "target/$out_dir/easyvox" "$staging/Contents/MacOS/easyvox"
cp packaging/Info.plist "$staging/Contents/Info.plist"
# The app icon (Info.plist's CFBundleIconFile). Made with the Command Line Tools from the design
# hand-off's Default-appearance PNGs, a renamed copy of design/assets/app-icon/AppIcon.appiconset:
#   iconutil -c icns -o packaging/AppIcon.icns AppIcon.iconset
cp packaging/AppIcon.icns "$staging/Contents/Resources/"
# The About page opens these. The notices come from scripts/notices.sh (see the file's top).
cp packaging/THIRD_PARTY_NOTICES.html "$staging/Contents/Resources/"
cp LICENSE "$staging/Contents/Resources/LICENSE.txt"
plutil -replace CFBundleShortVersionString -string "$version" "$staging/Contents/Info.plist"
plutil -replace CFBundleVersion -string "$version" "$staging/Contents/Info.plist"
plutil -lint "$staging/Contents/Info.plist" >/dev/null

# Ad-hoc signature ("-"): no certificate and no keychain, so nothing can prompt (a keychain
# prompt would hang on a locked screen). Signing the bundle signs its one executable too, so
# --deep (deprecated for signing) isn't needed. macOS ties the Microphone and Accessibility
# grants to the signature, and an ad-hoc one changes with every build, so expect to grant them
# again.
#
# The hardened runtime needs the audio-input entitlement, or macOS shows no Microphone prompt
# and the microphone records silence. Nothing else is needed: Metal compiles shaders in its own
# process, and the binary loads only system libraries.
sign_args=(--force --sign -)
signature="ad hoc"
if [[ $profile != dev ]]; then
  sign_args+=(--options runtime --entitlements packaging/EasyVox.entitlements)
  signature="ad hoc, hardened runtime"
fi
echo "==> signing ($signature)"
codesign "${sign_args[@]}" "$staging"
codesign --verify --strict --deep --verbose=2 "$staging"

rm -rf "$app"
mv "$staging" "$app"
rmdir dist/.staging

binary_kb=$(du -k "$app/Contents/MacOS/easyvox" | cut -f1)
echo "==> built $app: $(du -sh "$app" | cut -f1) (binary $((binary_kb / 1024)) MB), version $version, profile $profile"
where=$app
[[ $caller_dir == "$root" ]] || where="'$root/$app'"
echo "Next: open $where"
if [[ " ${cargo_args[*]-} " == *self-test* ]]; then
  echo "Self-test: open -W $where --args --self-test --out /tmp/easyvox-self-test.json"
fi
