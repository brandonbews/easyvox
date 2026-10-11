#!/bin/bash
# quiet.sh [MAX_WAIT_MIN]: wait until this Mac is quiet enough for an energy run.
# Exits 0 when quiet, 1 on timeout. One status line per check, on stdout.
#
# "Quiet" means all of:
#   - 1-minute load average under LOAD_MAX;
#   - no build/indexing/benchmark process using more than 5 % of a core, and nothing else
#     above 20 % of a core (ps);
#   - a 15 s macmon probe: mean CPU power under CPU_W_MAX, no sample over CPU_W_PEAK,
#     GPU and ANE essentially idle;
#   - CPU temperature back within TEMP_TOL of TEMP_REF (when TEMP_REF is set);
#   - pmset reports no thermal or performance warning.
# Calibrate CPU_W_MAX / CPU_W_PEAK from a few minutes of macmon on the idle machine.
set -u
MAX_MIN=${1:-30}
LOAD_MAX=${LOAD_MAX:-2.0}
CPU_W_MAX=${CPU_W_MAX:-0.35}
CPU_W_PEAK=${CPU_W_PEAK:-1.5}
GPU_W_MAX=${GPU_W_MAX:-0.05}
ANE_W_MAX=${ANE_W_MAX:-0.02}
TEMP_REF=${TEMP_REF:-}
TEMP_TOL=${TEMP_TOL:-1.5}
BUSY_RE=${BUSY_RE:-'xcodebuild|XCBBuildService|SWBBuildService|swift-frontend|swift-build|swiftc|clang|/ld$|ld64|cargo|rustc|cc1|cmake|ninja|/make$|ibtool|actool|coremlcompiler|ANECompilerService|easyvox-bench|EasyVox|mds_stores|mdworker|backupd|softwareupdated|bird$|photoanalysisd'}

deadline=$(( $(date +%s) + MAX_MIN * 60 ))
probe=$(mktemp -t quietprobe)
trap 'rm -f "$probe"' EXIT

while :; do
  why=""
  load=$(sysctl -n vm.loadavg | awk '{print $2}')
  awk -v l="$load" -v m="$LOAD_MAX" 'BEGIN{exit !(l < m)}' || why+="load=$load "

  busy=$(ps -A -o pcpu=,comm= | awk -v re="$BUSY_RE" '($1 > 5 && $0 ~ re) || $1 > 20 {printf "%s(%s%%) ", $2, $1}')
  [[ -n "$busy" ]] && why+="busy: $busy"

  therm=$(pmset -g therm)
  if ! grep -q "No thermal warning level" <<<"$therm" || ! grep -q "No performance warning level" <<<"$therm"; then
    why+="pmset-therm-warning "
  fi

  if [[ -z "$why" ]]; then
    macmon pipe -i 500 -s 30 > "$probe" 2>/dev/null
    verdict=$(python3 -I - "$probe" "$CPU_W_MAX" "$CPU_W_PEAK" "$GPU_W_MAX" "$ANE_W_MAX" "${TEMP_REF:-nan}" "$TEMP_TOL" <<'EOF'
import json, math, sys
path, cmax, cpeak, gmax, amax, tref, ttol = sys.argv[1], *map(float, sys.argv[2:])
rows = [json.loads(l) for l in open(path) if l.strip()][1:]   # first sample spans macmon's start-up
if not rows:
    print("no-macmon-samples"); sys.exit()
mean = lambda k: sum(r[k] for r in rows) / len(rows)
cpu, gpu, ane = mean("cpu_power"), mean("gpu_power"), mean("ane_power")
peak = max(r["cpu_power"] for r in rows)
temp = rows[-1]["temp"]["cpu_temp_avg"]
bad = []
if cpu >= cmax: bad.append("cpu=%.2fW" % cpu)
if peak >= cpeak: bad.append("cpu_peak=%.2fW" % peak)
if gpu >= gmax: bad.append("gpu=%.2fW" % gpu)
if ane >= amax: bad.append("ane=%.2fW" % ane)
if not math.isnan(tref) and abs(temp - tref) > ttol: bad.append("temp=%.1fC(ref %.1f)" % (temp, tref))
print(" ".join(bad) if bad else "OK cpu=%.2fW peak=%.2fW gpu=%.2fW ane=%.3fW temp=%.1fC" % (cpu, peak, gpu, ane, temp))
EOF
)
    if [[ "$verdict" == OK* ]]; then
      echo "$(date '+%H:%M:%S') quiet: load=$load $verdict"
      exit 0
    fi
    why+="$verdict"
  fi

  echo "$(date '+%H:%M:%S') waiting: $why"
  if (( $(date +%s) >= deadline )); then
    echo "gave up after $MAX_MIN min"
    exit 1
  fi
  sleep 30
done
