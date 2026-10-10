# EasyVox design notes

Working notes. Decisions are recorded as they're made; what's still open is listed at the end.

## Requirements

- All processing happens on the Mac. No audio or text leaves the machine. The only network use is downloading models.
- English only for now. French is a stretch goal (see below).
- Lightweight and fast: low memory use when idle, and text pasted quickly after you release the key.
- **Light on the machine while you dictate.** Brandon, 2026-10-09: "The preview doesn't need to feel lagless and perfectly live. I want this to be performant and light on the host machine. If the fans turn on a MacBook while recording we went way overkill." Lightness beats preview liveness; the final text's accuracy and speed still matter. Being light and performant is a feature of the product, not a fan-noise threshold: "this is a dictation app at the end of the day, so it needs to be light and performant as a feature." So when a choice trades the preview's liveliness against power, take the lighter one, as long as the final text stays as accurate and as quick.
- Push-to-talk on a shortcut the user picks: hold to record, release to transcribe.
- Long dictations are normal use: holding the key for several minutes and rambling. The live preview has to keep up the whole time, and the final text has to arrive quickly with nothing dropped.
- Show text as it's recognized, in chunks, so you don't wait until the end.
- On release, paste the transcript at the cursor. The transcript stays on the clipboard until something else replaces it.
- History of past transcripts, each with a quick copy button.
- Subtle, quick animation for polish: a live indicator that reacts when it hears you, a smooth open and close, and new words easing in.
- Light and dark mode, following the macOS appearance setting and switching live when it changes. This covers the overlay, Settings and History; the menu bar icon is a template image, so macOS tints it automatically.
- Local storage for history, settings and downloaded models.
- A tight model list with one opinionated default. Models are easy to swap, since we'll add options over time.
- Use both "two model" approaches (see below) without giving up speed or lightness.
- macOS first. Linux possibly later.

## Core flow

1. Hold the shortcut. Recording starts.
2. A small floating window shows the text as it comes in.
3. Release. EasyVox finishes the transcript, puts it on the clipboard and pastes it at the cursor.
4. The transcript is saved to history.

## Keeping it light and fast

- The speech model drives speed and memory far more than the UI framework does. In the spike, the GPUI Kit shell idled at about 22 MB with no model loaded and about 636 MB with a Parakeet model loaded.
- **Nothing runs while idle.** With nobody dictating, the app wakes up about once every few seconds and uses no measurable CPU, model loaded or not. It used to wake about 145 times a second, from a timer inside ggml's Metal code; see "Engine and macOS gotchas".
- **The model is freed after 10 minutes without use**, and loaded again when you next press the shortcut. Recording starts on the key press either way: the audio buffers while the model loads (about 0.15 s once the Metal shaders are cached), the preview starts once it's ready, and the final text covers everything you said. The first words show up about 20 ms later than with the model loaded. Unloading takes the idle app from about 670 MB to about 90 MB, and it still wakes only about once every 3 seconds. A "Keep model loaded" setting turns it off.
- **Dictating is light too.** While you hold the key, EasyVox adds about 0.65 W to what the Mac draws (some 40 J per minute of dictation): the GPU is active about 6 % of the time and the CPU uses about 8 % of one core. It used to be about 4.1 W, with the GPU working most of the time. The preview re-decodes only the words that may still change, every 1.6 s (see "Live text while speaking").
- **The level meter is nearly free.** In the app, replaying the 5-minute ramble with the overlay up, the meter and its drawing added 0.3 points of one core to the app's 11 % (2.7 % more), and no energy that could be told from noise: 35.4 J a minute with it, 35.5 J without, the GPU at 0.51 W both ways. It sends only visible changes, about 7 a second while you talk and none in a pause. At 30 ticks a second instead of 15 it took 0.43 points. Not yet measured: the GPU and WindowServer's share of drawing it on screen, since the measuring Mac's screen was locked. Numbers: [`docs/model-bench.md`](docs/model-bench.md), "Level meter".
- **The preview rests while you pause.** While the key is held, it only re-decodes when something was said since the last pass (see "Live text while speaking"), so a thinking pause costs almost nothing. Before you say anything nothing is decoded at all, and a press with nothing said costs no decode. The short breaths between phrases still get decoded, and real speech is mostly those, so the saving is small in practice (see "Engine").
- Use one speech model for both the live preview and the final text, so two models never sit in memory at once (see "two model" approach B).
- LLM cleanup is optional and off by default. The LLM loads only when cleanup is turned on.
- Set a latency target once we can measure, for example: a short sentence pasted within half a second of release, without cleanup. In the spike, the final text for 3–30 s of speech was ready 0.09–0.49 s after release (from recorded clips, not a real mic).

## Feature notes

### Onboarding

The first launch opens a "Set Up EasyVox" window (`crates/easyvox-app/src/onboarding_window.rs`), drawn like Settings, with four steps. Each says where it stands and what to do, and the steps can be done in any order: the download can run while you answer the prompts.

1. **Microphone.** "Allow…" shows the macOS prompt. Nothing opens the microphone before it has been asked (see "Engine and macOS gotchas"). A "no" shows as a problem with an "Open System Settings…" button for Privacy & Security → Microphone.
2. **Accessibility,** for the ⌘V paste. "Allow…" shows macOS's own prompt (`AXIsProcessTrustedWithOptions`), whose button opens System Settings at the Accessibility list; the step also links there directly. macOS has no "denied" to read back, so after the prompt the step points at the switch. The window reads both permissions again every second while one is missing, and whenever it comes forward, so turning the switch on shows within a second, without a relaunch. The timer stops once both are allowed, and goes with the window; nothing runs while onboarding is closed.
3. **The speech model**: the chosen one, Parakeet TDT v2 unless the settings file picks another offered model, in the same row Settings → Models shows (Download, progress, Cancel, Try again, and the credits). It goes through the one download path (see "Downloading a model"), so it's verified before it's used and its LICENSE.txt comes with it.
4. **Get ready.** When the chosen model finishes downloading, the controller loads it at once. The first load under a new app identity compiles ggml's Metal shaders (about 14 s), so it happens here, behind "Getting ready…", and not on the first dictation. Later loads take about 0.2 s, in this launch and the next ones. A failed load shows its error with "Try again".

**Failure states.** No network: "This Mac seems to be offline" and Try again. A checksum or size mismatch: nothing kept, Try again. Not enough disk space, or any other download error: its sentence, and Try again (a retry starts from zero). A dropped connection isn't a failure: the step says the download picks up where it stopped, and the row shows the try count. A denied or restricted microphone: Open System Settings. Only the first load shows a spinner, which redraws every frame; the download has its progress bar, and a wait on you needs no animation.

**No "onboarding done" flag.** Onboarding opens at launch whenever a required step is missing: the Microphone was never asked, Accessibility isn't allowed, or the chosen model isn't downloaded. So a fresh data folder, reset permissions or a deleted model bring it back, and a relaunch with everything in place skips it. A microphone you refused doesn't reopen it: it was asked, and the push-to-talk notice and Settings say where to turn it on. The cost: someone who keeps Accessibility off on purpose (the text then waits on the clipboard) sees onboarding at every launch, and closes it with "Finish later". If that turns out to annoy people, a "don't ask again" would be the one reason to store a flag. Settings → Permissions → "Open setup…" opens it any time. Development builds signed ad hoc lose their permissions at each rebuild (see "Signing and notarization"), so onboarding opens after each one, which is right: the grants are gone.

