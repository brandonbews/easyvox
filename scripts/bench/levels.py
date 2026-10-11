"""levels.py WAV...: 30 ms frame levels (RMS, dBFS) of 16-bit mono WAVs. Numbers only."""
import array, math, sys, wave

FRAME = 480  # 30 ms at 16 kHz


def load(path):
    """16-bit PCM or 32-bit float, mono, any fmt tag (incl. WAVE_FORMAT_EXTENSIBLE)."""
    import struct
    b = open(path, "rb").read()
    assert b[:4] == b"RIFF" and b[8:12] == b"WAVE", path
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
    assert ch == 1, (path, ch)
    if tag == 1 and bits == 16:
        a = array.array("h", data[: len(data) // 2 * 2])
        return [x / 32768.0 for x in a], sr
    if tag == 3 and bits == 32:
        a = array.array("f", data[: len(data) // 4 * 4])
        return list(a), sr
    raise ValueError("unsupported %r in %s" % (fmt, path))


def levels(pcm):
    out = []
    for i in range(0, len(pcm), FRAME):
        c = pcm[i:i + FRAME]
        out.append(math.sqrt(sum(x * x for x in c) / len(c)))
    return out


def db(x):
    return 20 * math.log10(x) if x > 0 else -999.0


def pct(sorted_vals, p):
    if not sorted_vals:
        return float("nan")
    return sorted_vals[min(len(sorted_vals) - 1, int(p / 100 * (len(sorted_vals) - 1)))]


if __name__ == "__main__":
    print("file\tsecs\tsr\tzero_frames\tp10\tp50\tp90\tmax\tfirst>-50\tfirst>-45\tfirst>-40\tfirst>-35")
    for path in sys.argv[1:]:
        pcm, sr = load(path)
        lv = levels(pcm)
        nz = sorted(db(x) for x in lv if x > 1e-5)
        zero = sum(1 for x in lv if x <= 1e-5)
        firsts = []
        for th in (-50, -45, -40, -35):
            i = next((i for i, x in enumerate(lv) if db(x) > th), None)
            firsts.append("%.2f" % (i * 0.03) if i is not None else "-")
        print("%s\t%.2f\t%d\t%d\t%.1f\t%.1f\t%.1f\t%.1f\t%s" % (
            path.rsplit("/", 1)[-1], len(pcm) / sr, sr, zero, pct(nz, 10), pct(nz, 50), pct(nz, 90),
            nz[-1] if nz else -999, "\t".join(firsts)))
