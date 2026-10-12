#!/usr/bin/env bash
# Build the website's fonts (site/public/fonts) from the official releases.
#
#   scripts/site-fonts.sh
#
# Inter 4.1 for text and JetBrains Mono 2.304 for code, both under the SIL Open Font License 1.1.
# Each release zip is checked against its sha256 (the release pages publish none; these match
# Homebrew's font-inter and font-jetbrains-mono casks). The variable fonts are cut to the weights
# the site uses (400 to 700; Inter keeps its opsz axis, so large type gets Inter Display) and
# subset to Latin as WOFF2 with a pinned fontTools. Needs curl, shasum, unzip and uv.
set -euo pipefail

cd "$(dirname "$0")/.."
out=site/public/fonts

inter_url=https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip
inter_sha=9883fdd4a49d4fb66bd8177ba6625ef9a64aa45899767dde3d36aa425756b11e
jbm_url=https://github.com/JetBrains/JetBrainsMono/releases/download/v2.304/JetBrainsMono-2.304.zip
jbm_sha=6f6376c6ed2960ea8a963cd7387ec9d76e3f629125bc33d1fdcd7eb7012f7bbf

instancer=(uvx --from fonttools==4.66.0 --with brotli==1.2.0 fonttools varLib.instancer -q
  --no-recalc-timestamp)
pyftsubset=(uvx --from fonttools==4.66.0 --with brotli==1.2.0 pyftsubset)

# Latin (Google Fonts' "latin" range) plus the symbols the docs use: arrows, minus, ≈ ≠ ≤ ≥,
# and the Mac key symbols ⌃ ⌘ ⌥ ⇧ ⌫ ⎋ ↩ ⏎.
unicodes="U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,\
U+2000-206F,U+20AC,U+2122,U+2190-2193,U+21A9,U+21E7,U+2212,U+2215,U+2248,U+2260,U+2264-2265,\
U+2303,U+2318,U+2325,U+232B,U+238B,U+23CE,U+FEFF,U+FFFD"
subset=(--unicodes="$unicodes" --flavor=woff2 --no-hinting
  --name-IDs=0,1,2,3,4,5,6,13,14 --name-languages=1033)

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fetch() { # url sha256 file
  curl -fsSL -o "$tmp/$3" "$1"
  echo "$2  $tmp/$3" | shasum -a 256 -c --status || { echo "sha256 mismatch: $1" >&2; exit 1; }
}
fetch "$inter_url" "$inter_sha" inter.zip
fetch "$jbm_url" "$jbm_sha" jbm.zip
unzip -q -j "$tmp/inter.zip" InterVariable.ttf LICENSE.txt -d "$tmp/inter"
unzip -q -j "$tmp/jbm.zip" 'fonts/variable/JetBrainsMono?wght?.ttf' OFL.txt -d "$tmp/jbm"

mkdir -p "$out/inter" "$out/jetbrains-mono"

# Inter: tnum stays for the bench tables (font-variant-numeric: tabular-nums).
"${instancer[@]}" "$tmp/inter/InterVariable.ttf" wght=400:700 -o "$tmp/inter.ttf"
"${pyftsubset[@]}" "$tmp/inter.ttf" "${subset[@]}" --layout-features+=tnum \
  --output-file="$out/inter/InterVariable-latin.woff2"
cp "$tmp/inter/LICENSE.txt" "$out/inter/OFL.txt"

# JetBrains Mono without calt, which holds its ligatures: code shows the characters as typed.
"${instancer[@]}" "$tmp/jbm/JetBrainsMono[wght].ttf" wght=400:700 -o "$tmp/jbm.ttf"
"${pyftsubset[@]}" "$tmp/jbm.ttf" "${subset[@]}" --layout-features-=calt \
  --output-file="$out/jetbrains-mono/JetBrainsMono-latin.woff2"
cp "$tmp/jbm/OFL.txt" "$out/jetbrains-mono/OFL.txt"

ls -l "$out"/*/*.woff2