**Tests.** The step logic is plain Rust (`crates/easyvox-app/src/onboarding.rs`): it's worked out from the permissions, the download and the engine, passed in, so its unit tests need neither TCC nor the network. The self-test checks that onboarding opened at launch exactly when this Mac's permissions and models folder say it should (reading them, never changing them), that it stays shut or opens for injected answers, and that it draws every step state. Then it runs the model and warm-up steps for real in a temporary folder: the chosen model is served from 127.0.0.1 (read from the models folder), the connection is cut once and the download picks up where it stopped, and the model lands verified with its license notice and loads.

### Push-to-talk shortcut

- Needs both key-down and key-up events.
- Regular combos (like ⌥Space) are straightforward. They go through the `global-hotkey` crate and need no permission. ⌥Space works on macOS 26, despite the Option-only restriction added in macOS 15.
- Holding a modifier on its own (right ⌥, Fn) needs a lower-level event tap and the Input Monitoring or Accessibility permission. macOS uses Fn/Globe for its own features by default. Creating the tap succeeds even without the permission, so check it with `CGPreflightListenEventAccess()`.
- The shortcut recorder in Settings must pause the live shortcut while it records, or the OS swallows the combo.
- Later: a toggle mode (tap to start, tap to stop) for long dictation.

### Live text while speaking

- **One pipeline for every model.** Half a second after you start speaking, and then every 1.6 s, EasyVox re-decodes the newest audio and shows the text in two states: confirmed (stable) and provisional (may still change). A word is confirmed once two passes in a row agree on it. Each pass starts 1 s before the confirmed words (never before the last chunk cut), so it only decodes what may still change: about 5 s of audio on average. No model streams natively, Parakeet Unified included (see "Engine").
- **Why every 1.6 s, from the confirmed words: lightness first** (see "Requirements"). The preview used to re-decode everything since the last chunk cut, up to 20 s, every 0.4 s. That kept the GPU working most of the time you talked: about 4.1 W on top of the idle Mac (some 250 J per minute of dictation), and a third of a CPU core. The preview's GPU energy follows how much audio it decodes, so starting each pass at the confirmed words cut the whole cost by about three-quarters at the same 0.4 s; a pass every 1.6 s took the whole dictation to about 0.65 W (some 40 J a minute, a sixth of before), with the GPU active about 6 % of the time and the CPU at about 8 % of a core. The preview pays in lag: a word shows up a median 0.8 s after you say it (0.2 s before), and 95 % of words within about 2 s (0.7 s before); the first words of a dictation came about 1.7 s after the key press instead of 0.5 s, until the first pass moved forward (next point). Going further saves little: a pass every 2 s took about 6 % less GPU energy but pushed the lag past 2 s and the median to 1 s; every 1.2 s kept 95 % of words within 1.6 s, for about a tenth more. The final text is word for word the same. It comes as fast after long dictations (0.35 s after a 5-minute ramble, as before), and about 25 ms later after short ones (0.20 s instead of 0.18 s on an 11 s clip): the old preview kept the GPU at full speed, so the final decode started on a hot GPU. For scale, polishing the chunks alone, with no preview at all, costs about 16 J a minute. Numbers and method: [`docs/model-bench.md`](docs/model-bench.md), "Lightness".
- **First words: an early pass.** At one pass every 1.6 s, the first words waited for the first pass, about 1.7 s after the key press. So the first pass now runs once 0.5 s of speech has come in (`preview_first_ms`), counted from where speech starts (next point), then every 1.6 s. Until the preview shows words, passes keep coming 0.5 s apart: Parakeet Unified sometimes needs more than 0.5 s of speech before it writes anything, and otherwise made you wait 2.2 s. The first words now show about 0.6 s after you start speaking, with every model (1.70 → 0.58 s after the key press on the test clips, which speak at once; 0.98 → 0.58 s on 28 starts cut from real recordings). It's a pass or two per dictation: a 5-minute dictation took 39 J a minute against 38 before, with the same GPU power and the same 185 passes, and the final text is word for word the same.
- **No speech, no text.** Until a recording holds speech, nothing is decoded: no preview, no chunk, no final pass. A recording of silence or room noise ends with empty text, whatever the model: nothing is pasted, the clipboard and History are left alone, and the overlay says "Didn't catch anything" for a moment, then hides. Models make up words on silence: Whisper base.en wrote "you" for 2 s of it, and showed "you" in the preview on the room noise before the first word. Speech is 120 ms in a row of 30 ms frames at least 6 dB louder than the room, or louder than -35 dBFS whatever the room, and never at or under -55 dBFS. The room is the recording's quietest frames (their 10th percentile, leaving out the digital silence a Bluetooth mic can start with), measured again at every check, so a guess from the first few frames can't stick. The -35 dBFS lets speech that starts at once count straight away, before there are quiet frames to measure the room by; the 120 ms ignores clicks. The 6 dB is low on purpose: a false "no speech" loses a whole dictation without a word, which is far worse than a stray "you", so the check only calls a recording empty when nothing in it rises above a steady room. It started at 20 dB, which dropped a soft "OK" whose loudest moment was 17 dB over the room, and speech 25–30 dB quieter than the author's (a laptop's own mic at arm's length). It's the same loudness measure as the pauses, and needs no VAD model (Silero would need ONNX Runtime in the app). Once a recording has held speech, everything is decoded as before, so the final text doesn't change. On 2 s of silence and on the quiet starts of the author's two recordings, all three models now give empty text without a single decode, where Whisper wrote "you" or another made-up word on all three and v2 on one; 5 s of the ramble's pauses, with breaths in them, are decoded and come out empty. Every short command still comes through, and so did every case of a stress test: speech 10–30 dB quieter, a soft "OK" or "Yes" at -45 dBFS, speech whose loudest moment is only 10 dB over the room, and speech under loud noise or babble. Numbers: [`docs/model-bench.md`](docs/model-bench.md), "First words and silence".
- **Pauses.** A pass runs only if something was said since the last check, measured by loudness against your own speech (the same measure as the chunk cuts below), plus two more passes after you stop, so your last words are decoded with the silence after them and confirmed. Then the preview waits for the next word. The final text decodes all the audio either way. One cost: releasing the key after a pause of more than about a second makes the final text about 25 ms later (0.31 s instead of 0.28 s on a test clip), most likely because the GPU slowed down while it rested.
- **Long dictations are polished as you talk.** Once about 30 s of audio is waiting, the oldest stretch is cut at a pause and decoded again in final (offline) mode in the background. On release only the rest is decoded, so the final text takes about as long after 9 minutes as after 30 s: about 0.4 s, where one pass over the whole recording took 21.6 s. Every word of the final text comes from a final-mode decode.
  - **Cuts** go in the middle of the longest pause in the newer half of the waiting audio. Pauses are found by loudness for now; a voice activity detector (Silero) could replace that later. With no pause for a full minute, the cut goes at the quietest moment.
  - **Joins.** When the model times each word (Parakeet), a chunk is decoded with 1 s of the neighbouring audio on each side, so punctuation across a cut comes out as in one pass, and words are assigned to a chunk by their timing. Around the cut, where both decodes heard the audio, their words are first matched by text, so a word that the two decodes put on opposite sides of the cut is kept once, not lost or doubled. Whisper only times segments, so its chunks are decoded without context and simply joined.
  - **In the preview**, a polished chunk's text replaces its preview text. That's the only time confirmed text changes, and it only touches a finished chunk, seconds behind the newest words.
