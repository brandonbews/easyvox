# EasyVox branding brief: assets, views, and a Claude Design prompt

Researched 10 October 2026 from the working tree (including uncommitted overlay changes), `DESIGN.md`, `README.md`, `packaging/`, `scripts/bundle.sh` and the tuhdoo ledger. Line numbers are as of that tree. No recording or transcript content is used; sample dictation text is made up.

**The designs came back on 11 October 2026:** the full hand-off is in [`design/`](../design/README.md) (mockups, tokens, the mark, the app icon, the menu bar glyphs, web and DMG assets). This brief stays as the request it answered.

**Release tiers:** **v1** = needed for the first public release (the notarized DMG). **Later** = can wait.

**Where things stand:** EasyVox has **no app icon at all** (no `CFBundleIconFile` in `packaging/Info.plist`, and `scripts/bundle.sh:70-75` copies no icon, so Finder, Gatekeeper and the permission prompts show the generic app icon). The menu bar icon is a **placeholder microphone drawn in code, with one state** (`tray.rs:53-78`). The overlay pill uses Apple's system colours. Settings, History and onboarding use GPUI Kit's **stock neutral theme**, with no brand colour, logo or icon anywhere in the app.

---

# Part A: the checklist

## Assets

### A1. App icon (v1)
- **What:** the EasyVox.app icon. Nothing exists today.
- **Where it shows:** Finder and Applications; Spotlight; the DMG window; Gatekeeper's first-launch and "Open Anyway" dialogs; the macOS **Microphone prompt** (with `NSMicrophoneUsageDescription`, `Info.plist:28-29`) and **Accessibility prompt**; **System Settings → Privacy & Security** lists (Microphone, Accessibility, Input Monitoring). Onboarding sends people to find EasyVox in those lists (`onboarding.rs:199-262`), so it must read at 16–32 pt. Also Activity Monitor and Force Quit. **Not** in the Dock or ⌘-Tab: the app is an accessory (`LSUIElement`, `Info.plist:22-23`; `main.rs:93-94`). **Not** in notifications: the app sends none. **Not** on the About page or in onboarding today. Adding it there is a small code change (see V7, V3).
- **Sizes and format:** a 1024 × 1024 px master. `.iconset` → `.icns` via `iconutil`: 16, 32, 128, 256 and 512 pt, each at @1x and @2x (16–1024 px). The build would need `CFBundleIconFile` in Info.plist plus a copy step in `bundle.sh`, or cargo-packager's icon list later. macOS 26 option: an Icon Composer `.icon` (layered, with Default, Dark, Clear and Tinted appearances), compiled into `Assets.car` by Xcode 26's `actool`. That adds an Xcode dependency the build avoids today (README: Command Line Tools only).
- **States:** Default, plus Dark and Tinted if Icon Composer is used.
- **Constraints:** the macOS 26 rounded-square shape (icons that don't fill it get put on a grey plate); legible at 16 pt; layered source (glyph separate from background) so both formats are easy; nothing that implies the cloud.

### A2. Menu bar (status item) icon (v1: idle, recording, attention; later: loading, downloading)
- **What:** the only always-visible piece of EasyVox. Today: one static template microphone, a 36 × 36 px RGBA buffer drawn in code (`tray.rs:53-78`), with the tooltip "EasyVox" (`tray.rs:48`). **The code never changes it.** Every state below is new and needs a code change to switch (tray-icon's `set_icon`).
- **Sizes and format:** a template image (black plus alpha only; macOS tints it for light, dark and tinted menu bars), about 18 pt tall (in a ~22–24 pt bar), @1x 18 px and @2x 36 px tall. It can be a little wider than tall. Deliver PNG @1x/@2x, plus SVG or PDF source.
- **States to design:**
  1. **Idle**: ready, model loaded or freed (unload is invisible by design).
  2. **Recording**: while the shortcut is held. macOS already shows its own orange microphone indicator while the mic is open, so this can stay subtle, e.g. filled against outlined.
  3. **Getting ready**: the model's first load under a new install (~15 s, `onboarding.rs:309-336`), or a load at launch.
  4. **Needs attention**: a permission is missing, no model is downloaded, the model failed to load, or the shortcut couldn't be registered (`controller.rs:299-301`; `state.rs` `shortcut_error`).
  5. *(Later)* **Downloading**: a model download is running.
- **Constraints:** monochrome; **static, with no animation loop** (lightness: an animated status item redraws all the time); distinct from Apple's own Dictation mic and the system's orange indicator; consistent with the mark (A4).

### A3. Speaking indicator and the overlay's glyphs (v1: sign-off on the existing ones; optional redesign)
- **Where:** `overlay.rs:63-73` (geometry), `:105-124` (colours), `:721-731` (drawing). Spec: DESIGN.md "Overlay animation" (lines 84-106) and "Light and dark" (108-114).
- **Level meter:** five bars, each 3 pt wide with 2 pt gaps (23 pt wide), in an 18 pt tall slot. Bars run from 3 pt (a dot) to 18 pt, shaped 0.45 / 0.75 / 1 / 0.75 / 0.45 (so at most 9.75 / 14.25 / 18 / 14.25 / 9.75 pt). Fully rounded caps. Colour "recording": `#ff453a` dark / `#d70015` light.
  - **Flat (silence):** five 3 pt dots in the recording colour.
  - **Moving:** about 15 updates a second, only on visible change. Each eases over 120 ms (CSS ease-out). Room noise rests flat.
  - **Can't hear you:** flat dots in **amber** ("notice": `#ffd60a` dark / `#b25000` light) with the text "Can't hear you. Check your microphone." (`overlay.rs:81`).
- **Done dot:** a 10 pt circle in green ("done": `#30d158` dark / `#248a3d` light), replacing the meter on the final text.
- **Notice dot:** the same 10 pt circle in amber, for every notice. Success-ish ("Copied…"), info ("still loading") and problems all look the same today, so a design could offer distinct glyphs.
- **Error mark:** an inline "⚠" text character after the transcript, faded like provisional text (`controller.rs:389`).
- **Overflow cues:** a 16 pt fade at the pill's left edge when text has scrolled (`overlay.rs:59`), and a "…" prefix once the text is cut at 200 characters.
- **Constraints:** fits the 23 × 18 pt slot (the text mustn't move when meter and dot swap); ≥ 3:1 against the pill in both modes (system colours are used for this: the accessible variants in light); no extra draws (the meter is "nearly free" today, so keep it that way).

### A4. Logo: the mark (v1)
- **What:** a symbol for EasyVox. Nothing exists.
- **Uses:** README header, website header and favicon, About page header, onboarding header, DMG background, social cards; it informs the app icon (A1) and menu bar glyph (A2).
- **Formats:** SVG master; full-colour, one-colour, and reversed (on dark); PNG exports. A minimum size (it must hold at 16 px for the favicon).
- **Constraints:** simple enough to become a monochrome 18 pt template glyph; independent of colour; no cloud cues.

### A5. Logo: wordmark and lockups (v1)
- **What:** "EasyVox" (capital E and V, as in `Info.plist:6-8` and everywhere in the UI) as a wordmark, plus a horizontal lockup (mark + wordmark). A stacked lockup is optional.
- **Uses:** README (GitHub light and dark, via `<picture>` with `prefers-color-scheme`), website, About page, DMG, social preview.
- **Formats:** SVG plus PNG @1x/@2x, light and dark versions.

### A6. DMG installer window (v1)
- **What:** the window that opens from the DMG. Release path: cargo-packager builds, signs, notarizes and wraps it (ledger `tuh-01M4H2EGNVZ00YA6604MXABTDP`; DESIGN.md lines 337-341, 353). The DMG holds only the app (~25 MB).
- **Sizes and format:** a background image at @1x and @2x (or one multi-resolution TIFF) for a window of about 660 × 400 pt. cargo-packager's DMG settings take the background, window size, app position and Applications-folder position. A volume icon (`.icns`) is optional (later).
- **Content:** app icon left, Applications alias right, a drag cue between them, wordmark. Maybe a one-line "Requires macOS 26 on Apple silicon."
- **Constraints:** Finder doesn't swap the background for dark mode, but icon labels change colour (white in dark mode), so check both appearances. Keep the file small.

### A7. Favicon set (v1 if the website launches with the app; else later)
- **Formats:** `favicon.svg` (it can carry its own `prefers-color-scheme` colours), `favicon.ico` (16/32/48), `apple-touch-icon.png` 180 × 180, plus 192/512 for a manifest (optional).
- **Context:** the site is planned (ledger `tuh-01M4MJQZ9ZPPHC1MZY01T63XB8`, "keep the website very light"). README already links **easyvox.dev** (`README.md:7`), and the bundle id is `dev.easyvox.EasyVox`.

### A8. GitHub social preview and OpenGraph image (v1 if the repo goes public with the release)
- **Sizes:** GitHub social preview 1280 × 640 px (min 640 × 320, under 1 MB); OpenGraph/Twitter card 1200 × 630 px. Keep content in a centred safe area (cards crop).
- **Content:** mark + wordmark, the line "A free, open source voice-to-text app for macOS." or "Hold a shortcut and speak.", optionally a pill showing live text.
- **Context:** the repo is private now; the cask task says it must be public first (`tuh-01M4H2EGPP2HDY5Y1WD6ENDTTS`).

### A9. Onboarding illustrations (later, optional)
- **Today:** none. Step badges are a number in a ring, Lucide `CircleCheck` (green) when done, Lucide `TriangleAlert` (red) on a problem, and a spinner only during the ~15 s first load (`onboarding_window.rs:208-221`).
- **Candidates:** a header image (app icon + "Set up EasyVox"); the "All set" footer (holding ⌥Space); a small diagram of the switch in System Settings → Accessibility (`onboarding.rs:237-262`).
- **Constraints:** static (DESIGN.md line 54: only the first load gets a spinner, since it redraws every frame); light and dark; sits beside Lucide's thin-stroke icons; small files.

### A10. Accent colour and theme tokens (later, optional)
- **Today:** GPUI Kit's stock "Default Light/Dark" theme (Tailwind neutral greys: primary near-black in light and near-white in dark; success green-500/400; danger red-500/400). The overlay uses Apple system colours (A3).
- **Would change:** primary buttons (Download, Use, Allow…, Get ready, Start dictating), the shortcut recorder's focus ring, the current step's badge and the spinner, links, and the "Active" tag. It needs a theme override in code, and contrast checked in both modes.

## Views

All windows are standard titled macOS windows, centred on open, using the system font (SF Pro), and following light/dark live (DESIGN.md lines 17, 110). Window sizes come from `controller.rs:965, 1002, 1026`.

### V1. The overlay pill (v1: exists; design refines it)
- **Code:** `overlay.rs` (constants 38-73, palettes 105-124, text 604-625, render 742-814, window 874-915). Spec: DESIGN.md lines 84-114.
- **Geometry:** the window is 652 × 56 pt, centred, bottom 140 pt above the bottom of the primary display. The pill is a capsule **48 pt tall** (56 less a 4 pt margin), with a 1 pt border, 20 pt padding at each end and 12 pt between indicator and text. Text is 16 pt system font, one line. An empty pill is 77 pt plus its text, ~150 pt for "Listening…", **max 632 pt** (text area ≤ 555 pt). It grows with the text (250 ms glide) and never shrinks mid-dictation. Long text sits flush right and scrolls left.
- **Colours:** dark `#1c1c1e` at 94 % opacity, border white at 13 %, text `#ffffff`, provisional text at 55 % opacity. Light `#f5f5f7` at 94 %, border black at 10 %, text `#1d1d1f`, provisional at 67 %. Contrast: confirmed 14.5:1 dark / 13.6:1 light; provisional 5.5:1 / 5.1:1 (AA, whatever is behind). No window shadow (`platform/macos.rs:31-33`), and any shadow or glow has only ~4–6 pt of room unless the window grows.
- **Behaviour:** never takes focus; click-through; above everything, on every Space and over full-screen apps.
- **Motion:** opens on the key press (200 ms: fades in while growing from 85 % to 101.5 % and settling at 100 %). Closes 0.6 s after the paste (120 ms: fades while shrinking to 85 %). New words fade in over 200 ms; provisional words firm up over 200 ms. **Reduce motion:** no scale, no glide, no meter easing; fades only.
- **States (each in light and dark):**

| # | State | Indicator | Text (exact) | Source |
|---|---|---|---|---|
| 1 | Listening | red, flat (5 dots) | `Listening…` | `overlay.rs:610` |
| 2 | Listening, hearing you | red bars moving | `Listening…` | `:588-591` |
| 3 | Live preview | red bars | confirmed text at full strength + provisional tail faded | `:604-625` |
| 4 | Live, long | red bars | scrolled flush right, left 16 pt fades; `…` prefix past 200 chars | `:59`, `:616` |
| 5 | Final | green 10 pt dot | the whole text, full strength; holds 0.6 s, then closes | `controller.rs:412`, `state.rs:56` |
| 6 | Nothing heard | amber dot | `Didn't catch anything` | `controller.rs:409` |
| 7 | Can't hear you | amber, flat (5 dots) | `Can't hear you. Check your microphone.` | `overlay.rs:81` |
| 8 | Model still loading | amber dot | `The speech model is still loading…` | `controller.rs:301` |
| 9 | No model | amber dot | `No speech model downloaded yet: see Settings` | `:300` |
| 10 | Model failed | amber dot | `The speech model didn't load: see Settings` | `:299` |
| 11 | No mic permission | amber dot | `Microphone access needed: menu bar icon → Settings… → Permissions` | `:273` |
| 12 | Copied, not pasted | amber dot | `Copied. Allow Accessibility in Settings to paste automatically` | `:432` |
| 13 | Paste failed | amber dot | `Copied. Paste failed: {error}` | `:434` |
| 14 | Can't start | amber dot | `Can't start: {error}` | `:314` |
| 15 | Error mid-dictation | red bars | transcript + faded `  ⚠ {error}` (e.g. `⚠ final decode failed: …`) | `:389` |

  Notices (6, 8–14) stay up for 2.5 s (`state.rs:59`). A notice too long for the pill shows its start. After an unload, a press shows state 1 at once while the model reloads (~0.15 s), so there's no "loading" pill in normal use.

### V2. The menu bar menu (v1: content fixed; design the icon only)
- **Code:** `tray.rs:15-20, 29-50`. A native NSMenu (no custom drawing).
- **Items:** `Settings…` · `History…` · separator · `About EasyVox` (opens Settings on its About page) · `Quit EasyVox`. Debug builds add `Test dictation (WAV replay)` and a separator at the top. Tooltip: `EasyVox`.
- **Not in code (ideas only):** a status line at the top ("Ready: hold ⌥Space", "Getting ready…", "Setup incomplete"). That would pair with A2's states.

### V3. "Set Up EasyVox" onboarding (v1: exists; design refines it)
- **Code:** `onboarding_window.rs` (header 185-194, step row 208-254, buttons 258-271, footer 292-319); step logic and strings in `onboarding.rs:199-336`. Spec: DESIGN.md lines 45-58. Window **640 × 640 pt**, title "Set Up EasyVox". Note: the header says "Set up EasyVox", lowercase u.
- **Header:** `Set up EasyVox` / "A few things before your first dictation. Speech turns into text on this Mac; the only download is the speech model." There's no icon today; the design could add one.
- **Step row:** a 22 pt badge (number in a ring, primary when it's the current step; green tick when done; red triangle on a problem; spinner only during warm-up), then the title, a sentence (red on a problem), and buttons (first primary, rest outline).
  1. **Microphone**: to do: "EasyVox listens only while you hold the shortcut. macOS asks you once." [Allow…] · asking: "Waiting for your answer in the macOS prompt…" · done: "Allowed." · denied: "Microphone access is off, so EasyVox can't hear you. Turn EasyVox on in System Settings → Privacy & Security → Microphone." [Open System Settings…] · restricted: "This Mac's settings don't allow microphone access (Screen Time or a management profile)." · unknown: "macOS didn't say whether EasyVox may use the microphone. Check System Settings → Privacy & Security → Microphone."
  2. **Accessibility**: to do: "Lets EasyVox paste the text where you're typing, with ⌘V. Without it, the text waits on the clipboard for you to paste." [Allow…] [Open System Settings…] · after the prompt: "Turn EasyVox on in System Settings → Privacy & Security → Accessibility. This step ticks itself off when you do." [Open System Settings…] · done: "Allowed."
  3. **Speech model**: to do: "It turns your speech into text, on this Mac. It's downloaded once, from Hugging Face; no audio or text ever leaves the Mac." It shows the **model row** (as in V5) underneath: Download → progress bar (220 pt) "Downloading… 120 MB of 539 MB" [Cancel] · "Checking the copy that's already here…" · "The connection dropped. The download picks up where it stopped, by itself." (row: "Connection lost at 120 MB of 539 MB. Trying again (2 of 5)…") · done: "Downloaded and checked." · offline: "This Mac seems to be offline. Connect to the internet, then click Try again." · wrong file: "What arrived wasn't the expected file, so EasyVox didn't keep it. Click Try again to download it afresh." · no space: "Free up some disk space, then click Try again." · other: "Click Try again to start the download over." [Try again]
  4. **Get ready**: waiting or to do: "Once the model is here, EasyVox loads it once to set up this Mac's graphics chip: about 15 seconds, the first time only. Then your first dictation starts at once." [Get ready] · busy (spinner): "Getting ready… The first time takes about 15 seconds; after that, the model loads in a fraction of a second." · done: "Ready. Your first dictation starts at once." · failed: "Couldn't load the model: {error}" [Try again]
- **Footer:** unfinished: "{n} of 4 done. Settings → Permissions opens this again." [Finish later] (outline) · finished: "All set. Hold [⌥Space] and speak; let go to paste." [Start dictating] (primary).
- **Fallback:** "This build offers no speech model." (`:179`).
- **Context:** the system Microphone prompt shows the app icon and "EasyVox listens while you hold the push-to-talk shortcut and turns your speech into text, on this Mac only."

### V4. Settings: General page, with the shortcut recorder (v1: exists)
- **Code:** `settings.rs:61-174` (page), `:379-415` (recorder). Window **880 × 560 pt**, title "EasyVox Settings": a sidebar with a search field and the pages General, Models, Permissions, About (GPUI Kit `Settings` component).
- **Group "Dictation":**
  - **Model**: a dropdown of downloaded models, or "None downloaded yet". The description shows the engine's state, then " Download, switch or delete models under Models.": "Not loaded." · "Not downloaded yet (539 MB)." · "Downloading… 120 MB of 539 MB" · "Loading… The first load after installing takes about 15 s." · "Loaded in 0.2 s. Used for both the live preview and the final text." · "Freed after 10 minutes without use. It loads again as you start your next dictation." · "Couldn't load: {error}".
  - **Keep model loaded**: a switch. "Off: after 10 minutes without dictation, EasyVox frees the model's ~600 MB. Your next dictation records at once while it loads again (about 0.15 s), so nothing you say is lost. On: it stays in memory."
  - **Push-to-talk shortcut**: the recorder (a 140 × 30 pt min, rounded, bordered field). Idle shows a keycap `⌥Space`; recording shows "Press shortcut…" (muted) with a focus-ring border; error (small, red, below): "Add a modifier (⌃ ⌥ ⇧ ⌘)" or a registration error; or "Not set". Description: "Hold it and speak; let go to paste. Click, then press a new combination."
  - **Hold right ⌥ to talk**: a switch. "Works alongside the shortcut. Needs Input Monitoring." (or its error).
  - **Settings file**: the path (shown with `~`) and [Show in Finder]. "Every setting is in this file, including advanced ones this window doesn't show (docs/settings.md lists them). Edits to it apply as soon as you save, and changes here are written to it." Warning variants start with "⚠".

### V5. Settings: Models page, with the model row (v1: exists)
- **Code:** `settings.rs:177-208`; row `model_row.rs:114-257`. Spec: DESIGN.md lines 166, 211.
- **Group "Speech models":** "EasyVox runs one model at a time, on this Mac. A switch applies from your next dictation."
- **Rows (two offered models):**
  - `Parakeet TDT 0.6B v2 (recommended)`, "539 MB download · Pick this if you want the best all-round model." Credits: "Parakeet TDT 0.6B v2 by NVIDIA, licensed under [CC BY 4.0]." / "Converted to GGUF and quantized to Q5_K_M by [handy-computer]. [Model card]"
  - `Whisper base.en (smallest)`, "85 MB download · Pick this if you want the smallest model (an 8 GB Mac, or memory matters more than the last bit of accuracy)." Credits: "Whisper base.en by OpenAI, licensed under [Apache 2.0]." / "Converted to GGUF and quantized to Q8_0 by [handy-computer]. [Model card]"
- **Row states:** Not downloaded [Download] (primary) · Downloading: progress bar + "Downloading… 120 MB of 539 MB" [Cancel] · Verifying (an indeterminate bar) · Reconnecting text · Failed: a red line "Download failed: {sentence}" [Try again] · Downloaded: "539 MB on this Mac" [Use] (primary) [Delete] · Delete confirm: [Keep] [Delete 85 MB] (danger) · Active: a green "Active" tag, Delete disabled with the tooltip "This is the model in use. Use another one first." · Active + loading: a muted "Loading…" next to the name · Active + load failed: "Couldn't load: {error}" in red.
- **Last item:** "Models folder": the path and [Show in Finder], "Each model is saved here with a LICENSE.txt that credits it."

### V6. Settings: Permissions page (v1: exists)
- **Code:** `settings.rs:210-275, 309-318`.
- **Group "Setup":** "Set up EasyVox" [Open setup…]: "The first launch's steps again: the Microphone, Accessibility and the speech model."
- **Group "macOS privacy":**
  - Microphone: "Allowed" · "Not asked yet" [Allow…] · "Waiting for your answer…" · "Denied" [Open System Settings…] · "Restricted by this Mac's policy" · "Unknown" [Open System Settings…]. "Needed for dictation. EasyVox listens only while you hold the shortcut."
  - Accessibility: "Allowed" / "Not allowed" [Open System Settings…]. "Pastes the text at the cursor. Without it, the text is on the clipboard for ⌘V."
  - Input Monitoring: same. "Only for holding right ⌥ to talk. Relaunch EasyVox after allowing it."

### V7. Settings: About page (v1: add the icon and wordmark)
- **Code:** `about.rs:75-178`. Opened by the menu's "About EasyVox". There's no separate About window.
- **Group "EasyVox":** `EasyVox 0.1.0` (medium weight) / "© 2026 Brandon Bews. Released under the MIT License." / "Speech is turned into text on this Mac: no audio or text leaves it." [License]. **No icon or wordmark today**: the main in-app brand moment to design.
- **Group "Speech models":** "EasyVox downloads these models from Hugging Face when you choose them. They aren't part of EasyVox and keep their own licenses. Each one is saved with a LICENSE.txt that says the same." Per model: short name; credit line; "Original model: huggingface.co/nvidia/parakeet-tdt-0.6b-v2"; modification line; "Provided as is, without warranties of any kind." Footer: "EasyVox isn't affiliated with or endorsed by NVIDIA or OpenAI."
- **Group "Open-source software":** "EasyVox is built on open-source software: GPUI (Zed Industries), GPUI Kit (Longbridge), transcribe.cpp and ggml, SQLite, Lucide icons and about 450 Rust crates. Their licenses and notices are in one file." [Third-party notices].

### V8. History window (v1: exists)
- **Code:** `history.rs:109-216`. Window **560 × 640 pt**, title "EasyVox History". Text only, no audio (DESIGN.md line 132). Loads the newest 500.
- **Header:** `History` (large, semibold) / "N transcripts" ("1 transcript").
- **Row (56 pt tall):** the text on one truncated line; meta "{ago} · {seconds} s · {model name}", e.g. "3 min ago · 4.2 s · Parakeet TDT 0.6B v2 (recommended)" (ago: "just now", "N min ago", "N h ago", "N d ago"); a copy button with the tooltip "Copy".
- **Empty:** "No transcripts yet" / "Hold [⌥Space] and speak. Everything you dictate shows up here."
- **Error banner:** "History problem: {error}" in red.
- **Later (ledger inbox):** search, delete, a retention limit.

### V9. Website homepage (later; v1 if the site ships with the app)
- **Context:** ledger `tuh-01M4MJQZ9ZPPHC1MZY01T63XB8`: a monorepo with a **very light** site, "a basic homepage and some docs". Lightness is a feature, docs stay Markdown, and nothing goes public without Brandon. Domain in README: easyvox.dev.
- **Sections and content (from README and DESIGN.md):**
  1. **Hero:** wordmark; "A free, open source voice-to-text app for macOS."; "Hold a shortcut and speak. EasyVox shows the words as it hears them, and when you let go it pastes the text where your cursor is."; Download (DMG) and GitHub buttons; the pill as a static or CSS-animated demo.
  2. **Private:** the model runs on your Mac; no audio or text leaves it; the only network use is downloading the model; History keeps text, never audio.
  3. **Light:** idle, it does nothing (wakes about once every 3 s); while dictating it adds about 0.65 W on the test Mac; the model is freed after 10 minutes unused (~90 MB idle). Keep the caveat: one Mac (M4), one speaker.
  4. **How it works:** hold ⌥Space → watch the words (confirmed and provisional) → let go and it's pasted (and stays on the clipboard).
  5. **Models:** Parakeet TDT 0.6B v2 (default, 539 MB, best all-round); Whisper base.en (85 MB, smallest). Downloaded in the app, checked, credited.
  6. **Requirements:** Apple silicon, macOS 26 or later. English for now.
  7. **Install:** DMG (GitHub Releases), Homebrew cask (`brew install --cask easyvox`), build from source. Show "coming soon" or an early-development banner until a release exists (README: "Early development, not usable yet").
  8. **Open source:** MIT, the GitHub link; footer with licence, model credits and the no-affiliation line.

### V10. Website docs page (later)
- **Layout:** sidebar navigation + an article, light and dark, a phone layout.
- **Suggested sections:** Install (DMG, Homebrew, first-launch Gatekeeper) · First launch (the four onboarding steps, and why each permission) · Dictating (the shortcut, holding right ⌥, confirmed vs provisional text, "Didn't catch anything", "Can't hear you") · Models (choose, download, delete, where they live, licences) · Settings (the window, and the settings file applied live, with every key) · History · Privacy (the only connections, where data lives, no audio stored) · Troubleshooting (paste needs Accessibility, so text waits on the clipboard; dev builds lose permissions at each rebuild) · Build from source · Uninstall.
- **Results page:** planned later (ledger inbox `tuh-01M4MGCF0JZW3A508YWHTS4140`: a condensed, visual page of the benchmark and efficiency findings, with honest caveats and no transcripts). Leave a nav slot; don't design it.

## Design constraints (from the docs and code)

- **Overlay:** "Subtle and quick. It should feel responsive, never slow you down, and never draw attention away from what you're typing into." (DESIGN.md line 86). It's non-activating, never takes key focus, and is click-through (`platform/macos.rs:26-33`).
- **Light and dark:** every window follows the system and switches live, mid-dictation too; the menu bar icon is a template (DESIGN.md lines 17, 108-114).
- **Contrast:** confirmed and provisional text ≥ 4.5:1 in both modes whatever is behind the pill; meter and dots ≥ 3:1 (DESIGN.md line 113; unit-tested).
- **Reduce motion:** no pop scale, no glide, no meter easing; fades stay (DESIGN.md line 103).
- **Lightness as a product value:** "light and performant as a feature" (DESIGN.md line 10). Animations run only while something moves; no loops; a spinner only for the ~15 s first load; the meter updates ~15 times a second, only on visible change. Applies to the site too.
- **macOS-native:** SF system font (the overlay uses the system UI font at 16 pt; GPUI Kit's theme follows the system font); native menu; Apple system colours in the pill; Lucide stroke icons in Settings and onboarding; accessory app with no Dock icon or app menu; Apple silicon, macOS 26+.
- **Private:** nothing leaves the Mac. Avoid cloud imagery.
- **Real defaults for mockups:** shortcut ⌥Space; version 0.1.0; models as in V5; "© 2026 Brandon Bews"; MIT.

## Questions for Brandon that would change the list

1. **Dock:** EasyVox is menu-bar only (no Dock or ⌘-Tab). Should it ever show in the Dock, e.g. while Settings or onboarding is open? If so, the icon also needs to work there, and the app needs an app menu.
2. **Name and domain:** is "EasyVox" final and **easyvox.dev** registered? README links it and the bundle id uses it. This affects the favicon, social cards and site.
3. **Menu bar states:** the icon never changes today. Which states should it switch through (recording overlaps macOS's orange mic dot)? Each one needs code.
4. **Icon format:** plain `.icns` (no Xcode needed, as now) or Icon Composer's `.icon` (Dark, Clear and Tinted on macOS 26, but the build needs Xcode 26)?
5. **Brand colour in the app:** may the meter's red, the done green and the notice amber, or GPUI Kit's neutral theme, take a brand accent?
6. **Website and repo at v1:** will the site and a public repo launch with the first DMG? That decides whether favicon and social cards are v1.
7. **Auto-updater** (undecided, `tuh-01M4H2EGN1Y7XVCGM2JVNSTZ3A`): Sparkle would add an update window showing the icon.

---

# Part B: the Claude Design prompt

Paste everything between the BEGIN and END markers. It is under 1,200 words, with no internal paths, setting keys or private content, and the sample dictation is invented.

<!-- BEGIN PROMPT -->
**EasyVox: brand identity and interface mockups**

**The product.** EasyVox is a free, open-source dictation app for the Mac. Hold a shortcut (⌥Space by default) and speak. A small floating pill near the bottom of the screen shows your words as they're recognised, and when you let go the text is pasted at your cursor. The speech model runs entirely on the Mac: no audio or text leaves it. EasyVox lives in the menu bar (no Dock icon) and needs an Apple silicon Mac on macOS 26 or later.

**Personality.** Light, quick, private, unobtrusive, native to macOS. Being light on the machine is a feature: idle, it does nothing, and dictating should never wake the fans. A small, well-made Mac utility that gets out of the way. Nothing should suggest the cloud.

**Direction is open.** I'm not setting colours, a typeface or a mascot. Propose two or three distinct directions for the mark, each with a one-line rationale. You may use or ignore two existing elements: the pill and its five-bar level meter.

**Constraints**
- Light and dark versions of everything on screen.
- In the pill, both full-strength and faded "provisional" text meet WCAG AA (4.5:1) in both modes over any background (the pill is 94% opaque). Graphics need 3:1.
- Reduce motion: fades only, no scaling or sliding.
- Native: the system font (SF Pro), standard macOS windows and menus, thin-stroke line icons.
- No looping animations; static, small illustrations.

**Work in this order, in labelled frames, light and dark side by side.**

**1. Mark and wordmark.** Two or three mark directions, a wordmark "EasyVox" (capital E and V) and a horizontal lockup. Show each in colour and in one colour, at 16, 32 and 128 px. Pause here if you can, so I can pick.

**2. App icon.** A macOS 26 rounded-square icon (1024 px master), legible at 16 and 32 pt, where people find it in System Settings' privacy lists. Show Default, Dark and Tinted looks, in context: Finder and the macOS Microphone prompt ("EasyVox listens while you hold the push-to-talk shortcut and turns your speech into text, on this Mac only.").

**3. Menu bar icon, all states side by side**, on light and dark menu bars. It's a monochrome template image (macOS tints it), about 18 pt tall, @1x and @2x, static. States:
- Idle
- Recording (macOS already shows an orange mic dot)
- Getting ready (the first model load, about 15 s)
- Needs attention (a missing permission or no model)

Show the menu open: Settings…, History…, a separator, About EasyVox, Quit EasyVox.

**4. The overlay pill, all states side by side**, light and dark.
- **Shape:** a capsule 48 pt tall that grows with its text up to 632 pt wide, with one line of 16 pt text.
- **Indicator:** a 23 × 18 pt slot on the left: five 3 pt bars 2 pt apart, rising from dots to 18 pt, the middle one tallest.
- **Placement:** centred, 140 pt above the screen's bottom; it never takes focus, clicks pass through it, and there's only ~4 pt of room for a shadow.

States:
- "Listening…" with a flat meter (five dots), then with the bars at a few levels
- Live: "Can we move the design review to Thursday afternoon?" at full strength, then "I'd like the whole team there" in the fainter provisional style
- Long dictation: the newest words held at the right, older ones fading out at the left edge
- Final: the full text with a small green "done" dot in place of the meter
- "Can't hear you. Check your microphone." with the bars flat in amber
- Notices, each with a dot:
  - "Didn't catch anything"
  - "The speech model is still loading…"
  - "No speech model downloaded yet: see Settings"
  - "Microphone access needed: menu bar icon → Settings… → Permissions"
  - "Copied. Allow Accessibility in Settings to paste automatically"
- An error after some text: "Thanks for the update", then a faded "⚠ final decode failed"

Today the meter is system red and every notice shares one amber dot; you may propose distinct glyphs for done, info and problems. Storyboard the motion: the open fades in while growing from 85% with a slight overshoot (200 ms), the close fades and shrinks (120 ms), new words fade in, and Reduce motion drops the scaling.

**5. App windows.**
- **Set up EasyVox** (640 × 640 pt).
  - **Header:** "A few things before your first dictation. Speech turns into text on this Mac; the only download is the speech model."
  - **Steps,** each with a badge (number, tick, warning or spinner), title, sentence and buttons:
    - Microphone: "EasyVox listens only while you hold the shortcut. macOS asks you once." [Allow…]
    - Accessibility: "Lets EasyVox paste the text where you're typing, with ⌘V." [Allow…] [Open System Settings…]
    - Speech model: a row for "Parakeet TDT 0.6B v2 (recommended)", "539 MB download · Pick this if you want the best all-round model." [Download]. While downloading: a progress bar, "Downloading… 120 MB of 539 MB" [Cancel].
    - Get ready: a spinner and "Getting ready…"
  - **Footer:** "2 of 4 done." [Finish later]. When finished: "All set. Hold ⌥Space and speak; let go to paste." [Start dictating].
- **Settings** (880 × 560 pt), with a searchable sidebar:
  - **General:**
    - Model: a dropdown
    - Keep model loaded: a switch
    - Push-to-talk shortcut: a recorder showing ⌥Space, with "Press shortcut…" while it records
    - Hold right ⌥ to talk: a switch
  - **Models:** Parakeet TDT 0.6B v2 (Active, 539 MB) and Whisper base.en (smallest, 85 MB).
    - States: [Download], downloading with [Cancel], [Use] [Delete], the confirmation [Keep] [Delete 85 MB], and a failed download with [Try again].
  - **Permissions:** Microphone, Accessibility and Input Monitoring, each "Allowed", or "Not allowed" with [Open System Settings…]; and [Open setup…].
  - **About:** "EasyVox 0.1.0", "© 2026 Brandon Bews. Released under the MIT License.", [License], the model credits and [Third-party notices]. Add a header with the icon and wordmark.
- **History** (560 × 640 pt).
  - **Header:** "History", with "12 transcripts".
  - **Rows:** 56 pt tall, each with one line of invented text, "3 min ago · 4.2 s · Parakeet TDT 0.6B v2" and a copy button.
  - **Empty state:** "No transcripts yet" / "Hold ⌥Space and speak. Everything you dictate shows up here."

**6. Distribution.**
- A DMG installer background (about 660 × 400 pt, @1x and @2x) with the app icon, Applications and a drag cue. Finder labels turn white in dark mode, so check both.
- A favicon set.
- A GitHub social preview (1280 × 640) and an OpenGraph card (1200 × 630).

**7. Website**, very light pages, desktop and phone.
- **Homepage:**
  - A hero: "A free, open source voice-to-text app for macOS." and "Hold a shortcut and speak.", with a pill demo
  - Private: everything stays on your Mac
  - Light: idle, it does nothing
  - How it works
  - The two models
  - Requirements
  - Install (DMG, Homebrew, source), marked "coming soon"
  - Open source (MIT)
- **Docs page:** a sidebar and an article.

A benchmark results page comes later: leave a navigation slot, but don't design it.
<!-- END PROMPT -->
