#!/usr/bin/env python3
"""Rebuild docs/bench/*.csv from EasyVox's raw bench runs of 9-10 October 2026. Numbers only.

    python3 -I scripts/bench/extract.py --s1 DIR --s2 DIR [--out docs/bench]
                                        [--testdata testdata] [--recordings recordings]

--s1 and --s2 are the two scratch folders the runs were made in (docs/bench/README.md lists what
each one holds). --recordings is only read for the loudness of the private read-aloud clip, to
place the start of its speech; leave it out and those few onsets stay empty.

The raw files hold text decoded from private recordings: final_text in the session JSON, and
preview, word and pass lines on stderr. This script reads numbers out of them and writes only
numbers and labels from a fixed vocabulary: model ids, clip file names, run ids, setting names and
values, and phases. Text is held in memory only to count the words of a bench decode and to time
the preview's words (the lag tables), and never written. Every cell is checked against that rule
before a file is written.

Standard library only, Python 3.9+.
"""
import argparse
import bisect
import csv
import glob
import json
import math
import os
import re
import statistics
import struct
import sys
from datetime import datetime

# ---------------------------------------------------------------- vocabulary

MODELS = {  # model id -> short alias used in run ids
    "parakeet-tdt-0.6b-v2-q5_k_m-gguf": "v2",
    "parakeet-tdt-0.6b-v2-q8_0-gguf": "v2q8",
    "parakeet-tdt-0.6b-v3-q5_k_m-gguf": "v3",
    "parakeet-unified-en-0.6b-q5_k_m-gguf": "uni",
    "whisper-base.en-q8_0-gguf": "wh",
    "canary-180m-flash-q5_k_m-gguf": "can",
}
SHORT_TAGS = {"parakeet-tdt": "v2", "parakeet-unified-en": "uni", "whisper-base.en": "wh"}

# study -> (docs/model-bench.md section, date of the runs)
STUDIES = {
    "results": ("Results", "2026-10-09"),
    "onepipe": ("One pipeline; Long dictation", "2026-10-09"),
    "longform": ("Long dictation (one pass)", "2026-10-09"),
    "longform-proto": ("Long dictation (prototype chunked runs, not in a table)", "2026-10-09"),
    "idle-lat": ("Idle and energy: Dictation after idling", "2026-10-09"),
    "idle-prev": ("Idle and energy: The preview in pauses", "2026-10-09"),
    "idle-fw": ("Idle and energy: The preview in pauses", "2026-10-09"),
    "idle-pn": ("Idle and energy: The preview in pauses", "2026-10-09"),
    "idle-real": ("Idle and energy: The preview in pauses", "2026-10-09"),
    "idle-tick": ("Idle and energy: The preview in pauses (first-words tick check)", "2026-10-09"),
    "idle-app": ("Idle and energy: The idle app", "2026-10-09"),
    "realvoice": ("Real voice", "2026-10-09"),
    "joinfix": ("Real voice: Chunk joins", "2026-10-09"),
    "tuning-check": ("none (check that the Tuning refactor changed nothing)", "2026-10-09"),
    "lightness": ("Lightness", "2026-10-10"),
    "settings-idle": ("Idle and energy: The idle app (the settings file)", "2026-10-10"),
    "firstwords-sanity": ("First words and silence (sanity runs)", "2026-10-10"),
    "firstpass-sweep": ("First words and silence (first-pass delay sweep)", "2026-10-10"),
    "firstpass-sweep-real": ("First words and silence (first-pass delay sweep)", "2026-10-10"),
    "firstwords-energy": ("First words and silence; Quiet mics and loud rooms (energy)", "2026-10-10"),
    "firstwords-lat": ("First words and silence", "2026-10-10"),
    "silence": ("First words and silence", "2026-10-10"),
    "starts-20db": ("First words and silence (first, 20 dB detector)", "2026-10-10"),
    "starts": ("First words and silence", "2026-10-10"),
    "stress": ("First words and silence: Quiet mics and loud rooms", "2026-10-10"),
    "v3": ("Real voice: Parakeet TDT v3 against v2", "2026-10-10"),
    "meter": ("Level meter", "2026-10-10"),
    "whisper": ("Whisper's preview", "2026-10-10"),
    "overlay": ("Overlay animation", "2026-10-10"),
}

SECTIONS = {v[0] for v in STUDIES.values()}
PRIVATE = {"ramble.wav", "structured.wav"}
TTS_RE = re.compile(r"^(cmd|punct|long|names|num)_[a-z]+\.wav$|^lf_\w+\.wav$|^(OK|Yes)_[A-Za-z]+\.wav$")

# Every text cell must be a known label: a model id, a clip file name, a run id, a setting, a phase.
CELL_RE = re.compile(r"^[A-Za-z0-9_.:/=;+\-#]*$")


def die(msg):
    sys.exit("extract.py: " + msg)


# ---------------------------------------------------------------- small helpers

def r(x, nd=1):
    """Round a number for the CSV; None and NaN become empty."""
    if x is None or isinstance(x, str):
        return x
    if isinstance(x, bool):
        return int(x)
    if isinstance(x, float) and (math.isnan(x) or math.isinf(x)):
        return None
    if isinstance(x, int):
        return x
    v = round(float(x), nd)
    return int(v) if nd == 0 else v


def load_json(path):
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def stem(path):
    return os.path.basename(path).rsplit(".", 1)[0]


def short(s):
    """Replace full model ids in a file stem with their aliases (for compact run ids)."""
    for k, v in sorted(MODELS.items(), key=lambda kv: -len(kv[0])):
        s = s.replace(k, v)
    for k, v in SHORT_TAGS.items():
        s = re.sub(r"(^|[-_])%s(?=[-_]|$)" % re.escape(k), r"\g<1>" + v, s)
    return s


def model_of(d):
    m = d.get("model")
    if m is None:
        return None
    if m not in MODELS:
        die("unknown model id %r" % m)
    return m


def clip_of(d):
    w = d.get("wav")
    if not w:
        return None
    c = os.path.basename(w)
    if not re.match(r"^[A-Za-z0-9_.\-]+\.wav$", c):
        die("unexpected clip name %r" % c)
    return c


def pct_rank(v, q):
    """The quantile the scratch analysis used: sorted(v)[round(q * (n - 1))]."""
    v = sorted(v)
    return v[min(len(v) - 1, int(q * (len(v) - 1) + 0.5))] if v else float("nan")


def read_lists(paths):
    """label -> (bin name, args) from the run lists ("[E|S|]label|bin|model|wav|ref|args")."""
    out = {}
    for p in paths:
        for line in open(p, errors="replace"):
            line = line.rstrip("\n")
            if not line or line.startswith("#") or "|" not in line:
                continue
            f = line.split("|")
            if len(f[0]) == 1 and len(f) > 2:
                f = f[1:]
            out[f[0]] = (os.path.basename(f[1]), f[5] if len(f) > 5 else "")
    return out


def settings_from_args(args):
    """'--set a=1 --set b=2' -> 'a=1;b=2' (setting names and values only)."""
    if not args:
        return ""
    kv = re.findall(r"--set\s+([a-z_]+)=([0-9.]+)", args)
    for flag, key in (("--chunk", "chunk_s"), ("--interval", "preview_interval_ms"), ("--window", "window_s")):
        m = re.search(re.escape(flag) + r"\s+([0-9.]+)", args)
        if m:
            kv.append((key, m.group(1)))
    return ";".join("%s=%s" % p for p in kv)


# ---------------------------------------------------------------- stderr parsers (text stays here)

PARTIAL_RE = re.compile(r"^\s*(\d+)ms  (.*)$")


def partial_events(path):
    """Preview updates: [(ms, confirmed text, provisional text)]. In memory only."""
    out = []
    for line in open(path, errors="replace"):
        m = PARTIAL_RE.match(line.rstrip("\n"))
        if not m or m.group(2).startswith("FINAL") or m.group(2).startswith("ERROR"):
            continue
        body = m.group(2)
        if "\x1b[2m" in body:
            conf, prov = body.split("\x1b[2m", 1)
            prov = prov.replace("\x1b[0m", "")
        else:
            conf, prov = body, ""
        out.append((int(m.group(1)), conf.strip(), prov.strip()))
    return out


