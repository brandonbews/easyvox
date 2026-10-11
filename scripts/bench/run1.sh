#!/bin/bash
# run1.sh LABEL BIN WAV [VAR=value ...] [-- extra bench args]
#
# One energy run, as in docs/model-bench.md "Lightness" ("How it was measured"), logged under
# $OUT (required):
#   1. macmon pipe (500 ms) and a sampler every 2 s (ps over 0.5 % CPU, plus the GPU's
#      "Device Utilization %" from ioreg) start;
#   2. PRE seconds of machine idle (no bench running): the baseline;
#   3. the bench: model load, warm-up decode, IDLE s loaded and idle (--idle-before), then the
#      real-time replay of WAV through the session, to its final text;
#   4. POST seconds of machine idle: the drift check.
# Files: LABEL.jsonl (macmon), LABEL.ps (ps and ioreg), LABEL.json (bench result),
# LABEL.err (bench stderr), LABEL.meta.json (wall-clock marks for energy.py).
# The bench runs without --quiet, so LABEL.err holds the preview's partial lines for lag.py.
# LABEL.json and LABEL.err hold transcript text: keep $OUT out of the repo, and for a private
# recording, keep it private. energy.py, hybrid.py and lag.py print numbers only.
#
#   OUT=/tmp/runs run1.sh A-r1 target/release/easyvox-bench recordings/ramble.wav -- --set preview_interval_ms=1200
set -u
OUT=${OUT:?set OUT to the folder for the run files}
MODEL=${MODEL:-parakeet-tdt-0.6b-v2-q5_k_m-gguf}
PRE=${PRE:-60}
IDLE=${IDLE:-10}
POST=${POST:-60}
REF=${REF:-}

L=$1; BIN=$2; WAV=$3; shift 3
ENVS=(); ARGS=()
while (($#)); do
  if [[ $1 == -- ]]; then shift; ARGS=("$@"); break; fi
  ENVS+=("$1"); shift
done
mkdir -p "$OUT"
for f in "$OUT/$L".{jsonl,ps,json,err,meta.json}; do
  [[ -e $f ]] && { echo "refusing to overwrite $f"; exit 2; }
done

now() { python3 -I -c 'import time; print("%.3f" % time.time())'; }
snap() { echo "\"load\": \"$(sysctl -n vm.loadavg)\", \"therm\": $(pmset -g therm | python3 -I -c 'import json,sys; print(json.dumps(sys.stdin.read()))')"; }

macmon pipe -i 500 > "$OUT/$L.jsonl" 2>/dev/null & MM=$!
( while :; do
    echo "T $(now)"
    echo "G $(ioreg -r -d 1 -c IOAccelerator | grep -o '"Device Utilization %"=[0-9]*' | head -1 | cut -d= -f2)"
    ps -A -o pid=,pcpu=,comm= | awk '$2 > 0.5'
    sleep 2
  done ) > "$OUT/$L.ps" 2>/dev/null & PSS=$!
trap 'kill $MM $PSS 2>/dev/null' EXIT

t_pre0=$(now); before=$(snap)
sleep "$PRE"
t_launch=$(now)
refargs=(); [[ -n $REF ]] && refargs=(--ref "$REF")
env ${ENVS[@]+"${ENVS[@]}"} "$BIN" session --model "$MODEL" --wav "$WAV" --idle-before "$IDLE" \
  --out "$OUT/$L.json" ${refargs[@]+"${refargs[@]}"} ${ARGS[@]+"${ARGS[@]}"} 2> "$OUT/$L.err" &
BP=$!
wait $BP; rc=$?
t_exit=$(now)
sleep "$POST"
t_post1=$(now); after=$(snap)
kill $MM $PSS 2>/dev/null; wait $MM $PSS 2>/dev/null

python3 -I - "$OUT/$L.meta.json" <<EOF
import json, sys
meta = {
  "label": "$L", "bin": "$BIN", "bin_sha256": "$(shasum -a 256 "$BIN" | cut -c1-16)",
  "wav": "$(basename "$WAV")", "model": "$MODEL", "env": $(printf '%s\n' ${ENVS[@]+"${ENVS[@]}"} | python3 -I -c 'import json,sys; print(json.dumps([l for l in sys.stdin.read().splitlines() if l]))'),
  "args": $(printf '%s\n' ${ARGS[@]+"${ARGS[@]}"} | python3 -I -c 'import json,sys; print(json.dumps([l for l in sys.stdin.read().splitlines() if l]))'),
  "ref": bool("$REF"), "pre_s": $PRE, "idle_s": $IDLE, "post_s": $POST,
  "t_pre0": $t_pre0, "t_launch": $t_launch, "t_exit": $t_exit, "t_post1": $t_post1,
  "bench_pid": $BP, "rc": $rc, "src_rev": "${SRC_REV:-}",
  "before": {$before}, "after": {$after},
  "lowpowermode": "$(pmset -g | awk '/lowpowermode/{print $2}')",
  "os": "$(sw_vers -productVersion) $(sw_vers -buildVersion)", "macmon": "$(macmon --version)",
}
json.dump(meta, open(sys.argv[1], "w"), indent=1)
EOF
echo "$L rc=$rc"
exit $rc
