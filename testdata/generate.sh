#!/bin/bash
# Regenerates the TTS test clips in this directory with macOS `say`.
# Output: <id>.wav (16 kHz mono s16le) + manifest.tsv (id <TAB> voice <TAB> category <TAB> text).
# TTS speech is far cleaner than real dictation; treat WER numbers from it as a best case.
set -euo pipefail
cd "$(dirname "$0")"
tmp=$(mktemp -d)
: > manifest.tsv

clip() { # id voice category text
  local id=$1 voice=$2 cat=$3 text=$4
  say -v "$voice" -o "$tmp/$id.aiff" "$text"
  afconvert -f WAVE -d LEI16@16000 -c 1 "$tmp/$id.aiff" "$id.wav"
  printf '%s\t%s\t%s\t%s\n' "$id" "$voice" "$cat" "$text" >> manifest.tsv
}

# ~2-4 s short commands
clip cmd_samantha  "Samantha" command "Send a message to Sarah saying I'll be ten minutes late."
clip cmd_daniel    "Daniel"   command "Open the calendar and schedule a meeting for Tuesday afternoon."
clip cmd_karen     "Karen"    command "Remind me to buy milk, eggs, and bread on the way home."
clip cmd_moira     "Moira"    command "Turn off the lights in the kitchen."
# numbers
clip num_samantha  "Samantha" numbers "The invoice total is 342 dollars, and it is due on March 15."
clip num_rishi     "Rishi"    numbers "Call me back at 555 0142 before 6 o'clock tonight."
clip num_daniel    "Daniel"   numbers "We sold 1,250 units in the first quarter, up 12 percent from last year."
# names
clip names_tessa   "Tessa"    names   "Please forward the report to Priya Raman, Joaquin Alvarez, and Siobhan O'Connor."
clip names_eddy    "Eddy (English (US))" names "Ask Doctor Nakamura whether Mister Kowalski can join the call with Ingrid on Thursday."
# punctuation-heavy, ~8-12 s
clip punct_samantha "Samantha" punctuation "Well, here's the thing: if we ship on Friday, and nobody objects, we'll have the whole weekend to fix bugs. Otherwise, we wait until next Wednesday. Does that sound reasonable?"
clip punct_daniel  "Daniel"   punctuation "Hi Tom, thanks for the quick reply! I've attached the draft; let me know what you think, especially about the second section."
# ~30 s passage
clip long_samantha "Samantha" passage "Good morning everyone. I wanted to give a quick update on the project before our planning meeting. Over the past two weeks, the team finished the new onboarding flow, fixed most of the crashes that users reported after the last release, and started testing the offline mode on older devices. We are still waiting on the design review for the settings screen, which should happen early next week. The biggest risk right now is the database migration, because it touches almost every table and we have not tested it with a full copy of production data. If that goes well, we can ship the update by the end of the month."
clip long_daniel   "Daniel"   passage "The old lighthouse stood at the edge of the cliff for nearly 200 years. Every night, the keeper climbed the narrow stairs, cleaned the great glass lens, and lit the lamp that guided ships away from the rocks below. When the storms came in from the west, he would stay awake until dawn, listening to the wind and watching the dark water for any sign of trouble. Most of the sailors who passed never knew his name, but many of them owed him their lives. Today the light is automatic, and the keeper's cottage has become a small museum."

# 2 s of digital silence (hallucination check). Expected text: empty.
python3 -I - "silence_2s.wav" <<'PY'
import sys, wave
w = wave.open(sys.argv[1], "wb"); w.setnchannels(1); w.setsampwidth(2); w.setframerate(16000)
w.writeframes(b"\x00\x00" * 32000); w.close()
PY
printf '%s\t%s\t%s\t%s\n' "silence_2s" "-" "silence" "" >> manifest.tsv

rm -rf "$tmp"
for f in *.wav; do printf '%s\t%s\n' "$f" "$(afinfo "$f" | awk '/estimated duration/ {print $3}')"; done
