"""agree.py A.json B.json [...]: how far apart the final transcripts of bench session results
are, without printing any of their words. For each B: word edit distance to A over A's word
count (lowercase alphanumerics, like the bench's WER), plus the word counts. Use it on the
private recordings, where there's no reference: it prints numbers only.

  python3 -I agree.py runs/C0-r1.json runs/C-r1.json runs/C2-r1.json
"""
import json, sys


def words(path):
    text = json.load(open(path)).get("final_text", "")
    out = []
    for w in text.split():
        w = "".join(ch for ch in w.lower() if ch.isalnum())
        if w:
            out.append(w)
    return out


def edits(a, b):
    prev = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        cur = [i] + [0] * len(b)
        for j, y in enumerate(b, 1):
            cur[j] = min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (x != y))
        prev = cur
    return prev[-1]


a = words(sys.argv[1])
for path in sys.argv[2:]:
    b = words(path)
    e = edits(a, b)
    print("%s vs %s: words %d / %d, edits %d, disagreement %.2f %%" % (
        sys.argv[1].rsplit("/", 1)[-1], path.rsplit("/", 1)[-1], len(a), len(b), e, 100.0 * e / max(len(a), 1)))
