#!/usr/bin/env bash
# Download a speech model into the folder EasyVox loads models from, and check its sha256.
#
#   scripts/fetch-model.sh              the default model
#   scripts/fetch-model.sh <model-id>   another model from the list
#   scripts/fetch-model.sh --list       the models, their sizes and where the app offers them
#
# The models folder is the app's (crates/easyvox-core/src/paths.rs):
#   $EASYVOX_MODELS_DIR, else $EASYVOX_DATA_DIR/models,
#   else ~/Library/Application Support/EasyVox/models on macOS.
#
# A model that's already there and passes the check isn't downloaded again. A download goes to
# a hidden temporary file next to the model and only takes the model's name once its size and
# sha256 match, so an interrupted or bad download never looks like a model. Uses only curl and
# shasum, which macOS ships with.
set -euo pipefail

# The transcribe.cpp entries of the catalog (crates/easyvox-core/src/models.rs): GGUF files, plus
# one whisper.cpp .bin that transcribe.cpp also loads. One model per line:
#   id  offering  bytes  sha256  path (in the models folder)  url
# crates/easyvox-core/tests/fetch_script.rs checks this table against the catalog, and the one
# `default` line against easyvox_core::RECOMMENDED_MODEL. The sherpa-onnx archives (bench only)
# aren't covered here.
MODELS=(
  "parakeet-tdt-0.6b-v2-q5_k_m-gguf default 539012608 dfa904520b95451599683613fea47766c378c5fdf5d8cddf48151226b6eaec85 gguf/parakeet-tdt-0.6b-v2-Q5_K_M.gguf https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v2-gguf/resolve/382796e405a9fe37145938dccbabfb25e93e4286/parakeet-tdt-0.6b-v2-Q5_K_M.gguf"
  "whisper-base.en-q8_0-gguf alternative 84886208 3b46ca40bccbf7609c68d88a36d96077a04ca7c87f2060ede06f129fac3e7652 gguf/whisper-base.en-Q8_0.gguf https://huggingface.co/handy-computer/whisper-base.en-gguf/resolve/f83d8d187299d4fc06e86a6de5084375ceac34bf/whisper-base.en-Q8_0.gguf"
  "parakeet-unified-en-0.6b-q5_k_m-gguf bench-only 540795264 f9def6f9b4e83ab7d006df3e1b676dfa1f973a3b6da232a9c99fcaa66bcd2836 gguf/parakeet-unified-en-0.6b-Q5_K_M.gguf https://huggingface.co/handy-computer/parakeet-unified-en-0.6b-gguf/resolve/d5249700b2382bf5c5024c2421d101b8db54a629/parakeet-unified-en-0.6b-Q5_K_M.gguf"
  "canary-180m-flash-q5_k_m-gguf bench-only 158704416 73c4861a43fe848cc7a64c179c08da04e985efac03f3e4684a3f5e94a1bb33fe gguf/canary-180m-flash-Q5_K_M.gguf https://huggingface.co/handy-computer/canary-180m-flash-gguf/resolve/46b86384d1db59108d6e060a0fc00924cb58499f/canary-180m-flash-Q5_K_M.gguf"
  "parakeet-tdt-0.6b-v3-q5_k_m-gguf bench-only 548946272 cc722e76adc1a629fc0b2535de879d99b8160d07ad4c0215e2ca7d7ea0ae4b8f gguf/parakeet-tdt-0.6b-v3-Q5_K_M.gguf https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v3-gguf/resolve/90f082450fcbacdb54e5900c44ef697c9ea59622/parakeet-tdt-0.6b-v3-Q5_K_M.gguf"
  "whisper-large-v3-turbo-q5_k_m-gguf bench-only 619628128 977b5db4e004349dffd1ab9caa10ba5aaba3fc3edd3ba72cadb84328a3203e36 gguf/whisper-large-v3-turbo-Q5_K_M.gguf https://huggingface.co/handy-computer/whisper-large-v3-turbo-gguf/resolve/ceea6c8a94a21ab85be244d311e874a39344dbf5/whisper-large-v3-turbo-Q5_K_M.gguf"
  "qwen3-asr-1.7b-q5_k_m-gguf bench-only 1517290464 034c557fe92ff8fcd9a9c041cbdaad347be0a86a58d3a348f63cf3f0180879d0 gguf/Qwen3-ASR-1.7B-Q5_K_M.gguf https://huggingface.co/handy-computer/Qwen3-ASR-1.7B-gguf/resolve/3555bd238a8572bbace3ebf60d23b036dc0a5dbe/Qwen3-ASR-1.7B-Q5_K_M.gguf"
  "distil-whisper-large-v3.5-f16-ggml bench-only 1519521155 ec2498919b498c5f6b00041adb45650124b3cd9f26f545fffa8f5d11c28dcf26 ggml/distil-large-v3.5/ggml-model.bin https://huggingface.co/distil-whisper/distil-large-v3.5-ggml/resolve/960ecb5c2ecfba3ebb9ebe485c1032ec266cf436/ggml-model.bin"
  "moonshine-streaming-medium-q8_0-gguf bench-only 295793568 f7c9564249b508f6012927ec4f9e536087da53a7047f858ca9975bea5f75299e gguf/moonshine-streaming-medium-Q8_0.gguf https://huggingface.co/handy-computer/moonshine-streaming-medium-gguf/resolve/0f99e956a9e63d591ddd7f2a20dfead255e96c68/moonshine-streaming-medium-Q8_0.gguf"
  "granite-speech-5.0-470m-turboctc-q5_k_m-gguf bench-only 335737184 a18c6882ff9dbcefbe674ccb8f8653578ed7dbc3fa9a955f036f3617b9627cce gguf/granite-speech-5.0-470m-turboctc-Q5_K_M.gguf https://huggingface.co/handy-computer/granite-speech-5.0-470m-turboctc-gguf/resolve/2b0b44d0d6c8b94a2e4f04a2cf3ec6a5c054463b/granite-speech-5.0-470m-turboctc-Q5_K_M.gguf"
  "nemotron-speech-streaming-en-0.6b-q5_k_m-gguf bench-only 538989568 d6b8b36f9aca6a751779d064a29f6583f3e82c0c15646904383c8436a25c1489 gguf/nemotron-speech-streaming-en-0.6b-Q5_K_M.gguf https://huggingface.co/handy-computer/nemotron-speech-streaming-en-0.6b-gguf/resolve/9789e0ebf77277911272f0d9a35e1646b5aa6004/nemotron-speech-streaming-en-0.6b-Q5_K_M.gguf"
  "cohere-transcribe-03-2026-q5_k_m-gguf bench-only 1770270208 14d02f1ad6dd77b3a60f82639879012c3adb4fe25c50a5a47a2c4c661daf1558 gguf/cohere-transcribe-03-2026-Q5_K_M.gguf https://huggingface.co/handy-computer/cohere-transcribe-03-2026-gguf/resolve/ab667acedcb5d8d56837ef3ed3568c3ad50dde47/cohere-transcribe-03-2026-Q5_K_M.gguf"
  "canary-qwen-2.5b-q5_k_m-gguf bench-only 1983729024 d71b1d9ae442dc538665ed4df63a69520eb9be769ec0bc78c7d2f40040928306 gguf/canary-qwen-2.5b-Q5_K_M.gguf https://huggingface.co/handy-computer/canary-qwen-2.5b-gguf/resolve/df576c1641eb59bb66bc3c396bbdd8b0b113825a/canary-qwen-2.5b-Q5_K_M.gguf"
  "granite-speech-4.1-2b-q5_k_m-gguf bench-only 1829704640 1ea15c2174b485b8de81337a1da3ae6703a7892a5330345b9a2212e9a6a0c365 gguf/granite-speech-4.1-2b-Q5_K_M.gguf https://huggingface.co/handy-computer/granite-speech-4.1-2b-gguf/resolve/5fa89dfac74d52a186f63647553b9e1f7ae36e83/granite-speech-4.1-2b-Q5_K_M.gguf"
)

