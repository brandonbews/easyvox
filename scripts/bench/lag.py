"""lag.py <session.err> <timeline>: how far the preview trails the voice (numbers only).

session.err is a session's stderr without --quiet (the preview's partial lines). The timeline is
a one-pass decode of the same recording with word timings: a session stderr with WORD lines (a
scratch copy of the bench printed each word of the final text with its start and end), or the
output of `transcribe --words`. For each word of the timeline, the first partial event whose
last three words match it; lag = event time - word end. Per minute: median, 95th percentile
and max. Also the longest gap between preview updates. scripts/bench/extract.py computes the
same lags for docs/bench/lag_*.csv."""
import os
import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from textutil import timed_words, transcribe_words, partials, norm, norm_word  # noqa: E402

sess, timeline = sys.argv[1], sys.argv[2]
words = timed_words(timeline) or transcribe_words(timeline)
S = [(t, s, e) for s, e, w in words for t in norm_word(w)]
toks = [t for t, _, _ in S]
ev = partials(sess)
shown = -1; reach = []  # (ms, index reached)
unmatched = 0
for ms, conf, prov in ev:
    P = norm(conf + " " + prov)
    if len(P) < 3:
        continue
    tail = P[-3:]; exp = len(P) - 1
    best = None
    for j in range(max(2, exp - 60), min(len(S), exp + 61)):
        sc = sum(1 for k in range(3) if toks[j - 2 + k] == tail[k])
        key = (sc, -abs(j - exp))
        if best is None or key > best[0]:
            best = (key, j)
    if best and best[0][0] >= 2:
        shown = max(shown, best[1])
    else:
        unmatched += 1
    reach.append((ms, shown))

lags = []  # (word end, lag)
k = 0
for i, (t, s, e) in enumerate(S):
    while k < len(reach) and reach[k][1] < i:
        k += 1
    if k == len(reach):
        break
    lags.append((e, reach[k][0] / 1000 - e))


def pct(v, q):
    v = sorted(v)
    return v[min(len(v) - 1, int(q * (len(v) - 1) + 0.5))] if v else float("nan")


gaps = [(b[0] - a[0]) / 1000 for a, b in zip(ev, ev[1:])]
print(f"events {len(ev)}, unmatched {unmatched}, timeline words {len(S)}, reached {len(lags)}; "
      f"longest gap between updates {max(gaps):.2f} s, gaps > 1.5 s: {sum(1 for g in gaps if g > 1.5)}")
mins = int(max(e for e, _ in lags) // 60) + 1 if lags else 0
allv = [l for _, l in lags]
print(f"  all: median {pct(allv, .5):.2f} p95 {pct(allv, .95):.2f} max {max(allv):.2f} s")
for m in range(mins):
    v = [l for e, l in lags if m * 60 <= e < (m + 1) * 60]
    if v:
        print(f"  min {m + 1}: n {len(v):3d} median {pct(v, .5):.2f} p95 {pct(v, .95):.2f} max {max(v):.2f} s")
