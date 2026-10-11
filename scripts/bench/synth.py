"""synth.py TESTDATA RECORDINGS STARTS WORDS OUT [quiet|noisy|soft ...]: builds the stress clips
of docs/model-bench.md "Quiet mics and loud rooms" (16 kHz 16-bit mono WAVs). Numbers only.

TESTDATA is the repo's testdata/, RECORDINGS holds structured.wav, STARTS the WAVs cut at the
dictation starts (r012.6.wav and so on), WORDS the short words ("OK", "Yes" from `say`) and OUT
the folder for the clips. Clips made from a private recording are private too: keep OUT private.

  quiet:  speech attenuated by A dB, over pink noise at F dBFS (RMS)
  noisy:  the original speech over pink noise or babble at N dBFS
  soft:   a short command scaled so its loudest 30 ms frame sits at P dBFS, with 0.4 s of the
          floor before it and 0.5 s after (or none before), over pink noise at F dBFS
"""
import array, math, os, random, sys, wave
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from levels import load, levels

if len(sys.argv) < 6:
    sys.exit(__doc__)
T, R, STARTS, WORDS, OUT = sys.argv[1:6]
os.makedirs(OUT, exist_ok=True)


def write(name, pcm):
    w = wave.open(os.path.join(OUT, name + ".wav"), "wb")
    w.setnchannels(1); w.setsampwidth(2); w.setframerate(16000)
    w.writeframes(array.array("h", [max(-32768, min(32767, int(round(x * 32768)))) for x in pcm]).tobytes())
    w.close()


def rms(x):
    return math.sqrt(sum(v * v for v in x) / max(1, len(x)))


def pink(n, dbfs, seed):
    rnd = random.Random(seed)
    b0 = b1 = b2 = 0.0
    out = []
    for _ in range(n):
        w = rnd.uniform(-1, 1)
        b0 = 0.99765 * b0 + w * 0.0990460
        b1 = 0.96300 * b1 + w * 0.2965164
        b2 = 0.57000 * b2 + w * 1.0526913
        out.append(b0 + b1 + b2 + w * 0.1848)
    g = 10 ** (dbfs / 20) / rms(out)
    return [v * g for v in out]


_babble = None


def babble(n, dbfs):
    """Four text-to-speech voices talking over each other, looped."""
    global _babble
    if _babble is None:
        srcs = [load(os.path.join(T, c + ".wav"))[0] for c in ("long_daniel", "long_samantha", "punct_daniel", "names_tessa")]
        L = max(len(s) for s in srcs)
        mix = [0.0] * L
        for k, s in enumerate(srcs):
            off = k * 16000 * 3 // 2
            for i in range(L):
                mix[i] += s[(i + off) % len(s)]
        _babble = mix
    out = [_babble[i % len(_babble)] for i in range(n)]
    g = 10 ** (dbfs / 20) / rms(out)
    return [v * g for v in out]


def add(a, b):
    return [x + y for x, y in zip(a, b)]


def scale(pcm, db):
    g = 10 ** (db / 20)
    return [x * g for x in pcm]


def peak_frame(pcm):
    return 20 * math.log10(max(levels(pcm)))


clips = {"structured": os.path.join(R, "structured.wav")}
for c in ("cmd_daniel", "cmd_karen", "cmd_moira", "cmd_samantha", "num_rishi"):
    clips[c] = os.path.join(T, c + ".wav")
for f in sorted(x for x in os.listdir(STARTS) if x.endswith(".wav")):
    clips["start_" + f[:-4]] = os.path.join(STARTS, f)

which = sys.argv[6:] or ["quiet", "noisy", "soft"]
if "quiet" in which or "noisy" in which:
    for name, path in clips.items():
        pcm, _ = load(path)
        n = len(pcm)
        if "quiet" in which:
            floor = pink(n, -62, 1)
            for a in (10, 20, 25, 30):
                write("q%d-%s" % (a, name), add(scale(pcm, -a), floor))
            # The plausibility edge: the loudest frame at -45 dBFS, the floor 10 dB under it.
            write("edge-%s" % name, add(scale(pcm, -45 - peak_frame(pcm)), pink(n, -55, 2)))
        if "noisy" in which:
            for nd in (45, 40, 35):
                write("n%d-%s" % (nd, name), add(pcm, pink(n, -nd, 3)))
            write("b40-%s" % name, add(pcm, babble(n, -40)))
if "soft" in which:
    words = sorted(f for f in os.listdir(WORDS) if f.endswith(".wav"))
    for f in words:
        w, _ = load(os.path.join(WORDS, f))
        for p in (40, 45):
            for fl in (62, 55):
                if p == 40 and fl == 55:
                    continue
                s = scale(w, -p - peak_frame(w))
                lead, tail = int(0.4 * 16000), int(0.5 * 16000)
                body = [0.0] * lead + s + [0.0] * tail
                write("soft%d-f%d-%s" % (p, fl, f[:-4]), add(body, pink(len(body), -fl, 4)))
                write("soft%d-f%d-nolead-%s" % (p, fl, f[:-4]), add(s + [0.0] * tail, pink(len(s) + tail, -fl, 5)))
print(len(os.listdir(OUT)), "clips in", OUT)
