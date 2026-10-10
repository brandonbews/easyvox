# Settings

EasyVox keeps its settings in one TOML file, so you can set it up from a text editor, your dotfiles or a script, as well as from its Settings window. Everything is in the file, including the advanced numbers the window doesn't show: how long the model stays loaded, and how the live preview, the chunks and the pauses work.

## Where the file is

- `~/.config/easyvox/config.toml`, on macOS too (not `~/Library/Preferences`).
- `$XDG_CONFIG_HOME/easyvox/config.toml` instead, when `XDG_CONFIG_HOME` is set to an absolute path.
- `EASYVOX_CONFIG=/some/file.toml` points EasyVox at any other file, for tests or a second setup: `open --env EASYVOX_CONFIG=/tmp/test.toml dist/EasyVox.app`.

The file doesn't have to exist: without it, everything is at its default. EasyVox creates the folder when it starts (so it can watch it), and the file the first time you change something in the Settings window. A symlink is fine, say into a dotfiles repository: EasyVox writes through it and watches where it points.

The file holds no permissions. Microphone, Accessibility and Input Monitoring are granted in System Settings, by a person.

## How it works

- **The defaults live in the app; the file only overrides them.** Any key you leave out follows its default, so when a release improves a default, you get it. Set only what you want different, and don't paste the whole reference below into your file: every key you copy stays at that value from then on.
- **The top of the file is what the Settings window shows:** `model`, `shortcut`, `hold_right_option` and `keep_model_loaded`. When you change one in the window, EasyVox writes that key and nothing else. Your comments, your order and your other keys stay as you wrote them.
- **`[advanced]` holds everything else.** EasyVox never writes these: they're yours to set by hand.
- **Edits apply within a second of saving,** with no restart. EasyVox watches the file (with FSEvents, so it costs nothing while the file doesn't change). A new shortcut or model takes effect at once. The numbers for the live preview, the chunks and the pauses apply from your next dictation, without reloading the model; a dictation already under way finishes with the values it started with.
- **The same keys apply to every model.** There's one pipeline, so there are no per-model sections.

## When something's wrong

EasyVox never stops over a bad setting, and never overwrites your file to fix it:

- **A value out of range, or of the wrong type** (`preview_interval_ms = 60000`, `keep_model_loaded = "yes"`): that key falls back to its default, and the others still apply.
- **A model the app doesn't offer**: an unknown id, or one that's for `easyvox-bench` only, like Parakeet Unified since 10 October 2026. EasyVox uses the default model (Parakeet TDT v2) and says why.
- **A key EasyVox doesn't know**, maybe misspelt (`previw_interval_ms`), or in the wrong place (an `[advanced]` key at the top, or a top key after `[advanced]`): it's ignored, and the warning says so (suggesting the key you probably meant, when one is close).
- **A file that isn't valid TOML:** EasyVox keeps the settings it had (the defaults, if it was just starting) and leaves the file alone until you fix it. Changes you make in the Settings window meanwhile apply, but aren't saved.

Problems show under **Settings → General → Settings file**, and in the log (stderr: `open --stderr /tmp/easyvox.log dist/EasyVox.app`).

## Loudness is in decibels

Six keys say how loud is "quiet" or "speech": `gate_quiet_db` and `pause_quiet_db` compare with your own speech, `speech_above_noise_db` with the room, and `gate_silence_dbfs`, `pause_silence_dbfs` and `speech_loud_dbfs` with the loudest sound a microphone can record.

- **dB** (`_db`) is relative to another level. -20 dB is a tenth of it, -6 dB about half, 0 dB the same, and 20 dB ten times it.
- **dBFS** (`_dbfs`) is relative to full scale, the loudest a sample can be (0 dBFS). In the recordings EasyVox was tuned on (AirPods in an office), speech is about -17 dBFS and the room about -50 dBFS.

The `_dbfs` keys are always 0 or below, and so are the two `_quiet_db` keys; `speech_above_noise_db` is 0 or above, since speech is louder than the room. Closer to 0 dBFS means louder. Before October 2026 the `gate_*` and `pause_*` levels were linear (0.1 and 0.0018); the defaults moved to -20 dB and -55 dBFS, and behave the same on every test clip.

## Examples

Use another shortcut and keep the model in memory (the Settings window can do both too):

```toml
shortcut = "ctrl-shift-space"
keep_model_loaded = true
```

A livelier preview, at some cost in battery: a pass every 0.8 s instead of 1.6 s, each re-decoding a bit more of the sentence.

```toml
[advanced]
preview_interval_ms = 800
preview_context_ms = 2000
```

Free the model after 2 minutes instead of 10, and leave the final text up longer:

```toml
[advanced]
unload_after_secs = 120
overlay_hide_after_final_ms = 1500
```

A loud room, where the preview keeps decoding through your pauses: count less as speech.

```toml
[advanced]
gate_quiet_db = -14
gate_silence_dbfs = -45
```

A recording with no speech in it gives no text: nothing is decoded, nothing is pasted, and the overlay says it didn't catch anything. The check errs towards decoding, but if it ever happens to you while you did speak (a whisper, or a microphone turned right down in a noisy room), count fainter sounds as speech. `speech_min_ms = 0` turns the check off, and decodes every recording.

```toml
[advanced]
speech_above_noise_db = 3
speech_loud_dbfs = -45
```

## Every key

Each key, at its default, with its unit, its range and what raising or lowering it trades off. The app prints the same text: `easyvox --print-default-config` (in a build, `dist/EasyVox.app/Contents/MacOS/easyvox --print-default-config`; from source, `cargo run -p easyvox-app -- --print-default-config`). A test keeps this page in step with the code.

```toml
# EasyVox settings: every key, at its default.
#
# This is a reference, not a file to copy whole. Your settings file only needs the keys you
# change: anything you leave out follows the default, so a release that improves one reaches
# you. The file is ~/.config/easyvox/config.toml ($XDG_CONFIG_HOME/easyvox/config.toml when
# that's set; EASYVOX_CONFIG points at another file). EasyVox applies a change as soon as you
# save it: the [advanced] numbers for the live preview, the chunks and the pauses from your
# next dictation, the same for every model.

# The speech model: the id of one the app offers (`scripts/fetch-model.sh --list` marks them
# default or alternative). Settings → Models downloads it, and the next dictation uses it.
model = "parakeet-tdt-0.6b-v2-q5_k_m-gguf"

# The push-to-talk shortcut: hold it and speak, let go to paste. Modifiers (ctrl, alt, shift,
# cmd) and a key, joined by "-", e.g. "ctrl-shift-space" or "f5".
shortcut = "alt-space"

# Also dictate while the right Option (⌥) key is held on its own. Needs Input Monitoring.
hold_right_option = false

# Keep the speech model in memory while you're not dictating. Off, it's freed after
# `unload_after_secs` (below) and loads again as your next dictation starts.
keep_model_loaded = false

[advanced]
# Settings the window doesn't show: how long the overlay stays up and the model stays
# loaded, then the numbers behind the live preview, the chunks, and what counts as
# speech or a pause.

# How long EasyVox keeps the speech model in memory after your last dictation, unless
# "Keep model loaded" is on. Freeing it takes the app from about 670 MB to about
# 90 MB, and the next dictation still records at once while the model loads again
# (about 0.15 s). Lower frees the memory sooner but reloads more often; higher keeps
# it ready longer.
# In seconds, from 1 to 86400.
unload_after_secs = 600

# How long the overlay stays up after the final text is pasted. Higher leaves more time
# to read it; lower gets it out of the way sooner. 0 hides it at once.
# In milliseconds, from 0 to 10000.
overlay_hide_after_final_ms = 600

# How long a notice (no microphone access, nothing heard, the model still loading)
# stays up. Higher leaves more time to read it.
# In milliseconds, from 500 to 30000.
overlay_hide_after_notice_ms = 2500

# The most text the overlay shows: longer text keeps only its end, so the newest words
# stay in view. Higher shows more of what came before, but past about 72 the newest
# words can run past the edge of the overlay and be cut off.
# In characters, from 10 to 500.
overlay_max_chars = 72

# How often the preview re-decodes the newest audio while the key is held. 0 turns the
# preview off (long dictations are still polished in chunks). Lower shows words sooner
# but wakes the GPU more often; higher is lighter, and the preview lags further behind
# and updates in bigger steps. At 1600, 95 % of words showed up within about 2 s of
# being said; past it the saving flattens while the lag keeps growing
# (`docs/model-bench.md`, "Lightness").
# In milliseconds, from 0 to 10000.
preview_interval_ms = 1600

# When the first preview pass runs: once this much audio has followed the start of
# speech (see `speech_above_noise_db`). Until the preview shows words, the next passes
# come as often (at least 100 ms apart), then every `preview_interval_ms`. Lower shows
# the first words sooner, but a first pass with too little speech shows nothing and
# costs another; higher keeps you waiting for the first words. It's a pass or two per
# dictation, so it costs almost nothing.
# In milliseconds, from 0 to 10000.
preview_first_ms = 500

# The most recent audio one preview pass decodes, at most. A pass normally starts just
# before the confirmed words (`preview_context_ms`), so it's usually much shorter; this
# only caps it while passes keep disagreeing and nothing gets confirmed. Lower makes
# those passes cheaper, but words that fall out of the window before two passes
# agreed on them leave the preview until their chunk is polished. At least 2000.
# In milliseconds, from 2000 to 120000.
preview_window_ms = 20000

# Audio before the confirmed words (the ones two passes agreed on) that each preview
# pass decodes again, as context for the words after them. A pass starts there, or at
# the last chunk cut if that's later, so confirmed words aren't decoded over and over:
# the preview's GPU energy follows the audio it decodes. Higher gives the newest words
# more of their sentence but makes every pass longer (2000 took about 14 % more GPU
# energy than 1000, for much the same preview); lower is lighter. The final text
# doesn't depend on it. 120000 decodes everything since the last cut, much as the
# preview did before this setting existed.
# In milliseconds, from 0 to 120000.
preview_context_ms = 1000

# Audio needed before the first preview pass, counted from the start of the recording
# (the pass also waits for `preview_first_ms` of speech). Lower can show the first word
# a moment sooner, but spends a pass on too little audio to decode.
# In milliseconds, from 0 to 5000.
preview_min_audio_ms = 300

# Rest between two preview passes when they can't keep up with `preview_interval_ms`
# (a pass, or a chunk polished in the same tick, took longer than the interval). Higher
# lets a busy GPU breathe; the preview then falls further behind.
# In milliseconds, from 0 to 5000.
preview_min_gap_ms = 50

# Local agreement: a pass's first new word that starts within this much of the last
# confirmed word's end is checked for being a re-decoded copy of the confirmed words
# (word timings move by a frame or two between passes). Higher catches more doubled
# words in the preview, but can drop a word that really was said twice ("very, very").
# In milliseconds, from 0 to 2000.
agreement_repeat_ms = 300

# Local agreement: the longest run of words checked for such a copy.
# In words, from 0 to 20.
agreement_max_repeat_words = 5

# Speech gate: preview passes that still run after speech stops, so the last words are
# decoded with the silence after them and confirmed by two passes that agree. Then the
# preview rests until the next word. 0 rests at once, and the last words may stay
# provisional until release; higher spends more passes on every pause.
# In passes, from 0 to 1000.
gate_settle_passes = 2

# Speech gate: new audio counts as speech (worth a preview pass) when a 30 ms frame of
# it is louder than this, relative to the loudest speech so far (-20 is 20 dB under:
# a tenth of its level). Higher (nearer 0) lets the preview rest in more pauses over
# room noise, but may skip soft words until a louder one comes; lower errs towards
# decoding. The final text decodes everything either way.
# In dB (relative to another level), from -60 to 0.
gate_quiet_db = -20

# Speech gate: ...and louder than this. Higher ignores more background noise; lower
# counts fainter sounds as speech. Nothing at or under it counts as speech for
# `speech_above_noise_db` either.
# In dBFS (decibels relative to full scale), from -120 to 0.
gate_silence_dbfs = -55

# Speech detection: a 30 ms frame counts as speech when it's this much louder than the
# room, which is the recording's quietest frames (their 10th percentile, once there's
# 0.3 s of sound; digital silence doesn't count). Until a recording holds speech,
# nothing is decoded: no preview, no final pass. So a recording of silence or room
# noise gives empty text, where a model could make up words (Whisper wrote "you" for
# 2 s of silence). Missing speech would lose the whole dictation, so this errs low: 6
# still catches a soft voice whose loudest moments are only 10 dB over the room, and
# a steady room stays under it. Higher ignores louder noises (a breath, a chair), but
# may miss a soft voice; lower takes fainter sounds for speech.
# In dB (relative to another level), from 0 to 60.
speech_above_noise_db = 6

# Speech detection: ...or louder than this, however loud the room. It catches speech
# that starts at once, before any quiet frames show how loud the room is, and speech
# in a loud room. Higher can hold the first words back until the room is measured;
# lower can take a loud room's noise for speech.
# In dBFS (decibels relative to full scale), from -120 to 0.
speech_loud_dbfs = -35

# Speech detection: frames like that must follow each other for this long (rounded up
# to whole 30 ms frames), so a click or a bump isn't speech. Higher ignores longer
# noises, but may miss a very short word. 0 treats every recording as speech from its
# start, and decodes it all.
# In milliseconds, from 0 to 10000.
speech_min_ms = 120

# Once this much audio waits unpolished, the oldest stretch is cut at a pause and
# decoded in final mode while you keep talking; release then only waits for the rest.
# Chunks come out 0.5 to 2 times this long: the cut goes in the longest pause in the
# newer half, or at the quietest moment once twice this much waits without a pause.
# The preview window also starts at the last cut. Lower makes release faster and
# preview passes cheaper, but cuts more often (each cut is a join that can go wrong,
# and its context is decoded twice). 0 never cuts: one pass over the whole recording
# on release, which grows fast (21.6 s after 9 minutes).
# In milliseconds, from 0 to 600000.
chunk_ms = 30000

# Audio decoded on each side of a chunk for context, when the model times each word
# (Parakeet; Whisper gets none). The context's words belong to the neighbours. Without
# it, punctuation across a cut suffers (a sentence lost its full stop at about one cut
# in ten); 1 s and 2 s gave the same accuracy. Higher costs more decoding per chunk.
# In milliseconds, from 0 to 5000.
chunk_context_ms = 1000

# The newest audio is never cut, since the speaker may be mid-word.
# In milliseconds, from 0 to 5000.
chunk_guard_ms = 500

# Joining chunks: a word whose middle lies up to this much before a chunk is still kept
# when the previous chunk's decode didn't hear it (two decodes' timings differ by a
# frame or two). Higher loses fewer words at joins, but may keep a word made up in the
# pause; lower the opposite.
# In milliseconds, from 0 to 2000.
join_slack_ms = 300

# Joining chunks: two decodes' copies of the same word around a cut count as one word
# when they lie at most this far apart. On real speech they differed by up to 0.46 s.
# Higher catches more doubled words; lower keeps more words said twice in a row.
# In milliseconds, from 0 to 5000.
join_match_ms = 1000

# Preview only: the longest run of words at the start of the preview that is checked
# against the end of the polished text, so words aren't shown twice after a chunk.
# In words, from 0 to 20.
join_max_repeat_words = 4

# Shortest quiet stretch a chunk can be cut at (rounded up to whole 30 ms frames). Gaps
# between words are mostly shorter. Higher cuts only in clear pauses, but more chunks
# end up cut at the quietest moment instead; lower may cut between words.
# In milliseconds, from 30 to 10000.
pause_min_ms = 150

# Pause finder: a 30 ms frame is quiet when its level is at most this, relative to the
# loud frames around it (their 90th percentile; -20 is a tenth of their level). Higher
# (nearer 0) still finds pauses over loud room noise, but may cut in a soft syllable;
# lower counts only clear silence.
# In dB (relative to another level), from -60 to 0.
pause_quiet_db = -20

# Pause finder: ...or at most this, so a stretch of pure silence counts even with no
# loud frames around it.
# In dBFS (decibels relative to full scale), from -120 to 0.
pause_silence_dbfs = -55
```