usage() {
  cat <<'EOF'
Usage: scripts/fetch-model.sh [model-id]
       scripts/fetch-model.sh --list

Downloads a speech model (the default one when no id is given) into EasyVox's models folder
and checks its sha256. Set EASYVOX_MODELS_DIR to use another folder.
EOF
}

die() {
  printf 'fetch-model: %s\n' "$*" >&2
  exit 1
}

# `a/b` like Rust's Path::join, where an empty `a` means the current directory.
join_path() {
  printf '%s' "${1:+$1/}$2"
}

# Mirrors easyvox_core::paths::models_dir(). An override counts when it's set, even if empty.
models_dir() {
  if [[ -n ${EASYVOX_MODELS_DIR+set} ]]; then
    printf '%s' "$EASYVOX_MODELS_DIR"
    return
  fi
  local data
  if [[ -n ${EASYVOX_DATA_DIR+set} ]]; then
    data=$EASYVOX_DATA_DIR
  elif [[ $(uname -s) == Darwin ]]; then
    data=$(join_path "${HOME-}" "Library/Application Support/EasyVox")
  elif [[ ${XDG_DATA_HOME-} == /* ]]; then
    data="$XDG_DATA_HOME/easyvox"
  else
    data=$(join_path "${HOME-}" ".local/share/easyvox")
  fi
  join_path "$data" models
}

# 540795264 -> "541 MB" (decimal, like Finder).
megabytes() {
  awk -v b="$1" 'BEGIN { printf "%.0f MB", b / 1000000 }'
}

# Sets id, offering, bytes, sha256, path and url from the table, or fails.
lookup() {
  local line
  for line in "${MODELS[@]}"; do
    read -r id offering bytes sha256 path url <<<"$line"
    [[ $id == "$1" ]] && return 0
  done
  return 1
}

default_id() {
  local line
  for line in "${MODELS[@]}"; do
    read -r id offering _ <<<"$line"
    if [[ $offering == default ]]; then
      printf '%s' "$id"
      return
    fi
  done
  die "the model table has no default entry"
}

# Empty when the file has the expected size and sha256, otherwise the reason it doesn't.
check_file() {
  local file=$1 size actual
  size=$(wc -c <"$file" | tr -d ' ')
  if [[ $size != "$bytes" ]]; then
    printf 'it is %s bytes, expected %s' "$size" "$bytes"
    return
  fi
  actual=$(shasum -a 256 "$file" | awk '{ print $1 }')
  if [[ $actual != "$sha256" ]]; then
    printf 'its sha256 is %s, expected %s' "$actual" "$sha256"
  fi
}

list_models() {
  local dir line note
  dir=$(models_dir)
  printf 'Models (folder: %s)\n\n' "${dir:-.}"
  for line in "${MODELS[@]}"; do
    read -r id offering bytes sha256 path url <<<"$line"
    note=""
    [[ -e $(join_path "$dir" "$path") ]] && note="downloaded"
    printf '  %-38s %7s  %-11s  %s\n' "$id" "$(megabytes "$bytes")" "$offering" "$note"
  done
  printf "\ndefault: the app's model. alternative: offered in Settings. bench-only: for easyvox-bench.\n"
}

tmp=""
cleanup() {
  if [[ -n $tmp ]]; then
    rm -f "$tmp"
  fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

fetch() {
  local want=$1 dir dest folder file problem started
  if ! lookup "$want"; then
    printf 'fetch-model: unknown model id "%s". Known ids:\n' "$want" >&2
    local line
    for line in "${MODELS[@]}"; do
      printf '  %s\n' "${line%% *}" >&2
    done
    exit 1
  fi
  dir=$(models_dir)
  dest=$(join_path "$dir" "$path")
  folder=$(dirname "$dest")
  file=$(basename "$dest")

  if [[ -e $dest ]]; then
    printf 'Checking the copy of %s that is already in %s ...\n' "$id" "$folder"
    problem=$(check_file "$dest")
    if [[ -z $problem ]]; then
      printf 'Already downloaded and verified: %s\n' "$dest"
      return
    fi
    printf 'That copy is damaged (%s). Deleting it and downloading again.\n' "$problem"
    rm -f "$dest"
  fi

  mkdir -p "$folder"
  # Partial downloads left by a run that was killed outright (the trap removes them otherwise).
  rm -f "$folder/.$file.part".*
  # macOS mktemp only fills in X's at the very end of the template.
  tmp=$(mktemp "$folder/.$file.part.XXXXXX")

  printf 'Downloading %s (%s) into %s\n' "$id" "$(megabytes "$bytes")" "$folder"
  printf 'from %s\n' "$url"
  local progress=(--silent --show-error)
  [[ -t 2 ]] && progress=(--progress-bar)
  started=$SECONDS
  if ! curl --fail --location --proto '=https' --retry 3 --connect-timeout 30 "${progress[@]}" \
    --output "$tmp" "$url"; then
    die "the download failed (see the curl error above). Nothing was saved; run the script again to retry."
  fi
  printf 'Downloaded in %d s. Checking the sha256 ...\n' "$((SECONDS - started))"

  problem=$(check_file "$tmp")
  if [[ -n $problem ]]; then
    die "the download doesn't match the catalog: $problem. It was deleted. If this keeps happening, the file on the server has changed."
  fi
  chmod 644 "$tmp"
  mv -f "$tmp" "$dest"
  tmp=""
  printf 'Verified and saved: %s\n' "$dest"
}

case $# in
  0) fetch "$(default_id)" ;;
  1)
    case $1 in
      -h | --help) usage ;;
      -l | --list) list_models ;;
      -*)
        usage >&2
        exit 2
        ;;
      *) fetch "$1" ;;
    esac
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac
