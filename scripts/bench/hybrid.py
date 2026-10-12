"""hybrid.py RUN_DIR [LABEL ...]: net SoC energy per minute with the CPU rail taken robustly.
This is the "energy per minute" docs/model-bench.md quotes from "Lightness" on. Numbers only.

GPU, ANE and DRAM: plain integration over the session minus the mean of the idle windows
(they're quiet at idle, so that's exact). CPU: other processes burst now and then, in the idle
windows and in the session alike, so the CPU rail's net is the median of 5 s block means over
the session minus the median over the idle windows (before and after), times the session
length. Reads the same files as energy.py (run1.sh's output); LABEL defaults to every
*.meta.json in RUN_DIR."""
import glob, json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import energy  # noqa: E402


def blocks(samples, lo, hi, rail, size=5.0):
    out, t = [], lo
    while t + size <= hi:
        e, c = energy.window(samples, t, t + size)
        if c > size * 0.8:
            out.append(e[rail] / c)
        t += size
    return out


def med(v):
    v = sorted(v)
    return v[len(v) // 2] if v else float("nan")


def hybrid(run_dir, label):
    meta = json.load(open(os.path.join(run_dir, label + ".meta.json")))
    bench = json.load(open(os.path.join(run_dir, label + ".json")))
    S = energy.load_samples(os.path.join(run_dir, label + ".jsonl"))
    s1 = meta["t_exit"] - energy.EXIT_PAD
    s0 = s1 - bench["cpu"]["wall_secs"]
    T = s1 - s0
    pre = (meta["t_pre0"] + 5, meta["t_launch"] - 1)
    post = (meta["t_exit"] + 10, meta["t_post1"] - 1)
    es, cs = energy.window(S, s0, s1)
    idle = {}
    for rail in ("gpu", "ane", "dram"):
        ep, cp = energy.window(S, *pre)
        eq, cq = energy.window(S, *post)
        idle[rail] = (ep[rail] / cp + eq[rail] / cq) / 2
    net = {r: es[r] - idle[r] * T for r in ("gpu", "ane", "dram")}
    cpu_session = med(blocks(S, s0, s1, "cpu"))
    cpu_idle = med(blocks(S, *pre, "cpu") + blocks(S, *post, "cpu"))
    net["cpu"] = (cpu_session - cpu_idle) * T
    audio_min = bench["audio_secs"] / 60
    total = sum(net.values())
    return {"label": label, "J_min_hyb": round(total / audio_min, 1), "W_add_hyb": round(total / T, 3),
            "W_cpu_hyb": round(net["cpu"] / T, 3), "W_gpu": round(net["gpu"] / T, 3), "W_dram": round(net["dram"] / T, 3)}


if __name__ == "__main__":
    run_dir = sys.argv[1]
    labels = sys.argv[2:] or sorted(os.path.basename(p)[: -len(".meta.json")]
                                    for p in glob.glob(os.path.join(run_dir, "*.meta.json")))
    cols = ["label", "J_min_hyb", "W_add_hyb", "W_cpu_hyb", "W_gpu", "W_dram"]
    print("\t".join(cols))
    for l in labels:
        h = hybrid(run_dir, l)
        print("\t".join(str(h[c]) for c in cols))
