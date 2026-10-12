#!/usr/bin/env bash
# Build, sign and package a release of EasyVox: dist/EasyVox-<version>.dmg.
#
#   scripts/release.sh --allow-dirty    ad-hoc rehearsal: every step except notarization
#   scripts/release.sh --identity <name or SHA-1> --keychain <path> --tag v<version> --notarize
#                                       what .github/workflows/release.yml runs
#
#   --identity <id>    codesign identity, by name or SHA-1. Default "-": ad hoc
#   --keychain <path>  look for the identity only in this keychain
#   --tag <tag>        must be "v" + the Cargo.toml version and point at HEAD, and CHANGELOG.md
#                      must have a section for that version. Required with a real identity
#   --notarize         notarize and staple the app, then the DMG. Needs a real identity, and
#                      an App Store Connect API key from the environment, never from arguments:
#                      NOTARY_API_KEY_PATH (the .p8 file), NOTARY_API_KEY_ID, NOTARY_API_ISSUER_ID
#   --check            run the preflight checks only, and stop before the build
#   --allow-dirty      build from a tree with uncommitted changes (ad hoc only)
#
# Writes dist/EasyVox.app and the DMG, and dist/release-notes.md (the version's CHANGELOG.md
# section and the DMG's SHA-256). When notarizing, it also writes the notary service's logs in
# dist/notary/ and dist/release.env (version, tag, dmg, sha256, url and notes, as key=value
# lines), and prints the Homebrew cask's values. docs/releasing.md describes how a release is built.
set -euo pipefail

usage() {
  sed -n '2,16s/^# \{0,1\}//p' "$0"
}

die() {
  echo "release: $*" >&2
  exit 1
}

step() {
  echo "==> $*"
}

identity=-
keychain=
tag=
notarize=0
check=0
allow_dirty=0
while (($#)); do
  case $1 in
    --identity | --keychain | --tag)
      (($# >= 2)) || die "$1 needs a value"
      case $1 in
        --identity) identity=$2 ;;
        --keychain) keychain=$2 ;;
        --tag) tag=$2 ;;
      esac
      shift 2
      ;;
    --notarize)
      notarize=1
      shift
      ;;
    --check)
      check=1
      shift
      ;;
    --allow-dirty)
      allow_dirty=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      die "unknown argument: $1"
      ;;
  esac
done

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
if ! command -v cargo >/dev/null && [[ -f $HOME/.cargo/env ]]; then
  # shellcheck source=/dev/null
  . "$HOME/.cargo/env"
fi

app=dist/EasyVox.app
binary=$app/Contents/MacOS/easyvox
entitlements=packaging/EasyVox.entitlements
adhoc=0
[[ $identity == - ]] && adhoc=1

# dmgbuild and the two packages it depends on, pinned. uvx fetches them into its cache. The release
# workflow fetches the same pins before the signing key arrives, then runs this offline: change
# both together.
dmgbuild=(uvx --python 3.12 --with ds_store==1.3.3 --with mac_alias==2.2.3 dmgbuild==1.6.7)

# Prints a CHANGELOG.md section's body: the lines under "## <name>" or "## [<name>]" (anything
# after the name, like a date, is ignored), up to the next "## " heading, without the blank lines
# around it or link definitions.
changelog_section() {
  awk -v name="$1" '
    /^## / {
      if (found) exit
      h = substr($0, 4)
      sub(/^\[/, "", h)
      sub(/[] ].*$/, "", h)
      found = (h == name)
      next
    }
    !found || /^\[[^]]+\]: / { next }
    /^[[:space:]]*$/ { if (started) blanks++; next }
    {
      for (; blanks > 0; blanks--) print ""
      print
      started = 1
    }
  ' CHANGELOG.md
}

# --- Preflight: everything that can fail before the build --------------------------------------

step "preflight"
if ((adhoc)); then
  ((!notarize)) || die "--notarize needs a real --identity: Apple doesn't notarize ad-hoc code"
  [[ -z $keychain ]] || die "--keychain needs a real --identity"
else
  ((!allow_dirty)) || die "--allow-dirty is for ad-hoc rehearsals only"
  [[ -n $tag ]] || die "a real --identity needs --tag v<version>"
fi

tools=(git cargo codesign plutil otool lipo shasum hdiutil uvx awk jq syspolicy_check)
((notarize)) && tools+=(ditto xcrun spctl)
for tool in "${tools[@]}"; do
  command -v "$tool" >/dev/null || die "missing tool: $tool"
