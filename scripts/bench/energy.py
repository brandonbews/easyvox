"""energy.py RUN_DIR [LABEL ...]: energy per run from macmon samples. Prints numbers only.

For each LABEL (default: every *.meta.json in RUN_DIR) it reads LABEL.jsonl (macmon pipe),
LABEL.meta.json (wall-clock marks from run1.sh), LABEL.json (bench session result) and
LABEL.ps (ps samples), and writes RUN_DIR/energy.csv plus a table on stdout.

How the numbers are made:
- macmon's sampler keeps its IOReport baseline between samples, so each sample's power is
  the average over the whole interval since the previous one. Sample i therefore covers
  (t[i-1], t[i]]; energy in a window is the sum of power x overlap. Sample 0 is dropped.
- Windows (unix seconds):
    pre      [t_pre0 + 5, t_launch - 1]       machine idle, before the bench starts
    loaded   [s0 - LOADED_S, s0 - 1]          bench loaded and idle (--idle-before; LOADED_S=15)
    session  [s0, s1]                         s1 = t_exit - EXIT_PAD, s0 = s1 - cpu.wall_secs
    post     [t_exit + 10, t_post1 - 1]       machine idle again
  cpu.wall_secs is the bench's own session wall time (start of replay to Final + join).
- Rails: cpu, gpu, ane, dram (macmon ram_power), sys (SMC PSTR, whole machine, slow).
  soc = cpu + gpu + ane + dram is the headline rail.
- Baseline = mean of pre and post power. net = session energy - baseline x session time.
- J/min = net soc energy / (audio_secs / 60).
- Flags: drift (|post - pre| soc > 0.15 W), foreign CPU (other processes > 10 % of a core on
  average during the session), rc != 0, bench errors, Core ML over-capacity decodes.
"""
import csv, glob, json, os, re, sys
from datetime import datetime

EXIT_PAD = 0.15   # seconds between Final + join and process exit (WER calc, JSON write, exit)
LOADED_S = float(os.environ.get("LOADED_S", "15"))   # loaded-idle window length; IDLE=600 runs: LOADED_S=580
RAILS = {"cpu": "cpu_power", "gpu": "gpu_power", "ane": "ane_power", "dram": "ram_power", "sys": "sys_power"}
SELF = re.compile(r"(macmon|/ps$|/awk$|/sleep$|python3|Python|run1\.sh|bash$|/sh$)")


def ts(s):
    """RFC 3339 from chrono (3, 6 or 9 fraction digits) to unix seconds, Python 3.9-safe."""
    m = re.match(r"(.*T\d\d:\d\d:\d\d)(\.\d+)?(.*)$", s)
    frac = (m.group(2) or ".0")[1:7].ljust(6, "0")
    tz = m.group(3).replace("Z", "+00:00")
    return datetime.fromisoformat(m.group(1) + "." + frac + tz).timestamp()


def load_samples(path):
    rows = [json.loads(l) for l in open(path) if l.strip()]
    out = []
    for prev, cur in zip(rows, rows[1:]):
        a, b = ts(prev["timestamp"]), ts(cur["timestamp"])
        if b > a:
            out.append((a, b, cur))
    return out


def window(samples, lo, hi):
    """Energy (J) per rail and covered seconds in [lo, hi]."""
    e = {k: 0.0 for k in RAILS}
    covered = 0.0
    for a, b, r in samples:
        ov = min(b, hi) - max(a, lo)
        if ov <= 0:
            continue
        covered += ov
        for k, f in RAILS.items():
            e[k] += r[f] * ov
    e["soc"] = e["cpu"] + e["gpu"] + e["ane"] + e["dram"]
    return e, covered


def stats_in(samples, lo, hi, key):
    vals = [key(r) for a, b, r in samples if b > lo and a < hi]
    return vals


def foreign_cpu(ps_path, lo, hi, bench_pid):
    """Mean and max summed %CPU of other processes during [lo, hi], and the top three."""
    snaps, cur, t = [], None, None
    for line in open(ps_path):
        if line.startswith("T "):
            if cur is not None and lo <= t <= hi:
                snaps.append(cur)
            t, cur = float(line.split()[1]), {}
            continue
        parts = line.split(None, 2)
        if len(parts) < 3 or cur is None:
            continue
        pid, pcpu, comm = int(parts[0]), float(parts[1]), parts[2].strip()
        if pid == bench_pid or SELF.search(comm):
            continue
        name = os.path.basename(comm)
        cur[name] = cur.get(name, 0.0) + pcpu
    if cur is not None and t is not None and lo <= t <= hi:
        snaps.append(cur)
    if not snaps:
        return float("nan"), float("nan"), ""
    totals = [sum(s.values()) for s in snaps]
    per = {}
    for s in snaps:
        for k, v in s.items():
            per[k] = per.get(k, 0.0) + v / len(snaps)
    top = sorted(per.items(), key=lambda kv: -kv[1])[:3]
    return sum(totals) / len(totals), max(totals), " ".join("%s:%.0f" % kv for kv in top)


