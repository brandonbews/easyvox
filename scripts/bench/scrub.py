"""scrub.py FILE.json ...: remove transcript text from bench result files in place, once the
analysis (agree.py) no longer needs it. Keeps every number. Prints only file names and counts."""
import json, sys

TEXT_KEYS = {"final_text", "confirmed_at_stop", "text", "hypothesis", "hypotheses_distinct", "reference"}


def strip(x):
    if isinstance(x, dict):
        return {k: strip(v) for k, v in x.items() if k not in TEXT_KEYS}
    if isinstance(x, list):
        return [strip(v) for v in x]
    return x


n = 0
for path in sys.argv[1:]:
    if path.endswith(".meta.json"):
        continue
    data = json.load(open(path))
    json.dump(strip(data), open(path, "w"), indent=1)
    n += 1
print("scrubbed %d files" % n)
