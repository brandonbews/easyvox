"""summary.py ENERGY_CSV BASELINE_CONFIG [OTHER_BASELINE ...]: per-config mean, sd, CV and
the ratio of each config's J/min to each baseline, with a 90 % confidence interval (Welch t on
log J/min). Runs with flags other than "gap-loaded" are left out and listed. Numbers only."""
import csv, math, sys
from collections import defaultdict

# Two-sided 90 % t quantiles (Python 3.9 stdlib has no t distribution).
T90 = {1: 6.314, 2: 2.920, 3: 2.353, 4: 2.132, 5: 2.015, 6: 1.943, 7: 1.895, 8: 1.860, 9: 1.833,
       10: 1.812, 12: 1.782, 15: 1.753, 20: 1.725, 30: 1.697}


def t90(df):
    keys = [k for k in sorted(T90) if k <= max(1, int(df))]
    return T90[keys[-1]]


def msd(xs):
    m = sum(xs) / len(xs)
    sd = math.sqrt(sum((x - m) ** 2 for x in xs) / (len(xs) - 1)) if len(xs) > 1 else float("nan")
    return m, sd


rows = list(csv.DictReader(open(sys.argv[1])))
baselines = sys.argv[2:]
by = defaultdict(list)
dropped = []
for r in rows:
    bad = [f for f in r["flags"].split() if not f.startswith("gap-loaded")]
    if bad:
        dropped.append((r["label"], " ".join(bad)))
        continue
    by[r["config"]].append(r)

print("config\tn\tJ/min soc mean\tsd\tCV%\tJ/min sys mean\tW cpu\tW gpu\tW ane\tW dram\tloaded-idle mW\tfinalize ms\tWER %")
for c, rs in sorted(by.items()):
    j = [float(r["J_per_min_soc"]) for r in rs]
    m, sd = msd(j)
    avg = lambda k: sum(float(r[k]) for r in rs) / len(rs)
    wer = [float(r["wer_pct"]) for r in rs if r["wer_pct"] not in ("", None)]
    fin = sorted(float(r["finalize_ms"]) for r in rs if r["finalize_ms"] not in ("", "None"))
    print("%s\t%d\t%.1f\t%.1f\t%.1f\t%.1f\t%.3f\t%.3f\t%.3f\t%.3f\t%.0f\t%s\t%s" % (
        c, len(rs), m, sd, 100 * sd / m if len(rs) > 1 else float("nan"), avg("J_per_min_sys"),
        avg("W_cpu"), avg("W_gpu"), avg("W_ane"), avg("W_dram"), avg("mW_loaded_idle_soc"),
        "%.0f" % fin[len(fin) // 2] if fin else "", "%.2f" % (sum(wer) / len(wer)) if wer else ""))

for b in baselines:
    if b not in by:
        print("baseline %s has no clean runs" % b)
        continue
    lb = [math.log(float(r["J_per_min_soc"])) for r in by[b]]
    print("\nratio of J/min (soc) to %s, 90 %% CI:" % b)
    for c, rs in sorted(by.items()):
        if c == b:
            continue
        lc = [math.log(float(r["J_per_min_soc"])) for r in rs]
        (mc, sc), (mb, sb) = msd(lc), msd(lb)
        ratio = math.exp(mc - mb)
        if len(lc) > 1 and len(lb) > 1:
            vc, vb = sc ** 2 / len(lc), sb ** 2 / len(lb)
            se = math.sqrt(vc + vb)
            df = (vc + vb) ** 2 / ((vc ** 2 / (len(lc) - 1)) + (vb ** 2 / (len(lb) - 1))) if se > 0 else 30
            lo, hi = math.exp(mc - mb - t90(df) * se), math.exp(mc - mb + t90(df) * se)
            verdict = "halves" if hi <= 0.5 else ("does not halve" if lo > 0.5 else "undecided: add runs")
            print("  %s / %s = %.3f  [%.3f, %.3f]  -> %s" % (c, b, ratio, lo, hi, verdict))
        else:
            print("  %s / %s = %.3f  (one run: no CI)" % (c, b, ratio))

if dropped:
    print("\nleft out (flagged):")
    for l, f in dropped:
        print("  %s: %s" % (l, f))