def analyse(run_dir, label):
    meta = json.load(open(os.path.join(run_dir, label + ".meta.json")))
    samples = load_samples(os.path.join(run_dir, label + ".jsonl"))
    bench_path = os.path.join(run_dir, label + ".json")
    bench = json.load(open(bench_path)) if os.path.exists(bench_path) else {}
    flags = []
    if meta.get("rc") != 0 or not bench:
        flags.append("bench-failed")
    if bench.get("errors"):
        flags.append("bench-errors=%d" % len(bench["errors"]))
    if bench.get("tcpp_overcap_decodes"):
        flags.append("overcap=%d" % bench["tcpp_overcap_decodes"])

    wall = bench.get("cpu", {}).get("wall_secs", float("nan"))
    s1 = meta["t_exit"] - EXIT_PAD
    s0 = s1 - wall
    win = {
        "pre": (meta["t_pre0"] + 5, meta["t_launch"] - 1),
        "loaded": (s0 - LOADED_S, s0 - 1),
        "session": (s0, s1),
        "post": (meta["t_exit"] + 10, meta["t_post1"] - 1),
    }
    e, cov, p = {}, {}, {}
    for name, (lo, hi) in win.items():
        e[name], cov[name] = window(samples, lo, hi)
        p[name] = {k: v / cov[name] if cov[name] > 0 else float("nan") for k, v in e[name].items()}
        if cov[name] < 0.9 * (hi - lo):
            flags.append("gap-%s" % name)
    base = {k: (p["pre"][k] + p["post"][k]) / 2 for k in p["pre"]}
    drift = p["post"]["soc"] - p["pre"]["soc"]
    if abs(drift) > 0.6:   # this Mac's background (simulator, agents) bursts; 0.15 W was too strict
        flags.append("drift=%.2fW" % drift)
    T = s1 - s0
    net = {k: e["session"][k] - base[k] * T for k in base}
    blocks = []
    for name in ("pre", "post"):
        lo, hi = win[name]
        t = lo
        while t + 5 <= hi:
            eb, cb = window(samples, t, t + 5)
            if cb > 4:
                blocks.append(eb["soc"] / cb)
            t += 5
    blocks.sort()
    base_robust = blocks[len(blocks) // 2] if blocks else float("nan")
    net_robust = e["session"]["soc"] - base_robust * T
    audio_min = bench.get("audio_secs", float("nan")) / 60.0
    fmean, fmax, ftop = foreign_cpu(os.path.join(run_dir, label + ".ps"), s0, s1, meta["bench_pid"])
    if fmean == fmean and fmean > 40:   # ~20 % of a core is this Mac's usual background
        flags.append("foreign-cpu=%.0f%%" % fmean)
    temps = stats_in(samples, s0, s1, lambda r: r["temp"]["cpu_temp_avg"])
    gpu_act = stats_in(samples, s0, s1, lambda r: r["gpu_active_ratio"])
    fans = stats_in(samples, s0, s1, lambda r: max([f["rpm"] for f in r.get("fans", [])] or [0]))
    part = bench.get("partials", {})
    return {
        "label": label,
        "config": label.rsplit("-r", 1)[0],
        "audio_s": round(bench.get("audio_secs", float("nan")), 1),
        "session_s": round(T, 1),
        "J_per_min_soc": round(net["soc"] / audio_min, 1),
        "J_per_min_robust": round(net_robust / audio_min, 1),
        "W_base_robust": round(base_robust, 3),
        "J_per_min_sys": round(net["sys"] / audio_min, 1),
        "J_per_min_gross_soc": round(e["session"]["soc"] / audio_min, 1),
        "W_cpu": round(p["session"]["cpu"] - base["cpu"], 3),
        "W_gpu": round(p["session"]["gpu"] - base["gpu"], 3),
        "W_ane": round(p["session"]["ane"] - base["ane"], 3),
        "W_dram": round(p["session"]["dram"] - base["dram"], 3),
        "W_base_soc": round(base["soc"], 3),
        "mW_loaded_idle_soc": round(1000 * (p["loaded"]["soc"] - base["soc"]), 0),
        "temp_start_C": round(temps[0], 1) if temps else float("nan"),
        "temp_max_C": round(max(temps), 1) if temps else float("nan"),
        "fan_max_rpm": max(fans) if fans else 0,
        "gpu_active_mean": round(sum(gpu_act) / len(gpu_act), 3) if gpu_act else float("nan"),
        "foreign_cpu_mean_pct": round(fmean, 1),
        "foreign_cpu_max_pct": round(fmax, 1),
        "foreign_top": ftop,
        "finalize_ms": bench.get("finalize_ms"),
        "first_partial_ms": part.get("first_partial_ms"),
        "passes": part.get("passes"),
        "skipped": part.get("skipped"),
        "median_gap_ms": part.get("median_gap_ms"),
        "revision_rate": part.get("revision_rate"),
        "chunks": bench.get("chunks", {}).get("polished"),
        "wer_pct": round(100 * bench["wer"]["wer"], 2) if "wer" in bench else "",
        "cpu_pct_one_core": round(bench.get("cpu", {}).get("cpu_pct_of_one_core", float("nan")), 1),
        "load_before": meta["before"]["load"],
        "flags": " ".join(flags),
    }


def main():
    run_dir = sys.argv[1]
    labels = sys.argv[2:] or sorted(os.path.basename(p)[: -len(".meta.json")] for p in glob.glob(os.path.join(run_dir, "*.meta.json")))
    rows = [analyse(run_dir, l) for l in labels]
    if not rows:
        sys.exit("no runs")
    with open(os.path.join(run_dir, "energy.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(rows)
    cols = ["label", "J_per_min_soc", "J_per_min_sys", "W_cpu", "W_gpu", "W_ane", "W_dram", "mW_loaded_idle_soc",
            "temp_start_C", "foreign_cpu_mean_pct", "finalize_ms", "passes", "wer_pct", "flags"]
    print("\t".join(cols))
    for r in rows:
        print("\t".join(str(r[c]) for c in cols))


if __name__ == "__main__":
    main()
