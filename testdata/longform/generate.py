#!/usr/bin/env python3
"""Regenerates the long-dictation test clips (1-9 minutes) with macOS `say`.

Same recipe as ../generate.sh: `say`, then afconvert to 16 kHz mono s16le WAV. Each paragraph of
paragraphs.txt is spoken on its own, then the pieces are joined with silence:
  lf_1m, lf_3m, lf_5m, lf_10m  Samantha reading the first 3, 8, 13 and all 22 paragraphs, 0.5 s
                               apart. Nested, so a paragraph can be compared across lengths.
  lf_3m_pauses                 Daniel reading paragraphs 1-7, with 2.5-8 s thinking pauses between
                               and inside paragraphs.

Output, in clips/ next to this script (gitignored) unless --out DIR:
  <id>.wav       the clip
  <id>.txt       its reference transcript, for `easyvox-bench session --ref`
  manifest.json  each clip's reference text, plus every piece's start and end time in the WAV
  para/          each piece alone (<voice>_<paragraph>[a|b].wav + .txt); kept as a cache and
                 re-spoken only when its text changes
manifest.tsv (id, voice, category, text) is written next to this script and committed. The
reference text depends only on paragraphs.txt and the clip lists below, never on the audio.

TTS speech is far cleaner than real dictation; treat WER numbers from it as a best case.
"""
import argparse, json, os, subprocess, sys, tempfile, wave

HERE = os.path.dirname(os.path.abspath(__file__))
SR = 16000
GAP = 0.5  # breath between paragraphs, seconds

# Samantha, nested prefixes of paragraphs.txt: (id, paragraph count). About 1, 3, 5 and 9 minutes.
NESTED = [("lf_1m", 3), ("lf_3m", 8), ("lf_5m", 13), ("lf_10m", 22)]

# Daniel as a speaker who stops to think: (paragraph, pause inside it after this phrase,
# that pause in seconds, pause after the paragraph in seconds).
RAMBLE = [
    (0, "before I forget them.", 3.0, GAP),
    (1, None, 0, 5.0),
    (2, "the support backlog,", 2.5, GAP),
    (3, "Another thing to bring up:", 4.0, 8.0),
    (4, None, 0, GAP),
    (5, "which feels impossible,", 3.5, 6.0),
    (6, None, 0, 0),
]


def plan(paras):
    """Every clip as (id, voice, category, pieces); a piece is (paragraph, text, name) or a pause in seconds."""
    clips = []
    for cid, n in NESTED:
        if n > len(paras):
            sys.exit(f"{cid} needs {n} paragraphs, paragraphs.txt has {len(paras)}")
        pieces = []
        for i in range(n):
            if pieces:
                pieces.append(GAP)
            pieces.append((i, paras[i], f"Samantha_{i:02d}"))
        clips.append((cid, "Samantha", "longform", pieces))
    pieces = []
    for i, after, mid, gap in RAMBLE:
        p = paras[i]
        if after:
            if after not in p:
                sys.exit(f"paragraph {i + 1} of paragraphs.txt no longer contains {after!r} (see RAMBLE)")
            k = p.index(after) + len(after)
            pieces += [(i, p[:k].strip(), f"Daniel_{i:02d}a"), mid, (i, p[k:].strip(), f"Daniel_{i:02d}b")]
        else:
            pieces.append((i, p, f"Daniel_{i:02d}"))
        if gap:
            pieces.append(gap)
    clips.append(("lf_3m_pauses", "Daniel", "longform_pauses", pieces))
    return clips


def write_text(path, text):
    with open(path, "w") as f:
        f.write(text + "\n")


def render(voice, text, wav, tmp):
    """`text` spoken by `voice` as 16 kHz mono s16le PCM, cached in `wav` (and its .txt) until the text changes."""
    txt = wav[:-4] + ".txt"
    cached = os.path.exists(wav) and os.path.exists(txt) and open(txt).read().rstrip("\n") == text
    if not cached:
        aiff = os.path.join(tmp, "x.aiff")
        subprocess.run(["say", "-v", voice, "-o", aiff, text], check=True)
        subprocess.run(["afconvert", "-f", "WAVE", "-d", "LEI16@16000", "-c", "1", aiff, wav], check=True)
        write_text(txt, text)
    with wave.open(wav) as w:
        assert (w.getframerate(), w.getnchannels(), w.getsampwidth()) == (SR, 1, 2), wav
        return w.readframes(w.getnframes())


def build(out, cid, voice, pieces, tmp):
    """Writes <out>/<cid>.wav; returns (pieces with start/end times, duration in seconds)."""
    data, segments = bytearray(), []
    for p in pieces:
        if not isinstance(p, tuple):
            data += b"\x00\x00" * int(p * SR)
            continue
        i, text, name = p
        start = len(data) / 2 / SR
        data += render(voice, text, os.path.join(out, "para", name + ".wav"), tmp)
        segments.append({"para": i, "text": text, "start": round(start, 3), "end": round(len(data) / 2 / SR, 3)})
    with wave.open(os.path.join(out, cid + ".wav"), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(bytes(data))
    return segments, round(len(data) / 2 / SR, 2)


def main():
    ap = argparse.ArgumentParser(description="Regenerates the long-dictation test clips with macOS `say`.")
    ap.add_argument("--out", default=os.path.join(HERE, "clips"), help="where the clips go (default: clips/ next to this script)")
    ap.add_argument("--text-only", action="store_true", help="write manifest.tsv and the <id>.txt references only, without running `say`")
    args = ap.parse_args()

    with open(os.path.join(HERE, "paragraphs.txt")) as f:
        paras = [line.strip() for line in f if line.strip()]
    os.makedirs(args.out if args.text_only else os.path.join(args.out, "para"), exist_ok=True)

    rows, manifest = [], []
    with tempfile.TemporaryDirectory() as tmp:
        for cid, voice, category, pieces in plan(paras):
            text = " ".join(p[1] for p in pieces if isinstance(p, tuple))
            assert "\t" not in text and "\n" not in text, cid
            rows.append(f"{cid}\t{voice}\t{category}\t{text}\n")
            write_text(os.path.join(args.out, cid + ".txt"), text)
            words, nparas = len(text.split()), len({p[0] for p in pieces if isinstance(p, tuple)})
            if args.text_only:
                print(f"{cid:14} {voice:9} {words:5d} words  paras {nparas}")
                continue
            segments, secs = build(args.out, cid, voice, pieces, tmp)
            manifest.append({"id": cid, "voice": voice, "wav": cid + ".wav", "secs": secs, "words": words,
                             "text": text, "segments": segments})
            print(f"{cid:14} {voice:9} {secs:7.1f}s {words:5d} words  paras {nparas}")

    with open(os.path.join(HERE, "manifest.tsv"), "w") as f:
        f.writelines(rows)
    if manifest:
        with open(os.path.join(args.out, "manifest.json"), "w") as f:
            json.dump(manifest, f, indent=1)


if __name__ == "__main__":
    main()
