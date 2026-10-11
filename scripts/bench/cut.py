"""cut.py SRC OUT START-END [START-END ...]: concatenate stretches (seconds) of SRC into a 16 kHz
16-bit mono WAV. Numbers only."""
import array, sys, wave
sys.path.insert(0, __file__.rsplit("/", 1)[0])
from levels import load

pcm, sr = load(sys.argv[1])
assert sr == 16000
out = []
for span in sys.argv[3:]:
    a, b = (float(x) for x in span.split("-"))
    out.extend(pcm[int(a * sr):int(b * sr)])
w = wave.open(sys.argv[2], "wb")
w.setnchannels(1); w.setsampwidth(2); w.setframerate(sr)
w.writeframes(array.array("h", [max(-32768, min(32767, int(round(x * 32768)))) for x in out]).tobytes())
w.close()
print("%s: %.2f s" % (sys.argv[2].rsplit("/", 1)[-1], len(out) / sr))
