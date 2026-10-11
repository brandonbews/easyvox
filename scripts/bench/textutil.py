"""Helpers for lag.py: parse a session's stderr and normalize words as the bench's scorer does.
The parsers return transcript text, in memory; callers print numbers only."""
import json, re

NUM = {w: str(i) for i, w in enumerate("zero one two three four five six seven eight nine ten eleven twelve "
                                         "thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty".split())}
NUM.update({"thirty": "30", "forty": "40", "fifty": "50", "sixty": "60", "seventy": "70", "eighty": "80", "ninety": "90",
            "dr": "doctor", "mr": "mister", "mrs": "missus"})


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


def align(a, b):
    """Levenshtein alignment of token lists a (reference) and b: list of (op, i, j)."""
    n, m = len(a), len(b)
    d = [[0] * (m + 1) for _ in range(n + 1)]
    for i in range(n + 1):
        d[i][0] = i
    for j in range(m + 1):
        d[0][j] = j
    for i in range(1, n + 1):
        ai = a[i - 1]; row = d[i]; prev = d[i - 1]
        for j in range(1, m + 1):
            c = prev[j - 1] + (ai != b[j - 1])
            row[j] = min(c, prev[j] + 1, row[j - 1] + 1)
    ops = []; i, j = n, m
    while i > 0 or j > 0:
        if i > 0 and j > 0 and d[i][j] == d[i - 1][j - 1] + (a[i - 1] != b[j - 1]):
            ops.append(("=" if a[i - 1] == b[j - 1] else "S", i - 1, j - 1)); i -= 1; j -= 1
        elif i > 0 and d[i][j] == d[i - 1][j] + 1:
            ops.append(("D", i - 1, None)); i -= 1
        else:
            ops.append(("I", None, j - 1)); j -= 1
    return ops[::-1]


def timed_words(err_path):
    """WORD lines from a session's stderr: [(start, end, text)]."""
    out = []
    for line in open(err_path, errors="replace"):
        if line.startswith("WORD "):
            _, s, e, t = line.rstrip("\n").split(" ", 3)
            out.append((float(s), float(e), t))
    return out


def transcribe_words(path):
    """`transcribe --words` stderr lines: '  start   end  text'."""
    out = []
    for line in open(path, errors="replace"):
        m = re.match(r"^\s*(-?\d+\.\d+)\s+(-?\d+\.\d+)\s{2}(.*)$", line.rstrip("\n"))
        if m:
            out.append((float(m.group(1)), float(m.group(2)), m.group(3)))
    return out


def cuts(err_path):
    out = []
    for line in open(err_path, errors="replace"):
        if line.startswith("CUT at"):
            m = re.search(r"CUT at ([\d.]+) s \(span ([\d.]+)\.\.([\d.]+), audio ([\d.]+) s, wall ([\d.]+) s\) words \+(\d+) decode (\d+) ms", line)
            out.append({"at": float(m.group(1)), "span": (float(m.group(2)), float(m.group(3))), "audio": float(m.group(4)),
                        "wall": float(m.group(5)), "words": int(m.group(6)), "decode_ms": int(m.group(7))})
        elif line.startswith("CUTDBG"):
            out.append({"dbg": line.strip()})
    return out


def partials(err_path):
    """Partial lines: [(ms, confirmed, provisional)]."""
    out = []
    for line in open(err_path, errors="replace"):
        m = re.match(r"^\s*(\d+)ms  (.*)$", line.rstrip("\n"))
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