- **Releasing the key while a preview pass runs** makes the final text wait for that pass: transcribe.cpp checks for a cancel only before a Parakeet decode starts, so a pass can't be stopped midway. With the lighter preview a pass runs about 7 % of the time and takes about 0.1 s, against 40 % and 0.17 s before, so the wait is rare and short.
- **The numbers are defaults.** The 1.6 s, the first pass 0.5 s into speech, the 1 s before the confirmed words, the 20 s cap on a pass, the 30 s chunks, their 1 s of context, the two passes after speech, the speech levels and the pause levels above, and the level meter's (see "Overlay animation"), all live in one place, `Tuning` in `crates/easyvox-core/src/tuning.rs`, each with its unit and what raising or lowering it trades off. Every model gets the same values. The bench can change any of them (`--set`), and so can the settings file's `[advanced]` table, from the next dictation on (see "Local storage" and [`docs/settings.md`](docs/settings.md)).
- Live text goes in EasyVox's own floating window, not the target app. Typing provisional text into another app and then correcting it is fragile. The final text is pasted once, on release.

### Overlay animation

Subtle and quick. It should feel responsive, never slow you down, and never draw attention away from what you're typing into.

- **Hearing input.** A level meter of five bars, left of the text, moves with your voice and rests flat (five dots) in silence. It's the first sign the mic is working. If no audio reaches EasyVox, the bars lie flat in amber and the pill says "Can't hear you. Check your microphone." instead.
  - **The core sends it** as `TranscriptEvent::Level(f32)` (0 flat, 1 full) and `TranscriptEvent::NoInput(bool)`, from a third session thread (`crates/easyvox-core/src/level.rs`) that lives only while the session captures. It ticks every `meter_interval_ms` and sends a level only when it moved by `meter_step_db` (2 dB, a little under a point of the tallest bar), so a steady room sends nothing and the overlay doesn't redraw. The overlay redraws only when a bar changes by half a point (a pixel on a Retina screen), and the bars move only while the pill is up: the events come from a running dictation, and nothing runs between dictations. The thread never waits on a decode, so the meter moves while the model loads, too.
  - **No second pass over the audio.** The capture thread measures the level of each 30 ms frame once, as audio arrives (`pause::FrameLevels`). The speech detector reads those levels (it used to measure them itself), and so does the meter: one computation, shared with the no-speech gate.
  - **Against the speech line.** The meter shows the loudest frame of each tick in decibels over the speech detector's line (6 dB over the room, or -35 dBFS), across `meter_range_db` (36 dB). So room noise rests flat, and the meter moves for what the detector counts as speech: a meter that stays flat means EasyVox hears nothing it would decode. It rises at once and falls back over `meter_release_ms` (150 ms: calm enough not to flicker; in a simulation on the ramble, 100 ms followed syllables more closely for about as many redraws).
  - **About 15 a second** (`meter_interval_ms` = 66), the lowest rate that should still look alive: a syllable lasts about a quarter of a second, so 15 ticks a second show each one rise and fall. Most ticks don't move it anyway. On the 5-minute ramble the meter moved 7.2 times a second at 15 ticks a second; 30 ticks (the first guess) moved it 10.9 times, for twice the wake-ups and 0.43 points of a core instead of 0.30 (below). Whether 15 looks alive needs a person at the screen; the setting changes it live.
  - **"Can't hear you"** means no audio reaches EasyVox, which isn't the same as quiet audio: no samples for `meter_no_input_ms` (1.5 s), or, until the input has delivered 0.3 s of sound, only digital silence (exact zeros). Zeros count because a muted or blocked input, the commonest dead microphone, delivers them: from EasyVox's side it hears nothing at all. But only until real sound has come, for two reasons. Bluetooth headsets start with about a second of zeros (the ramble's AirPods did), which the 1.5 s rides through. And some microphones and virtual devices gate their noise to exact zeros in pauses, so once the input has proved live, zeros are a pause, not a dead microphone. A microphone muted mid-dictation then shows as a flat meter rather than the warning; one that stops delivering samples still shows it. It replaces the core's old one-off errors ("No audio arrived from the microphone in 1.5 s", "...delivering digital silence"), and clears itself when audio comes back.
- **Open and close.** A fast fade plus a slight scale or slide, roughly 150 ms in and 120 ms out. The pill opens on key press without waiting for the model, and closes right after the paste.
- **New words and chunks.** New words fade in over about 100–150 ms. Provisional text eases to full strength when it's confirmed. Avoid layout jumps: the pill's width grows smoothly, and long text scrolls smoothly to keep the newest words in view.
- **Reduce motion.** Respect the macOS "Reduce motion" setting: swap the movement for simple fades.
- **Cost.** Animate only while the overlay is visible, with nothing running at idle. The level meter redraws far more often than the text changes (about 8 times a second on real speech, against 0.6), so it was measured: see "Keeping it light and fast". Its on-screen drawing (GPU and WindowServer) still needs measuring with the screen unlocked, which the measuring Mac never was.

### Light and dark

Every window follows the macOS appearance and switches live, mid-dictation too. Settings, History and onboarding use GPUI Kit's theme, kept in step by `Theme::sync_system_appearance` whenever their window's appearance changes.

- **The overlay** has its own pair of palettes (`Palette` in `crates/easyvox-app/src/overlay.rs`): the dark pill it started with, and a light one (`#f5f5f7` with near-black text). It reads the app's appearance at every draw, and redraws when its window's appearance changes. It reads the app's rather than its window's because GPUI caches a window's appearance and updates it only when AppKit tells that window, which a hidden panel may not be.
- **Contrast.** Confirmed and provisional text meet WCAG AA (4.5:1) in both modes, whatever is behind the pill (it lets 6 % through), and a unit test computes the ratios. Confirmed text is 14.5:1 dark and 13.6:1 light. Provisional text was 4.2:1 on the old dark pill, under AA, so it fades less now: 5.5:1 dark, 5.1:1 light, still clearly fainter. The meter and dots use Apple's system colours (the accessible variants in light mode, since the usual red, green and orange are too pale on white) and clear 3:1 against the pill, the bar for UI graphics.
- **Tests.** The self-test switches EasyVox's own appearance (`NSApp.appearance`) to the other mode and back during a dictation, never the system's, and checks that the overlay redraws by itself in the right palette each time.


### Paste at cursor

- Standard approach: put the text on the clipboard, then simulate ⌘V. This needs the Accessibility permission, and without it macOS drops the keystroke silently, so EasyVox checks first and shows an error.
- ⌘V is sent as key code 9 (where V sits on a US keyboard) with only Command held. Non-QWERTY layouts may need a layout lookup.
- The transcript stays on the clipboard on purpose, which means whatever was there before is lost. History covers this, since every transcript is saved. A "restore previous clipboard" setting could come later.
- EasyVox only ever writes the clipboard, never reads it. On macOS 26, reading the clipboard can trigger a privacy prompt.
- If no text field has focus, the text is still on the clipboard and in history.
- A recording with no speech pastes nothing and leaves the clipboard and history alone; the overlay says "Didn't catch anything" (see "Live text while speaking").
- Fallbacks for apps where ⌘V misbehaves: simulated typing (slow for long text) or inserting through the Accessibility API (not every app supports it).

### History

- Newest first, one-click copy per entry.
- Store text, timestamp, duration and which model was used. If cleanup was on, keep the raw text too.
- Later: search, delete, a retention limit.
- **Text only; no audio is stored.** Keeping audio would allow re-transcribing with a better model later, but it costs disk (about 2 MB a minute uncompressed) and raises privacy concerns.

### Local storage

- **Data:** models and history live in `~/Library/Application Support/EasyVox/` on macOS (`~/.local/share/easyvox/` on Linux): a `models/` folder and `history.sqlite3`.
- **Settings:** a TOML file at `~/.config/easyvox/config.toml`, on macOS too, never `~/Library/Preferences` (`$XDG_CONFIG_HOME/easyvox/config.toml` when that's set to an absolute path; `EASYVOX_CONFIG` points at any other file). Agents and dotfiles can configure EasyVox through it. It holds no permission state: agents can't grant Microphone or Accessibility, a person has to click those. Every key, its default, its unit and what it trades off: [`docs/settings.md`](docs/settings.md), or `easyvox --print-default-config`.
  - **The defaults live in code; the file only overrides.** A missing key is its default, so a default can improve in a release without being frozen in old files. The top level holds what the Settings window shows (`model`, `shortcut`, `hold_right_option`, `keep_model_loaded`). `[advanced]` holds everything else: 4 of the app's own numbers (`unload_after_secs`, the overlay's two hide delays and its `overlay_max_chars`) and all 28 fields of the pipeline's `Tuning`. 36 keys in all, the same for every model.
  - **The Settings window writes only what you change there**, through `toml_edit`, so the file keeps your comments, order and other keys. It never writes `[advanced]` keys. It writes atomically, through a symlink to where it points.
  - **Outside edits apply live.** The app watches the file's folder (creating it if needed) with FSEvents, through the `notify` crate GPUI Kit already uses, so nothing runs while the file doesn't change: idle wake-ups stayed at 0.3 a second. A change is read 150 ms after the last event, and compared with the text the app last read or wrote, so its own saves are seen as unchanged and can't loop. A new shortcut or model applies at once; the pipeline's numbers apply from the next dictation, without reloading the model, since each session takes its `Tuning` when it starts.
  - **Validation is per key.** A value out of range or of the wrong type falls back to that key's default, and an unknown or misplaced key is ignored, each with a warning in the log and in Settings. A file that isn't valid TOML keeps the last good settings (the defaults at startup), logs the error, shows it in Settings, and is never overwritten: changes made in the window meanwhile apply but aren't saved.
  - **Loudness is in decibels**, relative to your speech (`_db`, default -20) or to full scale (`_dbfs`, default -55). They were linear levels (0.1 and 0.0018); -55 dBFS is 0.00178, and on every test clip, the long-dictation clips and a pauses-over-noise clip, the speech gate and the pause finder make exactly the same decisions as before (`pause::tests::db_defaults_decide_like_the_old_levels`).
- **Downloads in progress** sit next to the model as a hidden `.<file>.part`, deleted if the download fails or is cancelled (see "Downloading a model").
- For development and tests, `EASYVOX_DATA_DIR` moves the data folder, `EASYVOX_MODELS_DIR` moves just the models, and `EASYVOX_CONFIG` points at a different settings file. The self-test uses a temporary settings file unless `EASYVOX_CONFIG` names one.

### Swappable models

- The app talks to one engine interface: start a session, feed audio (16 kHz mono), receive provisional and final text, finish.
- Each runtime gets an adapter behind that interface. Today: transcribe.cpp (in the app) and sherpa-onnx (bench only). Core ML or Apple Speech could get adapters later.
- The model catalog (`crates/easyvox-core/src/models.rs`) lists each model's id, name, runtime, offering (default, alternative or bench only), size, download URL (pinned to a Hugging Face commit), sha256 checksum, license and attribution. Adding a model means adding an entry, plus an adapter if it needs a new runtime.
- Settings screen: pick a model, download or delete models, see disk usage (Settings → Models, below). Which models are offered is covered in "Models" below.

## Models

The list is tight on purpose: one opinionated default and one small alternative, each with a one-line "pick this if". Every model on it runs on transcribe.cpp, the one speech runtime built into the app. No model ships inside the app. Each one goes through the same flow: download, sha256 check, then activate. Download URLs are pinned to a Hugging Face commit (`resolve/<commit>/<file>`), so a re-upload can't break the sha256 check.

| | Model (GGUF) | Download | Pick this if… | License |
|---|---|---|---|---|
| **Default** | Parakeet TDT 0.6B v2, Q5_K_M | 539 MB | You want the best all-round model. | CC BY 4.0 |
| Alternative | Whisper base.en, Q8_0 | 85 MB | You want the smallest model (an 8 GB Mac, or memory matters more than the last bit of accuracy). | Apache 2.0 |

Choices as of 9 October 2026, settled by a local bench the same day ([`docs/model-bench.md`](docs/model-bench.md)). Parakeet Unified was the default until then; v2 replaced it once the one-pipeline re-run showed Unified's edge was gone. On 10 October Unified left the app altogether and became bench only (see "Bench only"). This area moves fast, so re-check before changing the list. Accuracy numbers are average word error rates (WER, lower is better) and only compare within one source.

In the code, each catalog entry in `crates/easyvox-core/src/models.rs` has an `offering`: `Default` (exactly one), `Alternative`, or `BenchOnly`. A settings file that names a bench-only or unknown model gets the default, with a warning.

**Settings → Models** lists every offered model, the default first, downloaded or not, each with its size, its "pick this if" and its credits (see "Licenses"). Not downloaded: a Download button, then a progress bar with Cancel; a failed download shows why, in one sentence, and leaves nothing behind. Downloaded: Use and Delete (Delete asks once more, with the size it frees), and the disk space it takes. The model in use is marked Active and can't be deleted. Bench-only models never appear. Each row is one reusable element (`crates/easyvox-app/src/model_row.rs`) that reads the app's state and acts through the controller, and onboarding shows the same row for the chosen model. The General page keeps a quick picker among the downloaded models.

### Downloading a model

One code path for every model, onboarding and Settings alike: `easyvox_core::download` (`crates/easyvox-core/src/download.rs`). Onboarding and Settings add only the screens.

- **Where the bytes go.** A hidden `.<file>.part` next to the model (`models/gguf/.parakeet-tdt-0.6b-v2-Q5_K_M.gguf.part`), hashed with sha256 as it streams in, and locked (`flock`) while a download uses it. Nothing appears under the model's own name until it's verified.
- **Verify, then activate.** The size must match the catalog's `download_bytes` (a server announcing another size is refused before the body is read) and the hash its `sha256`. Then the part file is flushed to disk (`F_FULLFSYNC`), made readable (0644, like the script), and renamed into place, which is atomic. A checksum mismatch isn't retried on its own: the server's file is different, and fetching it again won't help.
- **Resume within one download, never across them.** A dropped or stalled connection (30 s without a byte) is picked up where it stopped with `Range: bytes=N-`, starting again from the pinned `huggingface.co` URL each time (its CDN links are signed and expire after about an hour). It gives up after 5 tries in a row that bring nothing, 1 to 8 s apart; a try that brings bytes starts the count again. That covers Wi-Fi dropping out and the Mac sleeping: the Mac may idle-sleep during a download (the app only holds off App Nap, as it does while dictating), and the download resumes after the wake. A server that ignores `Range` still works: the bytes already here are skipped as they arrive again. A cancelled or failed download leaves nothing behind, and the next one starts from zero.
- **Nothing left behind.** The part file is deleted on every failure, on cancel, and if anything panics. One left by a crash or a force quit is swept at the next launch, unless a running download holds its lock. Each tool cleans up only its own files: `scripts/fetch-model.sh` uses `.<file>.part.XXXXXX` names.
- **A copy already there** is hashed and kept if it checks out, with no request; a damaged one is replaced. At launch only its size is checked (`ModelInfo::is_available`), since hashing 539 MB at every launch would cost energy for nothing. A truncated model shows as not downloaded (`EngineStatus::Missing`).
- **Network.** Downloads are the only connections EasyVox makes: to `huggingface.co`, which redirects to its CDN (`*.hf.co`). They go through Apple's NSURLSession, so the proxies, PAC files and certificates set in System Settings apply, and firewall apps show the connection as EasyVox. The session is ephemeral (no cookies, cache or credentials on disk), and the request asks for no compression, so byte offsets are the file's own. No HTTP or TLS crate is compiled into the app, and nothing runs between downloads (idle wake-ups stayed at 0.3 a second).
- **In the app** (`controller.rs`), each download runs on its own thread and reports about 4 times a second, into `AppStatus::downloads`. Switching models goes through one function, `use_model`, which waits for a dictation to end (so a switch applies from the next one), then frees the old model before loading the new one, so only one is ever in memory. Frees and loads take one lock in turn on the background executor, and a load that a quicker second switch overtook frees its model before the next one starts. When the chosen model finishes downloading, it loads at once, which on a first launch is also the shader warm-up (onboarding's "Get ready" step).
- **The license travels with the file.** Once a model checks out, its license notice is written next to it as `<file>.LICENSE.txt`: the credit, the license link, the original model card, the modification note with the converter's repo, and the disclaimer. Delete removes both.
- **Tests.** Every failure is injected by a small HTTP server on 127.0.0.1 in `crates/easyvox-core/tests/download.rs` (cut and stalled connections, redirects, a server ignoring `Range`, wrong bytes, 404, cancel, a second download of the same model), so they run offline in `cargo test` and in CI. The self-test downloads from a server of its own the same way. The live checks against Hugging Face, a HEAD of every offered URL and a real download of Whisper base.en, are in `download_live.rs`: ignored, and gated behind `EASYVOX_LIVE_DOWNLOADS=1` so the engine checks (`--ignored`) never download.

### Default: Parakeet TDT 0.6B v2

**Downloaded during onboarding, not bundled.** This keeps the DMG small, app updates don't re-ship 539 MB, and onboarding already has to ask for the Microphone permission and wait out the ~14 s first-launch shader compile, so the download fits there. The catch: the first run needs a network connection.

Why it's the default (decided 9 October 2026; Parakeet Unified was the default before):

- **Fewer errors on long dictations.** In real-time replays of 3- and 9-minute recordings it made 0 % and 0.40 % word errors, against Unified's 0.30 % and 0.86 %. On the short clips the two made the same two errors (unusual names).
- **It always ends with a full stop.** It ended all 13 speech clips with one, where Unified left it out on 4 of 13, and Unified does the same at chunk cuts. Its word timings are also tidier around a cut: Unified stretches a word with trailing punctuation over the pause after it (see `chunks.rs`).
- **Speed and size.** In the October bench, on the one pipeline: first words after about 0.45–0.5 s, final text 0.11–0.45 s after release for 3–30 s of speech and about 0.4 s after 3 or 9 minutes, 15–51 % of one core while you talk, about 690 MB of memory (830 MB at the peak of a 9-minute dictation). That's what Unified costs too.
- **License.** CC-BY-4.0: free for any use, with credit to NVIDIA.
- **Accuracy elsewhere.** It's one of the best models under 1B parameters on the [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) (6.43). Unified scores a little better on public benchmarks (see "Bench only"), but on the author's own voice v2 was a word ahead.
- **Why it wasn't the default at first.** Unified was picked as the only model that could both stream the preview and do the full-accuracy final pass from one set of weights. With one pipeline, every model does both by re-decoding, and that edge went away. Numbers in [`docs/model-bench.md`](docs/model-bench.md).
- **Quantization: Q5_K_M, not Q8_0.** In the bench, Q5_K_M wrote exactly the same text as Q8_0 on every clip, showed first words just as fast, and finished within 10–20 ms of it (its decodes are about 5 % slower). In exchange it's 191 MB smaller to download and uses about 190 MB less memory (690 vs 880 MB). The Q8_0 entry is gone from the catalog.

### Alternatives

- **Whisper base.en (Q8_0): smallest.** 85 MB, about 205 MB of memory. On our clips it made as few errors as Canary 180M Flash on ordinary sentences (1.5 %) and wrote numbers as digits. Its known flaw: it makes up words on silence (2 s of silence came out as "you"), and it once added a stray "Mhm." at the end of a long clip. So the app decodes nothing, final pass included, for a recording that holds no speech, whichever model is active (a level check; see "Live text while speaking"). It also used to lose text past 30 s (16 % word errors on a 3-minute dictation, whole paragraphs missing); its adapter now asks for timestamps on audio over 30 s, and chunking keeps most decodes under 30 s anyway (0.6 %). Its preview flickers more than Parakeet's, since it has no word timings to align passes by.
- **Canary 180M Flash: benched, not offered.** It was the candidate for the small slot: more accurate than Whisper base.en on the leaderboard (7.48 vs 11.10 on the June 2026 version), and the empty-transcript issue ([transcribe.cpp #140](https://github.com/handy-computer/transcribe.cpp/issues/140)) never showed up (0 empty results in 84 decodes, nothing on silence). But on transcribe-cpp 0.3.1 it writes every number as words ("three hundred and forty two dollars", "five five five zero one four two"): the runtime has no number-formatting (ITN) switch for Canary. That puts it at 7.1 % WER on our clips against Whisper's 1.4 %, which fails the "no worse than Whisper base.en" bar, and digits matter for dictation. Worth re-checking if a later transcribe.cpp adds ITN for Canary.

### Licenses

The catalog has, for every model, a `license` (name and URL), the `creator`, an `attribution` line, the upstream model card, and a `modification` note: every file we download is a GGUF conversion and quantization made by `handy-computer`, and CC BY 4.0 §3(a)(1)(B) and Apache-2.0 §4(b) ask that a note of changes travels with it. The repository it comes from (`ModelInfo::source_url`) is the download URL up to `/resolve/`.

- **CC BY 4.0** ([license](https://creativecommons.org/licenses/by/4.0/)): Parakeet TDT v2 ([card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2)), v3 ([card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3), bench only), and later Parakeet Ultra ([card](https://huggingface.co/moondream/parakeet-ultra)). Free for any use; credit the creator (NVIDIA; for Ultra, Moondream, based on NVIDIA's v3), link the license, and keep the modification note.
- **Apache 2.0** ([license](https://www.apache.org/licenses/LICENSE-2.0)): Whisper base.en ([card](https://huggingface.co/openai/whisper-base.en)). Permissive. OpenAI's Hugging Face card and the GGUF's metadata say Apache-2.0 and its GitHub release says MIT, for the same weights; we follow the Hugging Face release, which the GGUF is made from.
- **NVIDIA Open Model License** ([agreement](https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/)): Parakeet Unified and Nemotron, both bench only, so the app shows no NVIDIA notice. Its terms bind whoever runs the model, so offering one would need an "I agree" step the app doesn't have (`no_offered_model_needs_an_agreement` guards this). The `handy-computer` GGUF repo labels Unified CC-BY-4.0, but NVIDIA's [model card](https://huggingface.co/nvidia/parakeet-unified-en-0.6b) says the Open Model License, and upstream wins.

Where the credits show:

- **Settings → Models**, under each model: "Parakeet TDT 0.6B v2 by NVIDIA, licensed under CC BY 4.0." and "Converted to GGUF and quantized to Q5_K_M by handy-computer. Model card", with the license, the converter and the card as links that open in the browser.
- **Settings → About** (also "About EasyVox" in the menu bar menu): the version and EasyVox's MIT license; every offered model's credit, original model, modification note and the disclaimer ("Provided as is, without warranties of any kind."), with a line that EasyVox isn't affiliated with or endorsed by NVIDIA or OpenAI; and a button for the third-party notices.
- **Next to each downloaded model**, `<file>.LICENSE.txt` (see "Downloading a model").

The models never ship inside the app: each Mac fetches them from Hugging Face, so the duties that come with redistributing (a copy of the license, CC's notices) fall on the host. Its repos don't meet them (no notice files, and the wrong license tag on Unified), and EasyVox picks, pins and starts the download, so the app carries the credits itself. It costs a few strings and one small file.

**Bundled code.** The app links about 450 crates, plus C/C++ vendored in `transcribe-cpp-sys` (transcribe.cpp, ggml, miniz) and the Lucide icons in `gpui-kit-assets`; no GPL, LGPL or AGPL. `scripts/notices.sh` runs `cargo-about` (settings in `packaging/about.toml`, template `packaging/about.hbs`) to write `packaging/THIRD_PARTY_NOTICES.html`, which is committed; `scripts/bundle.sh` copies it and `LICENSE` into `EasyVox.app/Contents/Resources`, since the DMG and the Homebrew cask carry only the app. The one MPL-2.0 crate (`option-ext`) is listed with where its source is. `crates/easyvox-app/tests/notices.rs` fails when Cargo.lock has a crate the file doesn't account for (a dependabot update, say), so CI catches drift without installing cargo-about: run the script and commit.

### Bench only

These stay in the catalog (`offering: BenchOnly`) for `easyvox-bench` and comparisons, but the app doesn't offer them:

- Parakeet Unified EN 0.6B (Q5_K_M, NVIDIA Open Model License), bench only since 10 October 2026. It has the best public scores: in [transcribe.cpp's own runs](https://github.com/handy-computer/transcribe.cpp/tree/main/docs/models) of each GGUF, the lowest FLEURS English error of any model under 1B parameters (3.99 %), and NVIDIA's same-pipeline comparison on its [model card](https://huggingface.co/nvidia/parakeet-unified-en-0.6b) puts it ahead of TDT v2 (5.91 vs 6.04); it isn't on the Open ASR Leaderboard itself. But on the author's own voice it was about a word behind v2 (2.10 % against 1.68 % on the 81 s read-aloud clip), as fast, with a little more CPU, and it often left out the last full stop; its one edge, native streaming, went away with the one pipeline. And its license binds whoever runs it, which would need an "I agree" step in the app (see "Licenses"). A settings file that still names it gets the default, with a warning;
- Canary 180M Flash (Q5_K_M, CC BY 4.0), for the reason above;
- Parakeet TDT 0.6B v3 (Q5_K_M, CC-BY-4.0), v2's multilingual successor. On the author's voice it was no more accurate than v2 (one word behind on the read-aloud clip), wrote hesitations as words, and used about 40 % more CPU and 70 MB more memory, for the same energy ([`docs/model-bench.md`](docs/model-bench.md), "Parakeet TDT v3 against v2"). It stayed in English throughout. It's the stopgap for French (see "Later: French");
- the two sherpa-onnx models (Parakeet TDT 0.6B v2 int8, CC-BY-4.0, and Nemotron Speech Streaming 0.6B, NVIDIA Open Model License) and the Silero VAD model. The app doesn't compile that runtime in (it would add 18.5 MB), so they could never show up there anyway.

### Later: French

Parakeet Ultra (Q5_K_M, 550 MB), Moondream's further-trained Parakeet v3, is the pick for French. Its English is about v2's, and it has the best French of any fast model in transcribe.cpp's runs (4.66 % on FLEURS French). It needs transcribe-cpp 0.4, because its support landed after the pinned 0.3.1 ([PR #192](https://github.com/handy-computer/transcribe.cpp/pull/192)). Parakeet TDT v3 runs on 0.3.1 today as a stopgap; it's in the catalog as bench only (see "Bench only").

### Other models we looked at

| Model | Why not (for now) |
|---|---|
| Canary 180M Flash | Benched: writes numbers as words on transcribe-cpp 0.3.1, so 7.1 % WER on our clips against Whisper base.en's 1.4 %. Otherwise clean. See "Alternatives". |
| Qwen3-ASR 1.7B | The best open model on the leaderboard (5.75), but 1.5 GB and about 4× slower than Parakeet. The strongest candidate for a future "maximum accuracy" option. |
| Cohere Transcribe, Canary-Qwen 2.5B, Granite Speech 4.x | Accurate, but 1.6–2 GB and 2.5–4× slower, for a small gain. Cohere also silently cuts transcripts short past ~50 s ([#128](https://github.com/handy-computer/transcribe.cpp/issues/128)). |
| Granite Speech 5.0 TurboCTC | The fastest here, but by all signs it outputs lowercase text with no punctuation. |
| Nemotron Speech Streaming EN | Streams natively, but writes numbers as words (6.6 % WER on our clips) and trails v2 on the leaderboard. |
| Whisper large-v3-turbo, Distil-Whisper | Slower and less accurate than Parakeet. Turbo took 1–2.7 s to finish in the spike and makes up text on silence. |
| Moonshine Streaming | Small and streams natively, but no more accurate than Canary 180M and slower on long audio. |
| Phonon-2, Parakeet Redux | About 170 MB with near-v3 accuracy (2-bit and ternary Parakeet v3), but they don't run on transcribe.cpp yet ([#189](https://github.com/handy-computer/transcribe.cpp/issues/189)). If they ever do, they'd be the best candidates for the small slot. |
| Kyutai STT | Doesn't run on transcribe.cpp; it would need a second runtime. |
| Apple SpeechAnalyzer | Built into macOS 26: no download, no memory cost to us. Less accurate than Parakeet in one independent test, and needs Swift bridging. A possible extra. |

Runtimes:

- **transcribe.cpp** (`handy-computer/transcribe.cpp`, from the team behind Handy): ggml with Metal on the GPU. Runs Parakeet, Canary and Whisper GGUF models, among others. In the spike it ran Parakeet 4–5× faster than ONNX Runtime did on the CPU. The app's runtime.
- **sherpa-onnx / ONNX Runtime**: CPU, many models, macOS and Linux. The fallback runtime, behind the core's optional `sherpa` feature.
- **whisper.cpp / `whisper-rs`**: not used. It vendors its own copy of ggml, which can't be linked next to transcribe.cpp's; Whisper GGUF models run on transcribe.cpp instead.
- **WhisperKit, FluidAudio, MLX**: Swift or Mac-only. Would need bridging; not planned.

## The "two model" approach

People use this phrase for two different setups. EasyVox will do both, kept light.

**A. Speech model + language model cleanup.** The speech model produces raw text. A small local LLM then removes filler words, fixes punctuation, handles self-corrections ("Tuesday, no, Wednesday" becomes "Wednesday") and formats lists.

- Local LLM options: small open models like Qwen3 or Gemma 3 through llama.cpp (macOS and Linux). On Mac, Apple's Foundation Models framework (part of macOS 26, on Macs with Apple Intelligence) costs us no disk or memory.
- Catch: llama.cpp bindings vendor their own ggml, and only one ggml can be linked into a binary next to transcribe.cpp. Cleanup will need a way around that, such as a separate helper process, or a llama.cpp build that shares transcribe.cpp's ggml.
- Costs: extra wait after release, more memory, and the LLM can change what you said.
- Plan: optional, off by default, loaded only when on. Keep the raw text in history.

**B. Fast preview + accurate final text.** Something fast streams text while you talk. On release, the whole recording gets one clean pass, and that result is what gets pasted.

- Done with one model: every model re-decodes the recent audio for the preview and polishes the recording chunk by chunk in final mode, the last chunk on release (see "Live text while speaking"). You get the benefit of B without a second model in memory.

Combined flow: live preview → final transcript → optional cleanup.

## Stack: Rust + GPUI Kit

**Decided** (October 2026), after a spike that built EasyVox twice, once on Tauri and once on GPUI Kit, around the same Rust core. Both worked. GPUI Kit was lighter with no model loaded (one process instead of five, about half the idle memory, much cheaper overlay updates), it's all Rust, and its pop-up window is already the kind of panel the overlay needs. The hands-on checks (real key presses, focus, paste, mic) are still pending and will run on the real app. Details and numbers: `docs/ui-stack-spike.md`, with the full reports in `docs/spikes/`.

The choice is cheap to reverse: everything except the thin shell lives in the core, so a Tauri shell could replace it without touching the rest.

### Workspace layout

A Cargo workspace:

- `crates/easyvox-core`: the UI-agnostic core: audio capture, the speech engines, live partials, paste and history. No UI code, so the shell can change without touching it.
- `crates/easyvox-app`: a thin GPUI Kit shell (binary `easyvox`): menu bar icon, shortcut, overlay, Settings, History, and a controller that drives the core. macOS-specific code lives under `src/platform/`, so a Linux port can add its own module there later.
- `crates/easyvox-bench`: a developer CLI that measures speed, memory and word errors on the test clips. Not shipped.
- `testdata/`: short test clips spoken by macOS voices (`say`), with their transcripts.
- `scripts/`: model download and app bundling.
- `packaging/`: `Info.plist` and other bundle files.
- `docs/`: the stack decision and the spike reports.

### Engine

- **One pipeline for every model, on transcribe.cpp** (ggml, Metal on the GPU). A model's adapter only turns audio into words with timings. The session does everything else the same way for every model: the re-decoded preview, the chunks polished while you talk, and the last chunk on release (`crates/easyvox-core/src/{session,partials,chunks}.rs`, with their numbers in `tuning.rs`). A runtime's quirks stay small flags in its adapter: Whisper, for one, needs segment timestamps on audio over 30 s, or it silently drops text. See "Models" for the list.
- **No native streaming, on purpose.** Parakeet Unified can stream natively, and EasyVox used to. But transcribe.cpp allows one stream or run per model at a time, so polishing chunks meant closing and reopening the stream each time, and the preview hiccupped; it also meant a second, model-specific path. Native streaming was cheaper than the old re-decoded preview (7–9 % of one core while talking, against about 50 %), but the lighter preview now uses about 8 %, so that argument is gone. If the preview ever costs too much again, native streaming could come back as an optimisation for the preview alone; git history has the adapter.
- **Long dictations** (real-time replays of 3- and 9-minute recordings on an M4): the final text arrives 0.37–0.44 s after release, where one pass over the whole recording took 4.1–4.7 s at 3 minutes and 21.2–21.6 s at 9. Word errors held or dropped (Whisper's from 16 % to 0.6 %: it no longer loses text past 30 s), Parakeet's peak memory fell from 0.9–1.2 GB to about 0.8 GB, and the preview kept up for all 9 minutes (its newest words a median 0.6 s or less behind the speech). The cost was the preview: about half of one core and most of the GPU while you talk, until it started passes at the confirmed words and slowed to one every 1.6 s (about 8 % of a core and 0.5 W of GPU now; see "Live text while speaking"). Long pauses no longer cost that much, since the preview rests in them: over a 44 s dictation with 2–8 s pauses, CPU fell from 40 % to 27 % of a core and the GPU from 89 % to 70 %. Real speech seldom pauses that long, though: in 5 minutes of the author's rambling the preview skipped under 2 % of its passes, and used as much as before. Method and numbers: [`docs/model-bench.md`](docs/model-bench.md), "Long dictation" and "Idle and energy".
- **Unloading.** The engine can free its model while the app is idle (`Engine::unload`). A session started afterwards opens the mic at once and loads the model on its worker thread, so audio buffers meanwhile and nothing is lost. Sessions hold their own reference to the model, so unloading never pulls it out from under a running one.
- **sherpa-onnx** is an optional fallback runtime (ONNX Runtime on the CPU, the core's `sherpa` feature). It isn't compiled into the app.
- **One ggml and one ONNX Runtime per binary.** Libraries that vendor their own copy (`whisper-rs`, llama.cpp bindings, `ort`) can't sit next to ours. The linker quietly keeps one copy, so this fails at runtime, not at build time.

### GPUI Kit

GPUI is the GPU-rendered UI framework behind the Zed editor, and GPUI Kit (`longbridge/gpui-kit`, Apache-2.0) adds ready-made components on top. Everything is Rust: no HTML, CSS or JavaScript, and no web view. It runs on macOS, Windows and Linux.

- **Exact version pins.** GPUI Kit ships weekly releases with breaking changes, and pins its own pre-release copy of GPUI. The workspace pins it exactly (`gpui-kit = "=0.7.1"`, which pins `gpui-pre =0.3.8`). Upgrade on purpose, and expect some work each time.
- **Overlay.** GPUI's `PopUp` window kind is already a non-activating panel on macOS, above other windows and on every Space. A small native shim adds click-through and a borderless style, plus a key guard so the overlay can never take keyboard focus (and a paste can never land in it). On Wayland, GPUI's `LayerShell` window kind is the standard way to draw overlays.
- **Menu bar icon and shortcut.** GPUI has neither built in. The `tray-icon` and `global-hotkey` crates (from the Tauri team, used without Tauri) work alongside it.
- **Light and dark.** GPUI Kit starts in light mode. The app has to follow the system appearance itself and switch when it changes (see "Light and dark").
- **Rough edges.** Some mistakes are runtime panics rather than compile errors, and a panic inside an AppKit callback aborts the whole app instead of unwinding. Opening a window uses a deprecated AppKit call to bring it to the front.
- **Builds.** Nothing compiles Metal shaders at build time, so building doesn't need Apple's Metal toolchain (`xcrun metal`). GPUI Kit turns on GPUI's `runtime_shaders` feature, and ggml embeds its shader source (`GGML_METAL_EMBED_LIBRARY`); both compile their shaders when the app first runs. Revisit if a GPUI Kit upgrade drops `runtime_shaders`. Compiles are slow-ish: about 2 minutes for a clean release build on an M4.
- **No bundler, signing or updater.** See "Signing and notarization".

### Engine and macOS gotchas

Learned the hard way in the spike. The code handles each one; they're here so nobody undoes the fix.

- **Microphone hang.** If the Microphone permission hasn't been asked yet, opening the mic hangs with no error. Onboarding asks first, and the core refuses to open the mic until then.
- **First model load is slow once.** The first load under a new app identity (bundle id) compiles ggml's Metal shaders: about 14 s, then about 0.2 s on every later load. Onboarding loads the model right after its download, so the compile happens there (see "Onboarding"). Shipping precompiled shaders would remove it altogether.
- **Idle wake-ups.** ggml's Metal code keeps a timer thread that wakes every 5 ms for as long as its GPU device exists, even after the model is unloaded: about 145 wake-ups a second in the idle app. It exists to keep "residency sets" (GPU memory kept wired) alive for 3 minutes after each decode. Both binaries set `GGML_METAL_NO_RESIDENCY=1` first thing in `main`, so the thread never starts: 0.3 wake-ups a second and no measurable CPU. Dictation is no slower for it: with and without the variable, the first words came 0.46–0.48 s after the key press and the final text 0.07–0.11 s after release, both warm and after 5 minutes idle (bench replays of a 3 s clip, three runs warm and two after the idle each way). It has to be set before the first model load creates the Metal device, and only `main` can set it safely (before any thread exists), so the core lists it in `easyvox_core::process_env()` and leaves setting it to the binaries. `EASYVOX_METAL_RESIDENCY=1` keeps ggml's default, for comparisons. A ~40-line patch could make the loop sleep when idle instead; it isn't needed while the variable costs nothing.
- **Quitting with a model loaded** aborts inside ggml. A hook in the core frees the model at exit. (The abort is an assert on the residency sets, which `GGML_METAL_NO_RESIDENCY=1` never creates; the hook stays for runs that keep them.)
- **Ending a session blocks** until its worker thread finishes, so the app drops sessions off the UI thread.
- **NSURLSession delegate methods must never panic.** objc2 methods are `extern "C-unwind"`, so a panic would unwind into CFNetwork. The download delegate (`download/macos.rs`) only copies what it's given into a channel, with no `unwrap` or indexing, and the file work happens on the downloading thread. Each attempt's session is invalidated when the attempt ends, however it ends, since a session holds on to its delegate until then.
- **Menu bar icons on macOS 26** are drawn by Control Center, so tools that look for the app's own windows don't find them.
- **GPUI doesn't draw while the screen is locked.** Views still ask for redraws, but no frame comes, so the self-test draws the windows it checks itself (`window.draw`), and nothing measured on a locked Mac includes on-screen drawing.
- **Sleeps run long under the agents' shell.** On the measuring Mac, commands run from the agents' (background) shell had their timers coalesced: a 5 ms sleep took about 37 ms and a 66 ms one about 190 ms. The app, started with `open`, isn't affected, but tests that time threads must allow for it.

### Alternatives considered

- **Rust + Tauri.** The more mature path, with a tray, bundler, signing and an updater built in, and Handy as a working example. Dropped: five processes and twice the idle memory, a UI in HTML/CSS/JavaScript, and an overlay that leans on a third-party plugin calling a private AppKit API. Its spike report is kept in `docs/spikes/tauri.md`.
- **Swift + SwiftUI.** The most native and lightest option on Mac, with direct access to every Apple framework. Rules out Linux without a rewrite.
- **Rust core + SwiftUI app.** The most native result, but two UIs to build eventually.
- Applies to any Rust option: Mac-only frameworks (SpeechAnalyzer, Foundation Models, Core ML) need bridging to Swift. We can skip them at first, since Parakeet and llama.cpp cover the core features on both platforms.

## Platform and distribution

- Apple silicon only. Intel Macs are too slow for the bigger models, and macOS 26 is the last release that supports them.
- **macOS 26.0 or later.** Forward-looking on purpose: macOS 27 is close, so supporting older releases isn't worth it. A bonus: the macOS 26 frameworks (SpeechAnalyzer, Foundation Models) need no fallbacks.
- **Bundle id: `dev.easyvox.EasyVox`.** Don't change it after release: macOS ties permission grants and the Metal shader cache to it.
- Permissions: Microphone, Accessibility (paste), possibly Input Monitoring (modifier-only shortcuts).
- Three ways to install:
  - a DMG on GitHub Releases, with auto-updates (updater not chosen yet);
  - a Homebrew cask pointing at that same DMG;
  - build-from-source steps in the README.
- The DMG holds only the app, about 25 MB with the engine (10 MB zipped). Models are downloaded in the app (see "Models").
- No App Store. Its sandbox gets in the way of simulated paste and global key monitoring, so most apps like this skip it.
- **Linux later.** On Wayland (the default on most current Linux desktops), global shortcuts and simulated paste are restricted for security reasons, and GNOME has no layer-shell for overlays. Linux support will take real per-desktop work, not just a rebuild. Handy documents how it handles this. X11 is workable today.

### Signing and notarization (macOS)

- A downloaded app opens without a warning only if it's signed with a Developer ID certificate and notarized by Apple. Both require the paid Apple Developer Program, and there's no free route.
- Without them, macOS blocks the app on first launch. Since macOS 15, the user has to go to System Settings → Privacy & Security → Open Anyway; the old right-click → Open trick no longer works.
- Homebrew's main cask repo has also been phasing out apps that aren't signed and notarized.
- Apps you build yourself from source don't get flagged, so contributors don't need a paid account.
- **Development builds** are signed ad hoc (`codesign -s -`) by `scripts/bundle.sh`, which needs no certificate. The catch: macOS ties the Microphone and Accessibility permissions to the app's signature, and an ad-hoc signature changes on every rebuild, so each rebuild resets those permissions.
- **Shipping builds** need a Developer ID Application certificate. Only the Apple account holder can create one; the current Apple Distribution certificate can't be used for this.
- `cargo-packager` is the planned tool for Developer ID signing, notarization and the DMG. Automate it in GitHub Actions with the certificate and an App Store Connect API key stored as secrets.
- Linux has no equivalent gate. Ship AppImage, .deb or Flatpak.

## Prior art

- **Handy** (`cjpais/Handy`): MIT, Rust + Tauri, Windows/macOS/Linux. Hold-to-talk, paste at cursor, Silero VAD, Whisper and Parakeet v3. Its README describes transcribing after release, with no live preview. Very close to this plan. Its team also makes transcribe.cpp and the GGUF builds of the models EasyVox uses.
- **VoiceInk** (`Beingpax/VoiceInk`): GPL-3.0-based, Swift, macOS. Local Whisper with optional AI cleanup. EasyVox is MIT, so we can't copy its code.
- **Superwhisper, Wispr Flow, MacWhisper**: commercial. Useful for UX ideas.

## Stretch goals

To address after the rest of the app is done.

- **French.** Two versions:
  - **French dictation:** needs a multilingual model. The current English Parakeet models can't do it. Parakeet Ultra is the pick (see "Later: French").
  - **Speak French, get English:** Whisper has a built-in translate-to-English mode, or the optional cleanup LLM could translate. A live English preview while speaking French is the hard part, because translation needs more context than transcription, so the preview would lag or change more.

## Backlog and open questions

The backlog lives on the tuhdoo ledger, not in this file: run `tuhdoo backlog` in the repo (see `CLAUDE.md`). Open questions are held tasks labelled `decision`, and checks that need a person at an unlocked screen are labelled `hands-on`.

Resolved in October 2026, each recorded in its section: the UI layer (Rust + GPUI Kit), the license (MIT, see `LICENSE`), the minimum macOS (26.0), the bundle id (`dev.easyvox.EasyVox`), the model list (Parakeet TDT v2 default; Whisper base.en as the small download; Parakeet Unified bench only), audio in history (no) and the settings file (TOML, applied live).