NUM = {w: str(i) for i, w in enumerate("zero one two three four five six seven eight nine ten eleven twelve "
                                         "thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty".split())}
NUM.update({"thirty": "30", "forty": "40", "fifty": "50", "sixty": "60", "seventy": "70", "eighty": "80",
            "ninety": "90", "dr": "doctor", "mr": "mister", "mrs": "missus"})


def norm_word(w):
    w = w.lower()
    w = re.sub(r"(?<=\d)[,](?=\d)", "", w)
    w = w.replace("%", " percent")
    w = re.sub(r"[-/:—–]", " ", w)
    w = re.sub(r"[^\w\s$.]", "", w)
    w = re.sub(r"(?<!\d)\.|\.(?!\d)", "", w)
    out = []
    for t in w.split():
        if t.startswith("$") and len(t) > 1:
            out += [t[1:], "dollars"]
        else:
            out.append(NUM.get(t, t.strip("$")))
    return [t for t in out if t]


def norm(text):
    return [t for w in text.split() for t in norm_word(w)]


def timeline_word_lines(path):
    """WORD lines (a one-pass decode with word timings): [(token, start, end)]. In memory only."""
    out = []
    for line in open(path, errors="replace"):
        if line.startswith("WORD "):
            _, s, e, t = line.rstrip("\n").split(" ", 3)
            out += [(tok, float(s), float(e)) for tok in norm_word(t)]
    return out


def timeline_transcribe(path):
    """`transcribe --words` lines ('  start   end  word'): [(token, start, end)]. In memory only."""
    out = []
    for line in open(path, errors="replace"):
        m = re.match(r"^\s*(-?\d+\.\d+)\s+(-?\d+\.\d+)\s{2}(.*)$", line.rstrip("\n"))
        if m:
            out += [(tok, float(m.group(1)), float(m.group(2))) for tok in norm_word(m.group(3))]
    return out


def preview_lags(err_path, S):
    """The scratch lag.py method. For each timeline word, the first preview update whose last three
    words match the timeline at that word (2 of 3 at least, near the expected place), minus the
    word's end. Returns ([(word end s, lag s)], unmatched updates, timeline words)."""
    toks = [t for t, _, _ in S]
    shown, reach, unmatched = -1, [], 0
    for ms, conf, prov in partial_events(err_path):
        P = norm(conf + " " + prov)
        if len(P) < 3:
            continue
        tail, exp, best = P[-3:], len(P) - 1, None
        for j in range(max(2, exp - 60), min(len(S), exp + 61)):
            sc = (toks[j - 2] == tail[0]) + (toks[j - 1] == tail[1]) + (toks[j] == tail[2])
            key = (sc, -abs(j - exp))
            if best is None or key > best[0]:
                best = (key, j)
        if best and best[0][0] >= 2:
            shown = max(shown, best[1])
        else:
            unmatched += 1
        reach.append((ms, shown))
    lags, k = [], 0
    for i, (_, _, e) in enumerate(S):
        while k < len(reach) and reach[k][1] < i:
            k += 1
        if k == len(reach):
            break
        lags.append((e, reach[k][0] / 1000 - e))
    return lags, unmatched, len(S)


CUT_RE = re.compile(r"^CUT at ([\d.]+) s \(span ([\d.]+)\.\.([\d.]+), audio ([\d.]+) s, wall ([\d.]+) s\) "
                    r"words \+(\d+) decode (\d+) ms")
CUTDBG_RE = re.compile(r"^CUTDBG pause rel ([\d.]+)\.\.([\d.]+) \(([\d.]+) s\)")


def cut_lines(path):
    out, pause = [], None
    for line in open(path, errors="replace"):
        m = CUTDBG_RE.match(line)
        if m:
            pause = tuple(float(x) for x in m.groups())
            continue
        m = CUT_RE.match(line)
        if m:
            g = m.groups()
            out.append({"cut_at_s": float(g[0]), "span_start_s": float(g[1]), "span_end_s": float(g[2]),
                        "pending_audio_s": float(g[3]), "wall_s": float(g[4]) or None, "words_added": int(g[5]),
                        "decode_ms": int(g[6]), "pause_start_s": pause[0] if pause else None,
                        "pause_end_s": pause[1] if pause else None, "pause_s": pause[2] if pause else None})
            pause = None
    return out


def pass_lines(path):
    """The scratch bench's PASS and CHUNK lines (numbers only), and the cpu-time stamps."""
    passes, stamps = [], []
    for line in open(path, errors="replace"):
        f = line.split()
        if not f:
            continue
        if f[0] == "T0SET" and len(f) >= 3:
            stamps = [(0.0, float(f[2]))]
        elif f[0] == "PASS":
            d = dict(zip(f[2::2], f[3::2]))
            t = float(f[1])
            passes.append({"phase": "preview", "t_s": t, "window_s": float(d["total"]) - float(d["anchor"]),
                           "decode_ms": float(d["dec"]), "words": int(d["words"]),
                           "new_confirmed": int(d["newconf"]), "cpu_s": float(d["cpu"])})
            stamps.append((t, float(d["cpu"])))
        elif f[0] == "CHUNK":
            d = dict(zip(f[2::2], f[3::2]))
            a, b = (float(x) for x in d["span"].split(".."))
            passes.append({"phase": "chunk", "t_s": float(f[1]), "window_s": b - a, "decode_ms": float(d["dec"]),
                           "words": None, "new_confirmed": None, "cpu_s": float(d["cpu"])})
            stamps.append((float(f[1]), float(d["cpu"])))
        elif f[0] == "SKIP":
            stamps.append((float(f[1]), float(f[-1])))
    return passes, stamps


def cpu_per_minute_stamps(stamps, end_t):
    """whisper/analyse.py: % of one core in each minute, interpolating the cpu-time stamps."""
    def at(t):
        for (ta, ca), (tb, cb) in zip(stamps, stamps[1:]):
            if ta <= t <= tb:
                return ca + (cb - ca) * (t - ta) / (tb - ta) if tb > ta else cb
        return None
    out, m = [], 0
    while stamps and ((m + 1) * 60 <= end_t + 1e-6 or (end_t - m * 60) > 20):
        hi, lo = min((m + 1) * 60, stamps[-1][0]), m * 60
        a, b = at(lo), at(hi)
        if a is None or b is None or hi - lo < 20:
            break
        out.append(100 * (b - a) / (hi - lo))
        m += 1
    return out


def time_peak_mb(path):
    """/usr/bin/time -l's 'peak memory footprint' (bytes) on stderr, in MB."""
    for line in open(path, errors="replace"):
        m = re.match(r"^\s*(\d+)\s+peak memory footprint", line)
        if m:
            return int(m.group(1)) / 1e6
    return None


# ---------------------------------------------------------------- traces

def ints(path):
    out = []
    for line in open(path, errors="replace"):
        s = line.strip()
        if s.isdigit():
            out.append(int(s))
    return out


def cputime(s):
    p = [float(x) for x in s.split(":")]
    return p[0] * 3600 + p[1] * 60 + p[2] if len(p) == 3 else p[0] * 60 + p[1]


def ps_samples(path):
    """ps -o time=,rss= every 2 s: [(cpu seconds, rss KB)]."""
    out = []
    for line in open(path, errors="replace"):
        f = line.split()
        if len(f) == 2 and ":" in f[0] and f[1].isdigit():
            out.append((cputime(f[0]), int(f[1])))
    return out


def ps_snapshots(path, bench_pid=None):
    """The energy runs' .ps file: per snapshot (unix t, ioreg GPU %, bench %CPU, others' %CPU).
    Process names are read only to leave out the measuring tools; none is kept."""
    SELF = re.compile(r"(macmon|/ps$|/awk$|/sleep$|python3|Python|run1\.sh|bash$|/sh$)")
    snaps, cur = [], None
    for line in open(path, errors="replace"):
        if line.startswith("T "):
            if cur:
                snaps.append(cur)
            cur = {"t": float(line.split()[1]), "gpu": None, "bench": None, "other": 0.0}
            continue
        if cur is None:
            continue
        if line.startswith("G "):
            v = line.split()
            cur["gpu"] = int(v[1]) if len(v) == 2 and v[1].isdigit() else None
            continue
        parts = line.split(None, 2)
        if len(parts) < 3 or not parts[0].isdigit():
            continue
        try:
            pid, pcpu = int(parts[0]), float(parts[1])
        except ValueError:
            continue
        if pid == bench_pid:
            cur["bench"] = pcpu
        elif not SELF.search(parts[2].strip()):
            cur["other"] += pcpu
    if cur:
        snaps.append(cur)
    return snaps