done
if ((notarize)); then
  for tool in notarytool stapler; do
    xcrun --find "$tool" >/dev/null 2>&1 || die "missing tool: xcrun $tool"
  done
  for var in NOTARY_API_KEY_PATH NOTARY_API_KEY_ID NOTARY_API_ISSUER_ID; do
    [[ -n ${!var:-} ]] || die "--notarize needs $var in the environment"
  done
  [[ -f $NOTARY_API_KEY_PATH ]] || die "NOTARY_API_KEY_PATH: no file at $NOTARY_API_KEY_PATH"
fi
[[ -z $keychain || -f $keychain ]] || die "no keychain at $keychain"

if [[ -n $(git status --porcelain) ]]; then
  if ((allow_dirty)); then
    echo "    the tree has uncommitted changes (--allow-dirty)"
  else
    git status --short >&2
    die "the tree has uncommitted changes (an ad-hoc rehearsal can pass --allow-dirty)"
  fi
fi

pkgid=$(cargo pkgid --locked -p easyvox-app)
version=${pkgid##*[#@]}
# macOS bundle versions are numbers only, so a release is X.Y.Z with no pre-release suffix.
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "the crate version '$version' isn't X.Y.Z"
if [[ -n $tag ]]; then
  [[ $tag == "v$version" ]] || die "tag $tag doesn't match the Cargo.toml version (v$version)"
  tag_commit=$(git rev-parse -q --verify "refs/tags/$tag^{commit}") || die "no tag $tag here"
  [[ $tag_commit == "$(git rev-parse HEAD)" ]] || die "tag $tag isn't on HEAD"
fi

notes=$(changelog_section "$version")
if [[ -z $notes ]]; then
  [[ -z $tag ]] || die "CHANGELOG.md has no section for $version (\"## $version\" or \"## [$version]\")"
  notes=$(changelog_section Unreleased)
  [[ -n $notes ]] || die "CHANGELOG.md has no section for $version and nothing under Unreleased"
  echo "    CHANGELOG.md has no $version section yet: the notes come from Unreleased"
fi

repo_url=$(sed -n 's/^repository = "\(.*\)"$/\1/p' Cargo.toml)
[[ $repo_url == https://github.com/* ]] || die "Cargo.toml's repository isn't a GitHub URL"

[[ $(head -c 5 "$entitlements") == "<?xml" ]] || die "$entitlements must be an XML plist"
plutil -lint -s "$entitlements" || die "$entitlements doesn't parse"
! grep -q get-task-allow "$entitlements" || die "$entitlements has get-task-allow"
min_macos=$(plutil -extract LSMinimumSystemVersion raw -o - packaging/Info.plist)

mode="identity $identity"
((!notarize)) || mode+=", notarizing"
echo "    version $version${tag:+, tag $tag}, $mode"
if ((check)); then
  echo "    preflight passed (--check: no build)"
  exit 0
fi

# --- Build --------------------------------------------------------------------------------------

# release.env describes a notarized DMG only: remove an earlier one before making another DMG.
rm -f dist/release.env
scripts/bundle.sh

step "checking the binary"
archs=$(lipo -archs "$binary")
[[ $archs == arm64 ]] || die "expected an arm64 binary, got: $archs"
build_version=$(otool -l "$binary" | awk '/cmd LC_BUILD_VERSION/ {f = 1} f && $1 == "minos" {m = $2} f && $1 == "sdk" {print m, $2; exit}')
read -r minos sdk <<<"$build_version"
[[ $minos == "$min_macos" ]] || die "the binary's minos is ${minos:-missing}, but Info.plist says $min_macos"
echo "    arm64, minos $minos, sdk $sdk"

# --- Sign and verify the app --------------------------------------------------------------------

# bundle.sh signed ad hoc; this signs again with the release identity. --timestamp adds Apple's
# secure timestamp, which notarization requires and which keeps the signature valid after the
# certificate expires. It needs a real identity and the network.
step "signing $app (identity $identity)"
sign_args=(--force --options runtime --entitlements "$entitlements" --sign "$identity")
((adhoc)) || sign_args+=(--timestamp)
[[ -z $keychain ]] || sign_args+=(--keychain "$keychain")
codesign "${sign_args[@]}" "$app"

step "verifying the signature"
codesign --verify --deep --strict --verbose=2 "$app"
details=$(codesign -dvv "$app" 2>&1)
grep -Eq '^CodeDirectory .*flags=0x[0-9a-f]+\([^)]*runtime' <<<"$details" || die "the signature has no hardened runtime"
if ((!adhoc)); then
  grep -q '^Authority=Developer ID Application: ' <<<"$details" || die "the identity isn't a Developer ID Application certificate"
  grep -q '^Timestamp=' <<<"$details" || die "the signature has no secure timestamp"
fi
grep -E '^(CodeDirectory|Authority|TeamIdentifier|Timestamp)' <<<"$details" | sed 's/^/    /'
signed_entitlements=$(codesign -d --entitlements - --xml "$app" 2>/dev/null)
grep -q com.apple.security.device.audio-input <<<"$signed_entitlements" || die "the audio-input entitlement is missing"
! grep -q get-task-allow <<<"$signed_entitlements" || die "the signature has get-task-allow"

# Apple's syspolicy_check on the app. It exits non-zero for any finding, warnings included, so
# this prints every finding and stops only for one above Warning level. An ad-hoc signature is
# always one Warning, "Adhoc Signed App".
#
# One Fatal finding is let through: "Internal Xprotect Error", which GitHub's macos-26 runner
# reported for the first 0.1.0 run while the same check passed on a Mac. It's XProtect failing
# on the machine, and the notary service runs Apple's own malware scan on every upload.
syspolicy() { # <mode>
  local mode=$1 report
  step "syspolicy_check $mode"
  if report=$(syspolicy_check "$mode" "$app" --json 2>/dev/null); then
    echo "    passed"
    return
  fi
  jq -r '.output[] | "    \(.SyspolicyCheckErrorLevel): \(.SyspolicyCheckShortError): \(.SyspolicyCheckLongError)"' \
    <<<"$report" 2>/dev/null || printf '%s\n' "$report" | sed 's/^/    /'
  jq -e '(.output | length) > 0 and all(.output[];
      .SyspolicyCheckErrorLevel == "Warning" or .SyspolicyCheckShortError == "Internal Xprotect Error")' \
    <<<"$report" >/dev/null 2>&1 || die "syspolicy_check $mode rejected the app"
  ! jq -e 'any(.output[]; .SyspolicyCheckShortError == "Internal Xprotect Error")' <<<"$report" \
    >/dev/null 2>&1 || echo "    XProtect failed on this machine; the notary service scans the upload"
}

# Apple's pre-notarization check, run locally, so a problem stops the release before an upload.
syspolicy notary-submission

# --- Notarize and staple the app ----------------------------------------------------------------

# Submits a file, waits, and keeps the service's log in dist/notary/<name>-log.json, accepted or
# not: the log lists every issue, warnings included. Most submissions take minutes, but a new
# team's first ones can stay "In Progress" for hours.
notarize_file() {
  local file=$1 name=$2 result=dist/notary/$2-submit.json id status
  step "notarizing $file (waits up to 2 hours)"
  xcrun notarytool submit "$file" \
    --key "$NOTARY_API_KEY_PATH" --key-id "$NOTARY_API_KEY_ID" --issuer "$NOTARY_API_ISSUER_ID" \
    --wait --timeout 2h --output-format json >"$result" || true
  id=$(jq -r '.id // empty' "$result" 2>/dev/null || true)
  status=$(jq -r '.status // empty' "$result" 2>/dev/null || true)
  if [[ -z $id ]]; then
    cat "$result" >&2
    die "notarytool submit failed for $file"
  fi
  xcrun notarytool log "$id" \
    --key "$NOTARY_API_KEY_PATH" --key-id "$NOTARY_API_KEY_ID" --issuer "$NOTARY_API_ISSUER_ID" \
    "dist/notary/$name-log.json" || echo "    couldn't fetch the log for submission $id" >&2
  echo "    submission $id: ${status:-no status}"
  jq -r '.issues // [] | .[] | "    \(.severity): \(.path): \(.message)"' \
    "dist/notary/$name-log.json" 2>/dev/null || true
  [[ $status == Accepted ]] || die "notarizing $file ended '${status:-no status}' (dist/notary/$name-log.json)"
}

# Attaches the notarization ticket. Stapling fetches it from Apple's servers, which sometimes fail
# right after an Accepted verdict, so this tries three times.
staple() { # <path>
  step "stapling $1"
  for _ in 1 2 3; do
    xcrun stapler staple "$1" && return
    sleep 20
  done
  die "couldn't staple $1"
}

if ((notarize)); then
  rm -rf dist/notary
  mkdir -p dist/notary
  zip=dist/EasyVox-notarize.zip
  ditto -c -k --keepParent "$app" "$zip"
  notarize_file "$zip" app
  rm -f "$zip"
  staple "$app"
fi

# --- The DMG ------------------------------------------------------------------------------------

dmg=dist/EasyVox-$version.dmg
step "building $dmg"
rm -f "$dmg"
# --no-hidpi: background.tiff already holds both resolutions.
"${dmgbuild[@]}" --no-hidpi -s packaging/dmg/settings.py -D app="$app" EasyVox "$dmg"

if ((!adhoc)); then
  step "signing $dmg"
  dmg_sign_args=(--sign "$identity" --timestamp)
  [[ -z $keychain ]] || dmg_sign_args+=(--keychain "$keychain")
  codesign "${dmg_sign_args[@]}" "$dmg"
  codesign --verify --strict --verbose=2 "$dmg"
fi
if ((notarize)); then
  notarize_file "$dmg" dmg
  staple "$dmg"
fi

# What a downloader gets: the same signed (and stapled) app, the Applications link, the
# background and the window layout.
step "checking the DMG's contents"
# A process can keep the volume busy for a moment: force the detach if a plain one fails.
detach() { hdiutil detach -quiet "$1" || hdiutil detach -quiet -force "$1"; }
mnt=$(mktemp -d "${TMPDIR:-/tmp}/easyvox-dmg.XXXXXX")
trap 'detach "$mnt" 2>/dev/null || true; rmdir "$mnt" 2>/dev/null || true' EXIT
hdiutil attach -nobrowse -readonly -noautoopen -mountpoint "$mnt" "$dmg" >/dev/null
for item in EasyVox.app .background.tiff .DS_Store; do
  [[ -e $mnt/$item ]] || die "the DMG has no $item"
done
[[ $(readlink "$mnt/Applications") == /Applications ]] || die "the DMG's Applications link is missing or wrong"
cmp -s "$mnt/EasyVox.app/Contents/MacOS/easyvox" "$binary" || die "the DMG's app differs from $app"
codesign --verify --deep --strict "$mnt/EasyVox.app"
((!notarize)) || xcrun stapler validate -q "$mnt/EasyVox.app"
detach "$mnt"
rmdir "$mnt"
trap - EXIT
echo "    EasyVox.app, Applications -> /Applications, .background.tiff, .DS_Store"

# --- Gatekeeper ---------------------------------------------------------------------------------

if ((notarize)); then
  step "Gatekeeper checks"
  xcrun stapler validate "$app"
  xcrun stapler validate "$dmg"
  gatekeeper() { # <path> <spctl arguments...>
    local path=$1 verdict
    shift
    verdict=$(spctl -a -vvv "$@" "$path" 2>&1) || {
      echo "$verdict" >&2
      die "spctl rejected $path"
    }
    printf '%s\n' "$verdict" | sed 's/^/    /'
    grep -q 'source=Notarized Developer ID' <<<"$verdict" || die "spctl: $path isn't a notarized Developer ID"
  }
  gatekeeper "$app" -t exec
  gatekeeper "$dmg" -t open --context context:primary-signature
  syspolicy distribution
else
  step "skipping the Gatekeeper checks (stapler validate, spctl, syspolicy_check)"
  echo "    They need a notarized Developer ID signature and always reject this build."
fi

# --- Outputs ------------------------------------------------------------------------------------

sha256=$(shasum -a 256 "$dmg" | cut -d ' ' -f 1)
# shellcheck disable=SC2016 # the backticks are Markdown
printf '%s\n\nSHA-256 of `%s`: `%s`\n' "$notes" "${dmg##*/}" "$sha256" >dist/release-notes.md
step "done: $dmg ($(du -h "$dmg" | cut -f 1 | tr -d ' '))"
echo "    notes  dist/release-notes.md"

# The cask's values come only from a notarized DMG, so a rehearsal's SHA-256 can't end up there.
if ((notarize)); then
  url=$repo_url/releases/download/v$version/${dmg##*/}
  cat >dist/release.env <<EOF
version=$version
tag=v$version
dmg=$dmg
sha256=$sha256
url=$url
notes=dist/release-notes.md
EOF
  cat <<EOF
    values in dist/release.env

    For the Homebrew cask:
      version "$version"
      sha256 "$sha256"
      url "$url"
EOF
else
  echo "    Not notarized: Gatekeeper blocks this DMG on any other Mac. Don't publish it."
fi
