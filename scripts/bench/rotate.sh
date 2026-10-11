#!/bin/bash
# rotate.sh CONFIGS ROUNDS [FIRST_ROUND]: run every config ROUNDS times in rotated order
# (round r starts at config r), waiting for a quiet machine before each run.
#
# CONFIGS: one config per line, fields separated by "|":
#   NAME|BIN|WAV|VAR=value VAR=value|bench args
# Blank lines and lines starting with # are skipped. Paths must not contain "|" or spaces
# in the env/args fields (the WAV and BIN fields may contain spaces).
# Labels are NAME-rN; an existing label is skipped, so a stopped schedule can be resumed.
set -u
KIT=$(cd "$(dirname "$0")" && pwd)
export OUT=${OUT:?set OUT to the folder for the run files}
CONF=$1; ROUNDS=$2; FIRST=${3:-1}

lines=()
while IFS= read -r line; do
  [[ -z "$line" || "$line" == \#* ]] && continue
  lines+=("$line")
done < "$CONF"
n=${#lines[@]}

for ((r = FIRST; r <= ROUNDS; r++)); do
  for ((k = 0; k < n; k++)); do
    line=${lines[$(( (k + r - 1) % n ))]}
    IFS='|' read -r name bin wav envs args <<<"$line"
    label="$name-r$r"
    if [[ -e "$OUT/$label.meta.json" ]]; then echo "skip $label (done)"; continue; fi
    "$KIT/quiet.sh" "${QUIET_MAX_MIN:-30}" || { echo "machine not quiet; stopping before $label"; exit 1; }
    # shellcheck disable=SC2086  # envs and args are deliberately word-split
    "$KIT/run1.sh" "$label" "$bin" "$wav" $envs -- $args
    python3 -I "$KIT/energy.py" "$OUT" "$label" | tail -1
  done
done