def ts(s):
    """RFC 3339 (3, 6 or 9 fraction digits) to unix seconds."""
    m = re.match(r"(.*T\d\d:\d\d:\d\d)(\.\d+)?(.*)$", s)
    frac = (m.group(2) or ".0")[1:7].ljust(6, "0")
    tz = m.group(3).replace("Z", "+00:00")
    return datetime.fromisoformat(m.group(1) + "." + frac + tz).timestamp()


RAILS = {"cpu": "cpu_power", "gpu": "gpu_power", "ane": "ane_power", "dram": "ram_power", "sys": "sys_power"}


def macmon(path):
    """macmon pipe samples: [(t_prev, t, row)]; each covers (t_prev, t]."""
    rows = []
    for line in open(path, errors="replace"):
        if line.startswith("{"):
            try:
                rows.append(json.loads(line))
            except ValueError:
                pass
    out = []
    for a, b in zip(rows, rows[1:]):
        ta, tb = ts(a["timestamp"]), ts(b["timestamp"])
        if tb > ta:
            out.append((ta, tb, b))
    return out


def window(samples, lo, hi):
    e = {k: 0.0 for k in RAILS}
    cov = 0.0
    for a, b, row in samples:
        ov = min(b, hi) - max(a, lo)
        if ov <= 0:
            continue
        cov += ov
        for k, f in RAILS.items():
            e[k] += row.get(f, 0.0) * ov
    e["soc"] = e["cpu"] + e["gpu"] + e["ane"] + e["dram"]
    return e, cov


def blocks(samples, lo, hi, rail, size=5.0):
    out, t = [], lo
    while t + size <= hi:
        e, c = window(samples, t, t + size)
        if c > size * 0.8:
            out.append(e[rail] / c)
        t += size
    return out


