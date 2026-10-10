#!/usr/bin/env bash
# Regenerate packaging/THIRD_PARTY_NOTICES.html: the licenses of every crate compiled into
# EasyVox.app, plus the vendored C/C++ (transcribe.cpp, ggml, miniz) and the Lucide icons.
# scripts/bundle.sh copies it into EasyVox.app/Contents/Resources, and the About page opens it.
#
#   scripts/notices.sh
#
# Run it after Cargo.lock changes (a dependency update): crates/easyvox-app/tests/notices.rs fails
# until you do, and CI doesn't need cargo-about for that check. Commit the result.
#
# Needs cargo-about 0.9 (`cargo install --locked cargo-about --features cli`). Works offline,
# from the crates in ~/.cargo/registry (after any build, or `cargo fetch`).
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
if ! command -v cargo >/dev/null && [[ -f $HOME/.cargo/env ]]; then
  # shellcheck source=/dev/null
  . "$HOME/.cargo/env"
fi
if ! cargo about --version >/dev/null 2>&1; then
  echo "notices: needs cargo-about: cargo install --locked cargo-about --features cli" >&2
  exit 1
fi

out=packaging/THIRD_PARTY_NOTICES.html
tmp=$(mktemp "$out.XXXXXX")
trap 'rm -f "$tmp"' EXIT

echo "==> $(cargo about --version): generating $out"
cargo about generate --locked --offline --fail \
  -c packaging/about.toml -m crates/easyvox-app/Cargo.toml -o "$tmp" packaging/about.hbs

# Every crate in Cargo.lock is either listed in the notices or named here as not in the app (build
# tools, tests, easyvox-bench, other platforms). The test checks that split, so a new or updated
# crate can't go unnoticed.
listed=$(grep -o 'data-crate="[^"]*"' "$tmp" | sed 's/^data-crate="//; s/"$//' | sort -u)
locked=$(awk '
  function flush() { if (name != "" && source) print name "@" version; name = ""; version = ""; source = 0 }
  /^\[\[package\]\]/ { flush(); next }
  /^name = /    { name = $3; gsub(/"/, "", name) }
  /^version = / { version = $3; gsub(/"/, "", version) }
  /^source = /  { source = 1 }
  END { flush() }
' Cargo.lock | sort -u)
not_shipped=$(comm -23 <(printf '%s\n' "$locked") <(printf '%s\n' "$listed"))
{
  echo "<!--"
  echo "  In Cargo.lock but not in the app (build tools, tests, easyvox-bench, other platforms), so"
  echo "  not listed above. crates/easyvox-app/tests/notices.rs checks that every crate in Cargo.lock"
  echo "  is listed above or here."
  printf '%s\n' "$not_shipped" | sed 's/^/  not-shipped: /'
  echo "-->"
} >>"$tmp"

chmod 644 "$tmp"
mv "$tmp" "$out"
trap - EXIT
printf '==> %s: %s crates in the app, %s others in Cargo.lock\n' "$out" \
  "$(printf '%s\n' "$listed" | grep -c .)" "$(printf '%s\n' "$not_shipped" | grep -c .)"