def med(v):
    v = sorted(v)
    return v[len(v) // 2] if v else float("nan")


def energy_meta(samples, meta, bench, ps_path):
    """docs/model-bench.md "Lightness", "How it was measured" (the scratch energy.py and hybrid.py).
    Session = the replay's start to its final text; idle = the minute before and after the bench.
    j_per_min, w_added and w_cpu take the CPU rail robustly (medians of 5 s blocks); GPU, Neural
    Engine and DRAM subtract the idle mean. j_per_min_plain subtracts the idle mean from every rail."""
    s1 = meta["t_exit"] - 0.15
    s0 = s1 - bench["cpu"]["wall_secs"]
    T = s1 - s0
    pre = (meta["t_pre0"] + 5, meta["t_launch"] - 1)
    post = (meta["t_exit"] + 10, meta["t_post1"] - 1)
    es, _ = window(samples, s0, s1)
    ep, cp = window(samples, *pre)
    eq, cq = window(samples, *post)
    base = {k: (ep[k] / cp + eq[k] / cq) / 2 for k in ep}
    net = {k: es[k] - base[k] * T for k in ("gpu", "ane", "dram")}
    cpu_w = med(blocks(samples, s0, s1, "cpu")) - med(blocks(samples, *pre, "cpu") + blocks(samples, *post, "cpu"))
    net["cpu"] = cpu_w * T
    audio_min = bench["audio_secs"] / 60
    inside = [row for a, b, row in samples if b > s0 and a < s1]
    snaps = ps_snapshots(ps_path, meta.get("bench_pid")) if ps_path else []
    gpu_ioreg = [s["gpu"] for s in snaps if s["gpu"] is not None and s0 <= s["t"] <= s1]
    other = [s["other"] for s in snaps if s0 <= s["t"] <= s1]
    total = sum(net.values())
    return {
        "session_s": r(T, 2), "audio_s": r(bench["audio_secs"], 2),
        "j_per_min": r(total / audio_min, 1), "w_added": r(total / T, 3),
        "w_cpu": r(cpu_w, 3), "w_gpu": r(net["gpu"] / T, 3), "w_ane": r(net["ane"] / T, 3),
        "w_dram": r(net["dram"] / T, 3),
        "j_per_min_plain": r((es["soc"] - base["soc"] * T) / audio_min, 1),
        "j_per_min_sys": r((es["sys"] - base["sys"] * T) / audio_min, 1),
        "w_idle_soc": r(base["soc"], 3),
        "gpu_active_pct": r(100 * statistics.mean(x["gpu_active_ratio"] for x in inside), 2) if inside else None,
        "gpu_util_pct": r(statistics.mean(gpu_ioreg), 1) if gpu_ioreg else None,
        "temp_start_c": r(inside[0]["temp"]["cpu_temp_avg"], 1) if inside else None,
        "fan_max_rpm": r(max([max([f["rpm"] for f in x.get("fans", [])] or [0]) for x in inside] or [0]), 0),
        "other_cpu_pct": r(statistics.mean(other), 1) if other else None,
    }, (s0, s1)


def energy_app(samples, run):
    """The level meter's runs (the scratch abtools/analyse.py): the session is the self-test's
    start_unix_ms..end_unix_ms; idle is the rest of the trace, 8 s clear of the session."""
    start, end = run["start_unix_ms"] / 1000, run["end_unix_ms"] / 1000
    dur = end - start
    t0 = samples[0][0]
    sess = [(a, b, x) for a, b, x in samples if start < b <= end]
    idle = [(a, b, x) for a, b, x in samples if (t0 + 3 < b < start - 8) or b > end + 8]

    def blk(points, size=5.0):
        out, cur, tb = [], [], None
        for t, p in points:
            if tb is None:
                tb = t
            if t - tb >= size:
                if cur:
                    out.append(sum(cur) / len(cur))
                cur, tb = [], t
            cur.append(p)
        if cur:
            out.append(sum(cur) / len(cur))
        return out
    res, total = {}, 0.0
    for k, f in (("cpu", "cpu_power"), ("gpu", "gpu_power"), ("ane", "ane_power"), ("dram", "ram_power")):
        if k == "cpu":
            w = statistics.median(blk([(b, x[f]) for a, b, x in sess])) - statistics.median(blk([(b, x[f]) for a, b, x in idle]))
        else:
            w = sum(x[f] * (b - a) for a, b, x in sess) / dur - statistics.mean(x[f] for a, b, x in idle)
        res["w_" + k] = r(w, 3)
        total += w
    res.update({"session_s": r(dur, 2), "audio_s": r(run["audio_secs"], 2),
                "j_per_min": r(total * dur / (run["audio_secs"] / 60), 1), "w_added": r(total, 3),
                "gpu_active_pct": r(100 * statistics.mean(x["gpu_active_ratio"] for a, b, x in sess), 2)})
    return res, (start, end)


def power_bins(samples, s0, s1, pad=15):
    """Mean power per rail in 1 s bins from s0 - pad to s1 + pad, t_s relative to s0. The Neural
    Engine rail is left out: macmon read 0 W on it in every sample of every run."""
    out = []
    k0, k1 = -pad, int(math.ceil(s1 - s0)) + pad
    starts = [a for a, _, _ in samples]
    for k in range(k0, k1):
        lo, hi = s0 + k, s0 + k + 1
        acc = {"cpu": 0.0, "gpu": 0.0, "dram": 0.0, "act": 0.0}
        cov = 0.0
        i = max(0, bisect.bisect_left(starts, lo) - 1)
        while i < len(samples) and samples[i][0] < hi:
            a, b, row = samples[i]
            ov = min(b, hi) - max(a, lo)
            if ov > 0:
                cov += ov
                for kk, f in (("cpu", "cpu_power"), ("gpu", "gpu_power"), ("dram", "ram_power")):
                    acc[kk] += row.get(f, 0.0) * ov
                acc["act"] += row.get("gpu_active_ratio", 0.0) * ov
            i += 1
        if cov < 0.5:
            continue
        out.append({"t_s": k, "cpu_mw": r(1000 * acc["cpu"] / cov, 0), "gpu_mw": r(1000 * acc["gpu"] / cov, 0),
                    "dram_mw": r(1000 * acc["dram"] / cov, 0),
                    "gpu_active_pct": r(100 * acc["act"] / cov, 1)})
    return out


# ---------------------------------------------------------------- audio (onsets)

def wav_levels_db(path):
    """30 ms frame levels (RMS, dBFS) of a 16-bit or float mono WAV."""
    b = open(path, "rb").read()
    if b[:4] != b"RIFF" or b[8:12] != b"WAVE":
        die("not a WAV: %s" % os.path.basename(path))
    i, fmt, data = 12, None, None
    while i + 8 <= len(b):
        cid, size = b[i:i + 4], struct.unpack("<I", b[i + 4:i + 8])[0]
        body = b[i + 8:i + 8 + size]
        if cid == b"fmt ":
            tag, ch, sr, _, _, bits = struct.unpack("<HHIIHH", body[:16])
            if tag == 0xFFFE:
                tag = struct.unpack("<H", body[24:26])[0]
            fmt = (tag, ch, sr, bits)
        elif cid == b"data":
            data = body
        i += 8 + size + (size & 1)
    tag, ch, sr, bits = fmt
    import array
    if tag == 1 and bits == 16:
        pcm = [x / 32768.0 for x in array.array("h", data[: len(data) // 2 * 2])]
    elif tag == 3 and bits == 32:
        pcm = list(array.array("f", data[: len(data) // 4 * 4]))
    else:
        die("unsupported WAV format in %s" % os.path.basename(path))
    out = []
    for j in range(0, len(pcm) - 479, 480):
        c = pcm[j:j + 480]
        x = math.sqrt(sum(v * v for v in c) / 480)
        out.append(20 * math.log10(x) if x > 0 else -999.0)
    return out


_onsets = {}


def first_loud(path, lead=0.0):
    """docs/model-bench.md "First words and silence": speech starts at the first 120 ms (4 frames
    of 30 ms) above -40 dBFS. None if the file is missing."""
    if path is None or not os.path.exists(path):
        return None
    if path not in _onsets:
        run, found = 0, None
        for i, x in enumerate(wav_levels_db(path)):
            run = run + 1 if x > -40 else 0
            if run >= 4:
                found = (i + 1 - run) * 0.03
                break
        _onsets[path] = found
    v = _onsets[path]
    return None if v is None else lead + v


# ---------------------------------------------------------------- run enumeration

class Run(dict):
    pass


def session_files(d):
    return sorted(p for p in glob.glob(os.path.join(d, "*.json"))
                  if not p.endswith((".meta.json", ".row.json", ".config.json")))


def sidecar(base, *exts):
    for e in exts:
        if os.path.exists(base + e):
            return base + e
    return None


def enumerate_runs(S1, S2):
    runs, benches, apps = [], [], []

    def add(study, label, path, build, settings="", idx=None, err=None, extra=None):
        run = Run(study=study, label=label, run_id="%s/%s" % (study, label), path=path, idx=idx, build=build,
                  settings=settings, err=err)
        base = path[:-5]
        for k, exts in (("gpu", (".gpu",)), ("other", (".other",)), ("samples", (".samples",)),
                        ("ps", (".ps",)), ("macmon", (".jsonl",)), ("meta", (".meta.json",))):
            run[k] = sidecar(base, *exts)
        if err is None:
            run["err"] = sidecar(base, ".err", ".stderr")
        if extra:
            run.update(extra)
        runs.append(run)
        return run

    # ---- S2: 9 October
    d = os.path.join(S2, "mbench/out")
    for p in session_files(d):
        s = stem(p)
        if s.startswith("bench"):
            benches.append(Run(study="results", label=short(s), path=p, build="old-engine"))
        elif s.startswith("sessions"):
            j = load_json(p)
            for i, x in enumerate(j["runs"]):
                mode = {"partials_on": "on", "partials_off": "off"}[x["mode"]]
                add("results", "%s/%s-%s" % (short(s), x["clip"], mode), p, "old-engine", idx=i,
                    extra={"interval_ms": x.get("interval_ms"), "gpu": None, "samples": None, "err": None})
    d = os.path.join(S2, "onepipe/results/run")
    for p in session_files(d):
        s = stem(p)
        if s.startswith("bench"):
            benches.append(Run(study="onepipe", label=short(s), path=p, build="onepipe",
                               err=sidecar(p[:-5], ".stderr")))
        elif s.startswith("sessions"):
            j = load_json(p)
            for i, x in enumerate(j["runs"]):
                mode = {"partials_on": "on", "partials_off": "off"}[x["mode"]]
                add("onepipe", "%s/%s-%s" % (short(s), x["clip"], mode), p, "onepipe", idx=i,
                    extra={"interval_ms": x.get("interval_ms"), "err": None, "gpu": None, "samples": None,
                           "trace_id": "onepipe/" + short(s), "trace_gpu": sidecar(p[:-5], ".gpu"),
                           "trace_samples": sidecar(p[:-5], ".samples")})
        else:
            add("onepipe", short(s), p, "onepipe")
    for study, sub, build in (("longform", "longform/results/sessions", "old-engine"),
                              ("longform-proto", "longform/results/proto", "proto")):
        for p in session_files(os.path.join(S2, sub)):
            s = stem(p)
            j = load_json(p)
            if j is None:
                continue
            if "clips" in j:
                benches.append(Run(study=study, label=short(s), path=p, build=build))
            elif "finalize_ms" in j:
                add(study, short(s), p, build)
    p = os.path.join(S2, "longform/results/bench3_repo_whisper-base.en-q8_0-gguf.json")
    if os.path.exists(p):
        benches.append(Run(study="longform", label=short(stem(p)), path=p, build="old-engine"))
    for sub, study in (("lat", "idle-lat"), ("prev", "idle-prev"), ("fw", "idle-fw"), ("pn", "idle-pn"),
                       ("real", "idle-real"), ("tick", "idle-tick")):
        for p in session_files(os.path.join(S2, "idle", sub)):
            s = stem(p)
            if study == "idle-lat":
                build = "d33ab36"
                st = re.sub(r"-\d+$", "", s)
                settings = ";".join(x for x in (
                    "metal_residency=%d" % (1 if "res-on" in st else 0),
                    "idle_before_s=300" if "idle" in st.split("-") else "",
                    "unload=1" if st.startswith("unload") else "") if x)
            elif study == "idle-tick":
                build, settings = {"a": "after", "b": "before"}[s[0]], ""
            else:
                build = "before" if s.startswith("before") else "after"
                settings = ""
            add(study, s, p, build, settings)

    # ---- S1: 9-10 October
    def lists_for(*patterns):
        paths = []
        for pat in patterns:
            paths += glob.glob(os.path.join(S1, pat))
        return read_lists(paths)

    for p in session_files(os.path.join(S1, "realvoice/runs")):
        s = stem(p)
        add("realvoice", s, p, "1544b11", "chunk_ms=0" if s[1] == "0" else "")
    for p in session_files(os.path.join(S1, "joinfix/runs")):
        s = stem(p)
        add("joinfix", s, p, "after" if s.startswith("b-") else "before")
    for p in session_files(os.path.join(S1, "eq")):
        s = stem(p)
        add("tuning-check", s, p, "after" if s.startswith("after") else "before")
    lists = lists_for("lightness/queue*.txt")
    for p in session_files(os.path.join(S1, "lightness/runs")) + \
            [os.path.join(S1, "lightness/logs", x) for x in ("sanity-base.json", "sanity-new.json")]:
        if not os.path.exists(p):
            continue
        s = stem(p)
        meta = load_json(p[:-5] + ".meta.json")
        if meta:
            bin_, args = os.path.basename(meta["bin"]), " ".join(meta.get("args", []))
        elif s in lists:
            bin_, args = lists[s]
        else:
            bin_, args = None, ""
        if bin_ is None:
            if s.startswith("sanity-"):
                bin_ = "bench-" + s.split("-", 1)[1]
            else:
                pre = s.split("-")[1] if s.startswith(("LAT-", "U-", "W-")) else s.split("-")[0]
                bin_ = {"A": "bench-base", "B": "bench-base", "F": "bench-final", "LATX": "bench-final"}.get(
                    pre, "bench-new" if pre.startswith(("N", "LF")) else None)
                if s.startswith("LATX-res"):
                    bin_ = "bench-final-res"
                if s.startswith("LF"):
                    bin_ = "bench-new"
        add("lightness", s, p, (bin_ or "unknown").replace("bench-", ""), settings_from_args(args))
    for sub, study, listpat in (("sanity", "firstwords-sanity", None), ("sweep", "firstpass-sweep", None),
                                ("sweep-real", "firstpass-sweep-real", None),
                                ("energy/runs", "firstwords-energy", "engine2/energy/queue*.txt"),
                                ("lat", "firstwords-lat", "engine2/lat-list.txt"),
                                ("sil", "silence", "engine2/sil-list.txt"),
                                ("sil2", "starts-20db", "engine2/sil2-list.txt"),
                                ("sil3", "starts", "engine2/sil3-list.txt"),
                                ("stress-runs", "stress", "engine2/stress-list.txt")):
        lists = lists_for(listpat) if listpat else {}
        for p in session_files(os.path.join(S1, "engine2", sub)):
            s = stem(p)
            meta = load_json(p[:-5] + ".meta.json")
            if meta:
                bin_, args = os.path.basename(meta["bin"]), " ".join(meta.get("args", []))
            elif s in lists:
                bin_, args = lists[s]
            elif study == "firstwords-sanity":
                bin_, args = "bench-" + s.split("-")[0], ""
            elif study.startswith("firstpass"):
                bin_, args = "bench-new", ""
            else:
                bin_, args = {"B": "bench-base", "N": "bench-new"}.get(s[0]), ""
            settings = settings_from_args(args)
            if study.startswith("firstpass"):
                settings = "preview_first_ms=" + s.split("-")[0][1:]
            add(study, s, p, (bin_ or "unknown").replace("bench-", ""), settings)
    for p in session_files(os.path.join(S1, "v3/runs")):
        s = stem(p)
        j = load_json(p)
        if j and "clips" in j:
            benches.append(Run(study="v3", label=short(s), path=p, build="f424519"))
        elif j:
            add("v3", s, p, "f424519", "chunk_ms=0" if re.match(r"^[rs]0-", s) else "")
    lists = lists_for("whisper/queue*.txt")
    for p in session_files(os.path.join(S1, "whisper/runs")):
        s = stem(p)
        meta = load_json(p[:-5] + ".meta.json")
        if meta:
            bin_ = os.path.basename(meta["bin"])
        elif s in lists:
            bin_ = lists[s][0]
        elif s.startswith("cmp-"):
            bin_ = "bench-fix-cmp"
        elif s.endswith("-fix"):
            bin_ = "bench-fix"
        else:
            bin_ = "bench-head"
        add("whisper", s, p, bin_.replace("bench-", ""))

    # ---- app self-test runs
    for p in session_files(os.path.join(S1, "ab")):
        s = stem(p)
        apps.append(Run(study="meter", label=s, run_id="meter/" + s, path=p, build="b9dd9fb",
                        macmon=sidecar(p[:-5], ".jsonl")))
    for sub in ("runs", "runs2"):
        for p in session_files(os.path.join(S1, "overlay-anim", sub)):
            s = stem(p)
            build = "fc7c60a" if s.startswith("before") else "5bd95f9"
            apps.append(Run(study="overlay", label="%s/%s" % (sub, s), run_id="overlay/%s/%s" % (sub, s),
                            path=p, build=build, macmon=None))
    return runs, benches, apps


# ---------------------------------------------------------------- source category

def source_of(study, clip, label):
    if clip is None:
        return None
    if study == "stress":
        if label.startswith("noise-"):
            return "private-cut"
        name = re.sub(r"^[^-]+-(v2|wh|uni)-", "", label)
        if "soft" in label:
            return "synthetic"
        return "synthetic-from-private" if name.startswith(("start_", "structured", "ramble")) else "synthetic"
    if clip in PRIVATE:
        return "private"
    if clip == "silence_2s.wav":
        return "silence"
    if clip == "pauses_noise.wav":
        return "tts"
    if re.match(r"^[rs]\d+(\.\d+)?\.wav$", clip) or clip.startswith(("ramble_", "structured_")) or \
            re.match(r"^d\d+\.wav$", clip):
        return "private-cut"
    if TTS_RE.match(clip):
        return "tts"
    return "other"


# ---------------------------------------------------------------- rows

KNOBS = ("preview_interval_ms", "preview_window_ms", "preview_context_ms", "preview_first_ms", "chunk_ms",
         "speech_above_noise_db", "speech_min_ms", "agreement_reach_words", "meter_interval_ms")

RUN_COLS = ["run_id", "study", "build", "model", "clip", "source", "settings",
            "audio_s", "idle_before_s", "wer_pct", "wer_errors", "ref_words", "speech_onset_s", "first_words_ms",
            "finalize_ms", "final_wait_ms", "final_decode_ms", "model_wait_ms", "cpu_pct", "cpu_s", "wall_s",
            "gpu_util_pct", "chunks", "chunk_decode_ms_total", "chunk_decode_ms_max", "passes", "skipped",
            "pass_ms_mean", "pass_ms_max", "window_s_mean", "preview_events", "revision_rate", "revisions",
            "words_revised", "updates_per_s", "median_gap_ms", "confirmed_rewrites", "confirmed_vs_final_errors",
            "confirmed_coverage", "final_words", "metal_residency", "unloaded_before", "loadavg_before",
            "loadavg_after", "other_cpu_pct", "build_overlap_s"] + list(KNOBS)


def session_dict(run):
    j = load_json(run["path"])
    if run["idx"] is not None:
        x = j["runs"][run["idx"]]
        d = dict(x["result"])
        d.setdefault("model", j.get("model"))
        for k in ("loadavg_before", "loadavg_after"):
            if k in x:
                d.setdefault(k, x[k])
        return d
    return j


def run_row(run, d):
    p, c, ch, t = d.get("partials", {}), d.get("cpu", {}), d.get("chunks"), d.get("tuning", {})
    w = d.get("wer") or {}
    if not isinstance(ch, dict):
        ch = {"polished": ch} if isinstance(ch, int) else {}
    model, clip = model_of(d), clip_of(d)
    row = {
        "run_id": run["run_id"], "study": run["study"], "build": run["build"],
        "model": model, "clip": clip, "source": source_of(run["study"], clip, run["label"]),
        "settings": run["settings"],
        "audio_s": r(d.get("audio_secs"), 2), "idle_before_s": r(d.get("idle_before_secs"), 0),
        "wer_pct": r(100 * w["wer"], 2) if "wer" in w else None, "wer_errors": w.get("errors"),
        "ref_words": w.get("ref_words"), "speech_onset_s": r(d.get("speech_onset_secs"), 3),
        "first_words_ms": r(p.get("first_partial_ms"), 1), "finalize_ms": r(d.get("finalize_ms"), 0),
        "final_wait_ms": r(d.get("final_wait_ms"), 1), "final_decode_ms": r(d.get("final_decode_ms"), 1),
        "model_wait_ms": r(d.get("model_wait_ms"), 1), "cpu_pct": r(c.get("cpu_pct_of_one_core"), 2),
        "cpu_s": r(c.get("cpu_secs"), 2), "wall_s": r(c.get("wall_secs"), 2),
        "chunks": ch.get("polished"), "chunk_decode_ms_total": r(ch.get("decode_ms_total"), 1),
        "chunk_decode_ms_max": r(ch.get("decode_ms_max"), 1), "passes": p.get("passes"),
        "skipped": p.get("skipped"), "pass_ms_mean": r(p.get("pass_ms_mean"), 1),
        "pass_ms_max": r(p.get("pass_ms_max"), 1), "window_s_mean": r(p.get("window_secs_mean"), 2),
        "preview_events": p.get("events"), "revision_rate": r(p.get("revision_rate"), 4),
        "revisions": p.get("revisions"), "words_revised": p.get("words_revised"),
        "updates_per_s": r(p.get("updates_per_sec"), 3), "median_gap_ms": r(p.get("median_gap_ms"), 1),
        "confirmed_rewrites": p.get("confirmed_rewrites"),
        "confirmed_vs_final_errors": p.get("confirmed_vs_final_errors"),
        "confirmed_coverage": r(p.get("confirmed_coverage"), 4), "final_words": p.get("final_words"),
        "metal_residency": r(d.get("metal_residency")) if "metal_residency" in d else None,
        "unloaded_before": r(d.get("unloaded_before")) if "unloaded_before" in d else None,
        "loadavg_before": d.get("loadavg_before"), "loadavg_after": d.get("loadavg_after"),
    }
    for k in KNOBS:
        row[k] = t.get(k)
    if not t:
        if run.get("interval_ms") is not None:
            row["preview_interval_ms"] = run["interval_ms"]
        if d.get("chunk_secs") is not None:
            row["chunk_ms"] = r(1000 * d["chunk_secs"], 0)
    return row


def memory_rows(run_id, d, phases):
    out = []
    for key, phase in phases:
        m = d.get(key)
        if isinstance(m, dict):
            out.append({"run_id": run_id, "phase": phase, "footprint_mb": r(m.get("footprint_mb"), 1),
                        "peak_footprint_mb": r(m.get("peak_footprint_mb"), 1)})
    return out


def gpu_mean(path, skip):
    g = ints(path)
    g = g[skip:] if len(g) > skip + 2 else g
    return statistics.mean(g) if g else None


def trace_rows(trace_id, gpu=None, other=None, samples=None):
    rows = {}
    if gpu:
        for i, v in enumerate(ints(gpu)):
            rows.setdefault(i, {})["gpu_util_pct"] = v
    if other:
        for i, v in enumerate(ints(other)):
            rows.setdefault(i, {})["other_cpu_pct"] = v
    if samples:
        s = ps_samples(samples)
        for i in range(len(s)):
            row = rows.setdefault(2 * i, {})
            row["rss_mb"] = r(s[i][1] * 1024 / 1e6, 1)  # ps reports KiB; MB here is 10^6 bytes, as the bench
            if i:
                row["cpu_pct"] = r((s[i][0] - s[i - 1][0]) / 2 * 100, 1)
    return [dict(run_id=trace_id, t_s=k, **v) for k, v in sorted(rows.items())]


def cpu_minutes_ps(samples):
    """onepipe/tools/long_summary.py: per-2 s deltas of the process's CPU time, averaged per minute
    of the sampler (which starts with the model load)."""
    s = ps_samples(samples)
    by = {}
    for i in range(1, len(s)):
        by.setdefault(int(i * 2 // 60), []).append((s[i][0] - s[i - 1][0]) / 2 * 100)
    return [(k + 1, statistics.mean(v)) for k, v in sorted(by.items())]


# ---------------------------------------------------------------- stress cases

def stress_case(label):
    m = re.match(r"^(v2|wh)-soft(\d+)-f(\d+)(-nolead)?-(.+)$", label)
    if m:
        model, peak, floor, nolead, name = m.groups()
        return {"case": "soft", "model_alias": model, "source_clip": name + ".wav", "speech_peak_dbfs": -int(peak),
                "room_dbfs": -int(floor), "lead_s": 0.0 if nolead else 0.4}
    m = re.match(r"^noise-(v2|uni|wh)-(.+)$", label)
    if m:
        return {"case": "noise", "model_alias": m.group(1), "source_clip": m.group(2) + ".wav"}
    m = re.match(r"^(q\d+|edge|n\d+|b\d+)-(v2|wh)-(.+)$", label)
    if not m:
        die("unknown stress case %r" % label)
    case, model, name = m.groups()
    out = {"model_alias": model, "source_clip": name + ".wav"}
    if case.startswith("q"):
        out.update(case="quieter", atten_db=int(case[1:]), noise="pink", noise_dbfs=-62)
    elif case == "edge":
        out.update(case="edge", speech_peak_dbfs=-45, room_dbfs=-55)
    elif case.startswith("n"):
        out.update(case="noisy", noise="pink", noise_dbfs=-int(case[1:]))
    else:
        out.update(case="noisy", noise="babble", noise_dbfs=-int(case[1:]))
    return out


# ---------------------------------------------------------------- idle app logs

IDLE_RE = re.compile(r"^(.+?)\s+pid \d+ over\s+([\d.]+)s: wakeups\s+([\d.]+)/s, idle-exit wakeups\s+([\d.]+)/s, "
                     r"cpu\s+([\d.]+)% of a core, footprint\s+([\d.]+) MB, resident\s+([\d.]+) MB")
IDLE_STATES = [  # (study, log, measured label) -> build, state
    ("idle-app", "idle/idle-measure.log", "base loaded ~1 min", "32b1af7", "loaded-1min"),
    ("idle-app", "idle/idle-measure.log", "base loaded ~4.5 min (keep-alive over)", "32b1af7", "loaded-4.5min"),
    ("idle-app", "idle/idle-measure.log", "base + NO_RESIDENCY via open --env", "32b1af7+no_residency", "loaded"),
    ("idle-app", "idle/app-idle-new.log", "new loaded", "d33ab36", "loaded"),
    ("idle-app", "idle/app-idle-new.log", "new unloaded", "d33ab36", "unloaded"),
    ("settings-idle", "idle/warm-idle.log", "warm loaded", "8f3f0b3-warm", "loaded"),
    ("settings-idle", "idle/new-idle.log", "new loaded", "8f3f0b3", "loaded"),
    ("settings-idle", "idle/new-idle.log", "new unloaded", "8f3f0b3", "unloaded"),
    ("settings-idle", "idle/new-idle.log", "new unloaded, after an edit", "8f3f0b3", "unloaded-after-edit"),
    ("settings-idle", "idle/base-idle.log", "base loaded", "ffa49a5", "loaded"),
    ("settings-idle", "idle/base-idle.log", "base unloaded", "ffa49a5", "unloaded"),
]


def idle_rows(S1, S2):
    rows = []
    for study, log, label, build, state in IDLE_STATES:
        root = S2 if study == "idle-app" else S1
        path = os.path.join(root, log)
        if not os.path.exists(path):
            continue
        for line in open(path, errors="replace"):
            m = IDLE_RE.match(line.strip())
            if m and m.group(1).strip() == label:
                g = m.groups()
                rows.append({"study": study, "build": build, "state": state,
                             "window_s": r(float(g[1]), 1), "wakeups_per_s": r(float(g[2]), 1),
                             "idle_exit_wakeups_per_s": r(float(g[3]), 1), "cpu_pct": r(float(g[4]), 3),
                             "footprint_mb": r(float(g[5]), 1), "resident_mb": r(float(g[6]), 1)})
    return rows


# ---------------------------------------------------------------- main

def cell(v):
    if v is None:
        return ""
    if isinstance(v, float) and v.is_integer():
        return int(v)
    return v


def write_csv(out_dir, name, cols, rows):
    for row in rows:
        for k, v in row.items():
            if k not in cols:
                die("%s: column %s not declared" % (name, k))
            if isinstance(v, str) and not CELL_RE.match(v) and not (k == "section" and v in SECTIONS):
                die("%s: cell %r in column %s is not a label" % (name, v, k))
    path = os.path.join(out_dir, name)
    with open(path, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=cols, lineterminator="\n")
        w.writeheader()
        for row in rows:
            w.writerow({k: cell(row.get(k)) for k in cols})
    return os.path.getsize(path), len(rows)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--s1", required=True, help="scratch folder of the 9 Oct evening to 10 Oct runs")
    ap.add_argument("--s2", required=True, help="scratch folder of the 9 Oct daytime runs")
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.dirname(os.path.dirname(here))
    ap.add_argument("--out", default=os.path.join(repo, "docs/bench"))
    ap.add_argument("--testdata", default=os.path.join(repo, "testdata"))
    ap.add_argument("--recordings", default=os.path.join(repo, "recordings"))
    a = ap.parse_args()
    S1, S2 = a.s1, a.s2
    for d in (S1, S2):
        if not os.path.isdir(d):
            die("no such folder: %s" % d)
    os.makedirs(a.out, exist_ok=True)

    runs, benches, apps = enumerate_runs(S1, S2)
    seen = set()
    for x in runs + apps:
        if x["run_id"] in seen:
            die("duplicate run id %s" % x["run_id"])
        seen.add(x["run_id"])

    timelines = {}

    def timeline(key):
        if key not in timelines:
            path = {"ramble-v2": os.path.join(S1, "realvoice/runs/r0-v2.err"),
                    "ramble-v3": os.path.join(S1, "v3/runs/r0-v3.err"),
                    "ramble-v2-0910": os.path.join(S1, "v3/runs/r0-v2.err"),
                    "lf_3m-v2": os.path.join(S1, "whisper/tl/lf_3m.words"),
                    "lf_10m-v2": os.path.join(S1, "whisper/tl/lf_10m.words")}[key]
            if not os.path.exists(path):
                timelines[key] = None
            else:
                timelines[key] = timeline_word_lines(path) if path.endswith(".err") else timeline_transcribe(path)
        return timelines[key]

    run_rows, mem, energy, power, traces, cpu_min, passes, cuts = [], [], [], [], [], [], [], []
    lag_q, lag_h, starts, stress = [], [], [], []
    done_traces = set()
    for run in runs:
        d = session_dict(run)
        if d is None or "finalize_ms" not in d:
            continue
        row = run_row(run, d)
        rid = run["run_id"]
        # GPU utilization while talking (ioreg "Device Utilization %", one sample a second).
        if run.get("gpu"):
            skip = 2 if run["study"].startswith("idle") else (0 if run["study"] == "onepipe" else 3)
            row["gpu_util_pct"] = r(gpu_mean(run["gpu"], skip), 1)
        if run.get("other"):
            o = ints(run["other"])
            row["other_cpu_pct"] = r(statistics.mean(o), 1) if o else None
            row["build_overlap_s"] = sum(1 for line in open(run["other"], errors="replace") if line.strip() == "xcodebuild")
        # Energy (macmon).
        meta = load_json(run["meta"]) if run.get("meta") else None
        if meta and run.get("macmon"):
            samples = macmon(run["macmon"])
            e, (s0, s1) = energy_meta(samples, meta, d, run.get("ps"))
            row["gpu_util_pct"] = e.pop("gpu_util_pct")
            row["other_cpu_pct"] = e.pop("other_cpu_pct")
            energy.append(dict(run_id=rid, **e))
            power += [dict(run_id=rid, **b) for b in power_bins(samples, s0, s1)]
        run_rows.append(row)
        # Memory.
        if run["idx"] is None:
            mem += memory_rows(rid, d, (("mem_before_session", "loaded"), ("mem_end", "end")))
            if run.get("err") and run["err"].endswith(".stderr"):
                pk = time_peak_mb(run["err"])
                if pk:
                    mem.append({"run_id": rid, "phase": "time_peak", "peak_footprint_mb": r(pk, 1)})
        # CPU and GPU traces.
        tid = run.get("trace_id", rid)
        tg, tsmp = run.get("trace_gpu", run.get("gpu")), run.get("trace_samples", run.get("samples"))
        if tid not in done_traces and (tg or tsmp or run.get("other")):
            done_traces.add(tid)
            traces += trace_rows(tid, tg, run.get("other"), tsmp)
            if tsmp:
                cpu_min += [{"run_id": tid, "minute": m, "cpu_pct": r(v, 1), "method": "ps"}
                            for m, v in cpu_minutes_ps(tsmp)]
        err = run.get("err")
        if not err or not os.path.exists(err):
            err = None
        # Per-pass decode times (the scratch bench's PASS and CHUNK lines).
        if err:
            ps_, stamps = pass_lines(err)
            for x in ps_:
                passes.append(dict(run_id=rid, **{k: (r(v, 1 if k == "decode_ms" else 2) if isinstance(v, float)
                                                      else v) for k, v in x.items()}))
            if len(stamps) > 2:
                cpu_min += [{"run_id": rid, "minute": m + 1, "cpu_pct": r(v, 1), "method": "cputime-stamps"}
                            for m, v in enumerate(cpu_per_minute_stamps(stamps, d["cpu"]["wall_secs"]))]
            for i, x in enumerate(cut_lines(err)):
                cuts.append(dict(run_id=rid, index=i, **x))
        # Preview lag, against a one-pass decode's word timings.
        clip = row["clip"]
        key = None
        if clip == "ramble.wav":
            key = "ramble-v2"
            if run["study"] == "v3":
                key = "ramble-v3" if row["model"].endswith("v3-q5_k_m-gguf") else "ramble-v2-0910"
        elif clip in ("lf_3m.wav", "lf_10m.wav"):
            key = clip[:-4] + "-v2"
        if err and key and (row["passes"] or 0) > 0 and (row["chunks"] or 0) > 0 and timeline(key):
            lags, unmatched, n_tl = preview_lags(err, timeline(key))
            if lags:
                vals = [l for _, l in lags]
                groups = [("all", vals)]
                mins = int(max(e for e, _ in lags) // 60) + 1
                for m in range(mins):
                    v = [l for e, l in lags if m * 60 <= e < (m + 1) * 60]
                    if v:
                        groups.append((str(m + 1), v))
                for g, v in groups:
                    q = {"run_id": rid, "timeline": key + "-onepass", "minute": g, "words": len(v),
                         "timeline_words": n_tl if g == "all" else None,
                         "unmatched_updates": unmatched if g == "all" else None}
                    for qq in range(5, 100, 5):
                        q["p%02d_s" % qq] = r(pct_rank(v, qq / 100), 2)
                    q["max_s"] = r(max(v), 2)
                    q["mean_s"] = r(statistics.mean(v), 3)
                    lag_q.append(q)
                hist = {}
                for l in vals:
                    b = math.floor(l * 10 + 1e-9) / 10
                    hist[b] = hist.get(b, 0) + 1
                lag_h += [{"run_id": rid, "lag_bin_s": r(b, 1), "words": n} for b, n in sorted(hist.items())]
        # Real dictation starts.
        if run["study"] in ("starts", "starts-20db", "firstpass-sweep-real") and clip:
            m = re.match(r"^([rs]\d+(?:\.\d+)?)\.wav$", clip)
            if m:
                onset = first_loud(os.path.join(S1, "engine2/starts", clip))
                fw = row["first_words_ms"]
                starts.append({"run_id": rid, "study": run["study"], "build": run["build"], "model": row["model"],
                               "start": m.group(1), "onset_s": r(onset, 2), "first_words_ms": fw,
                               "first_after_onset_s": r(fw / 1000 - onset, 3) if fw is not None and onset is not None
                               else None})
        if run["study"] == "stress":
            c = stress_case(run["label"])
            name = c["source_clip"]
            if c["case"] == "soft":
                src = os.path.join(S1, "engine2/words", name)
            elif name.startswith("start_"):
                src = os.path.join(S1, "engine2/starts", name[6:])
            elif name == "structured.wav":
                src = os.path.join(a.recordings, "structured.wav")
            elif c["case"] == "noise":
                src = None
            else:
                src = os.path.join(a.testdata, name)
            onset = first_loud(src, c.get("lead_s", 0.0)) if src else None
            fw = row["first_words_ms"]
            stress.append({"run_id": rid, "case": c["case"], "model": row["model"], "source_clip": name,
                           "atten_db": c.get("atten_db"), "noise": c.get("noise"), "noise_dbfs": c.get("noise_dbfs"),
                           "speech_peak_dbfs": c.get("speech_peak_dbfs"), "room_dbfs": c.get("room_dbfs"),
                           "lead_s": c.get("lead_s"), "speech_found": int(d.get("speech_onset_secs") is not None),
                           "text_kept": int((row["final_words"] or 0) > 0), "wer_pct": row["wer_pct"],
                           "onset_s": r(onset, 2),
                           "first_after_onset_s": r(fw / 1000 - onset, 3) if fw is not None and onset is not None
                           else None})

    # Bench runs (every test clip, N reps).
    clips, bench_rows = [], []
    for b in benches:
        j = load_json(b["path"])
        if not j or "clips" not in j:
            continue
        rid = "%s/%s" % (b["study"], b["label"])
        model = model_of(j)
        bench_rows.append({"run_id": rid, "study": b["study"], "build": b["build"],
                           "model": model, "reps": j.get("reps"), "ref_words": j.get("ref_words"),
                           "errors": j.get("errors"), "wer_pct": r(100 * j["wer_total"], 2) if "wer_total" in j else None,
                           "empty_speech_decodes": j.get("empty_speech_decodes"),
                           "failed_speech_decodes": j.get("failed_speech_decodes"),
                           "rtf_speech_clips": r(j.get("rtf_speech_clips"), 4),
                           "load_ms_first": r(j.get("load_ms_first_in_process"), 1),
                           "load_ms_warm": r(j.get("load_ms_warm_reload"), 1),
                           "first_decode_ms": r(j.get("first_decode_ms"), 1)})
        mem += memory_rows(rid, j.get("mem", {}), (("start", "start"), ("after_load", "loaded"),
                                                   ("after_decodes", "after_decodes"), ("idle_2s", "idle_2s"),
                                                   ("after_unload", "unloaded")))
        for c in j["clips"]:
            if not re.match(r"^[a-z0-9_]+$", c["id"]):
                die("unexpected clip id %r" % c["id"])
            hyp = c.get("hypothesis") or ""
            decs = c.get("decode_ms_all") or [c.get("decode_ms_median")]
            for i, dm in enumerate(decs):
                clips.append({"run_id": rid, "model": model, "clip": c["id"] + ".wav", "category": c.get("category"),
                              "rep": i + 1, "audio_s": r(c.get("audio_secs"), 2), "ref_words": c.get("ref_words"),
                              "errors": c.get("errors"), "wer_pct": r(100 * c["wer"], 2) if c.get("wer") is not None
                              else None, "rtf": r(c.get("rtf"), 4), "decode_ms": r(dm, 1),
                              "hyp_words": len(hyp.split()),
                              "distinct_hyps": len(c.get("hypotheses_distinct") or [hyp])})

    # App self-test runs: the level meter and the overlay animation.
    app_rows = []
    for x in apps:
        j = load_json(x["path"])
        if not j:
            continue
        settings = []
        if "meter_interval_ms" in j:
            settings.append("meter_interval_ms=%d" % j["meter_interval_ms"])
        if "overlay_frame_ms" in j:
            settings.append("overlay_frame_ms=%d" % j["overlay_frame_ms"])
            if x["label"].split("/")[-1].startswith("off"):
                settings.append("overlay_animations=0")
        settings.append("draw=%d" % (1 if j.get("draw_each_redraw") else 0))
        audio = j.get("audio_secs") or float("nan")
        row = {"run_id": x["run_id"], "study": x["study"], "build": x["build"],
               "clip": clip_of(j), "settings": ";".join(settings), "audio_s": r(audio, 2),
               "cpu_pct": r(j.get("cpu_pct_of_one_core"), 3), "cpu_s": r(j.get("cpu_secs"), 2),
               "wall_s": r(j.get("wall_secs"), 2), "model_ready_ms": j.get("model_ready_ms"),
               "levels_received": j.get("levels_received"), "redraws_asked": j.get("redraws_asked"),
               "renders": j.get("renders"), "animation_frames": j.get("animation_frames"),
               "drawing_pct": r(j.get("drawing_pct_of_one_core"), 3),
               "drawing_ms_per_render": r(j.get("drawing_ms_per_render"), 3),
               "levels_per_s": r((j.get("levels_received") or 0) / audio, 2),
               "redraws_per_s": r((j.get("redraws_asked") or 0) / audio, 2),
               "renders_per_s": r((j.get("renders") or 0) / audio, 2)}
        app_rows.append(row)
        if x.get("macmon"):
            samples = macmon(x["macmon"])
            e, (s0, s1) = energy_app(samples, j)
            energy.append(dict(run_id=x["run_id"], **e))
            power += [dict(run_id=x["run_id"], **b) for b in power_bins(samples, s0, s1)]

    idle = idle_rows(S1, S2)

    E_COLS = ["run_id", "session_s", "audio_s", "j_per_min", "w_added", "w_cpu", "w_gpu", "w_ane", "w_dram",
              "j_per_min_plain", "j_per_min_sys", "w_idle_soc", "gpu_active_pct", "temp_start_c", "fan_max_rpm"]
    Q_COLS = ["run_id", "timeline", "minute", "words", "timeline_words", "unmatched_updates"] + \
        ["p%02d_s" % q for q in range(5, 100, 5)] + ["max_s", "mean_s"]
    used = sorted({x["study"] for x in run_rows + bench_rows + app_rows + idle})
    study_rows = [{"study": k, "section": STUDIES[k][0], "date": STUDIES[k][1]} for k in used]
    out = [
        ("studies.csv", ["study", "section", "date"], study_rows),
        ("runs.csv", RUN_COLS, run_rows),
        ("bench_runs.csv", ["run_id", "study", "build", "model", "reps", "ref_words", "errors",
                            "wer_pct", "empty_speech_decodes", "failed_speech_decodes", "rtf_speech_clips",
                            "load_ms_first", "load_ms_warm", "first_decode_ms"], bench_rows),
        ("clips.csv", ["run_id", "model", "clip", "category", "rep", "audio_s", "ref_words", "errors", "wer_pct",
                       "rtf", "decode_ms", "hyp_words", "distinct_hyps"], clips),
        ("memory.csv", ["run_id", "phase", "footprint_mb", "peak_footprint_mb"], mem),
        ("energy.csv", E_COLS, energy),
        ("power.csv", ["run_id", "t_s", "cpu_mw", "gpu_mw", "dram_mw", "gpu_active_pct"], power),
        ("traces.csv", ["run_id", "t_s", "cpu_pct", "rss_mb", "gpu_util_pct", "other_cpu_pct"], traces),
        ("cpu_minutes.csv", ["run_id", "minute", "cpu_pct", "method"], cpu_min),
        ("passes.csv", ["run_id", "phase", "t_s", "window_s", "decode_ms", "words", "new_confirmed", "cpu_s"],
         passes),
        ("cuts.csv", ["run_id", "index", "cut_at_s", "span_start_s", "span_end_s", "pending_audio_s", "wall_s",
                      "words_added", "decode_ms", "pause_start_s", "pause_end_s", "pause_s"], cuts),
        ("lag_quantiles.csv", Q_COLS, lag_q),
        ("lag_hist.csv", ["run_id", "lag_bin_s", "words"], lag_h),
        ("starts.csv", ["run_id", "study", "build", "model", "start", "onset_s", "first_words_ms",
                        "first_after_onset_s"], starts),
        ("stress.csv", ["run_id", "case", "model", "source_clip", "atten_db", "noise", "noise_dbfs",
                        "speech_peak_dbfs", "room_dbfs", "lead_s", "speech_found", "text_kept", "wer_pct", "onset_s",
                        "first_after_onset_s"], stress),
        ("app_runs.csv", ["run_id", "study", "build", "clip", "settings", "audio_s", "cpu_pct",
                          "cpu_s", "wall_s", "model_ready_ms", "levels_received", "redraws_asked", "renders",
                          "animation_frames", "drawing_pct", "drawing_ms_per_render", "levels_per_s",
                          "redraws_per_s", "renders_per_s"], app_rows),
        ("idle.csv", ["study", "build", "state", "window_s", "wakeups_per_s", "idle_exit_wakeups_per_s",
                      "cpu_pct", "footprint_mb", "resident_mb"], idle),
    ]
    total = 0
    for name, cols, rows in out:
        size, n = write_csv(a.out, name, cols, rows)
        total += size
        print("%-18s %6d rows %9d bytes" % (name, n, size))
    print("%-18s %16d bytes" % ("total", total))


if __name__ == "__main__":
    main()
