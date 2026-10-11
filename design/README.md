# EasyVox — design hand-off

EasyVox is a free, open-source macOS menu-bar dictation app.
- Hold **⌥Space** anywhere and a floating **pill** shows your words live.
- **Let go** and the text is pasted into the app you were typing in.
- **Esc** while holding cancels.
- Everything runs on the Mac. The speech model ships inside the app: there's no account, no sign-in, and no network needed to dictate.

This folder is the complete design package for building it. Every mockup shows **Light and Dark side by side**.

## Start here
| Path | What it is |
|---|---|
| `mockups/index.html` | Every board in order, one scrolling page. Open it first. |
| `mockups/html/*.html` | The same boards as standalone HTML with inline styles and inline SVG. Read these for exact sizes, colours, spacing and copy. |
| `mockups/png/*.png` | The same boards as 1× PNG renders. |
| `tokens.json` | Colours, type sizes, mark geometry, pill metrics and motion timings, machine-readable. |
| `assets/mark/` | Mark, compact mark (the knot) and lockups as SVG, plus a few PNGs. |
| `assets/app-icon/` | App icon SVGs, Icon Composer layers and a ready `AppIcon.appiconset`. |
| `assets/menubar/` | Status-item glyphs, one SVG per state. **Read its README.md** for sizes, template flags and the live meter. |
| `assets/web/` | Favicons, web manifest, OpenGraph and GitHub social images, and `head-snippet.html`. |
| `assets/dmg/` | DMG background (1× and 2×) and **README.md** with the Finder window settings. |

The mockups were rendered where SF Pro isn't installed, so text falls back to Inter. On a Mac they use SF Pro, the intended font, and line lengths may differ by a few pixels.

---

## 1 · Brand
**The mark (locked as "8c"):** one line. A loose knot of thought leaves at mid-height and becomes the wave of a voice.
- The knot is ink and the wave is vermilion.
- The wave has five strokes: true verticals joined by half-circle turns.
- Never redraw, rotate, outline, re-space or recolour parts of it.
- The path data is in `tokens.json → mark.paths` and the SVGs.

| Version | Use | File |
|---|---|---|
| Full, colour | 24 px tall and up, on light or dark | `easyvox-mark-color-on-light.svg`, `…-on-dark.svg` |
| Full, one colour | Single-ink contexts | `easyvox-mark-black.svg`, `-white.svg`, `-ink.svg` |
| **Compact (the knot alone)** | 14–24 px: menu bar, favicon, small UI; always one colour | `easyvox-compact-knot-*.svg` |
| Lockup | Mark at 1.2 × the type size, 0.36 em gap. Below 24 px tall, use the knot. | `easyvox-lockup-*.svg` (the wordmark is live text in SF Pro Display Semibold, −2% tracking) |

- **Stroke:** 2.2 units in the mark's own viewBox (`VB_FULL = -11.4 6.9 46.67 26.2`, `VB_COMPACT = -11.4 8.87 25.1 23.93`). Round caps and joins.
- **Clear space:** half the mark's height on every side.
- **Below 14 px:** use the app icon instead.

### Colour (also in `tokens.json`)
| Token | Light | Dark | Use |
|---|---|---|---|
| ink | `#1D1D1F` | `#F5F5F7` | Knot, text |
| paper | `#FFFFFF` | `#1C1C1E` | Backgrounds |
| ink2 | `#6E6E73` | `#98989D` | Secondary text (5:1 or better) |
| accent (vermilion) | `#F0462D` | `#FF5C3F` | The wave, the listening state, switches. **Graphics only on light (3.7:1).** |
| accent_text | `#D93A22` | `#FF5C3F` | Links and small accents in text (4.6:1 / 5.5:1) |
| button fill | `#D93A22` | `#D93A22` | Primary buttons, white label (4.59:1) |

- Vermilion is the only brand colour. Errors use words plus a thin glyph, never colour alone.
- Menus keep the system highlight (the user's accent colour). Set the app's `AccentColor` asset to vermilion.

## 2 · App icon (`assets/app-icon/`)
- **Grid:** macOS 26, 1024 canvas, 824 body. The glyph is the full mark, 560 px wide and centred on its ink, at stroke 2.6.
- **Appearances:**
  - Default: knot `#1D1D1F` and wave `#F0462D`, on a `#FFFFFF → #E9E9EE` gradient.
  - Dark: knot `#F5F5F7` and wave `#FF5C3F`, on a `#3A3A3E → #141416` gradient.
  - Tinted: the system tints the layers. The example renders are in `svg/`.
- **Icon Composer:** build an `.icon` from `icon-composer-layers/` with the background as a fill or gradient layer, then `knot-*.svg`, then `wave-*.svg`. Keeping the knot and wave as separate layers lets Tinted mode and the glass highlight treat them differently.
- **16 px:** uses the knot alone (`knot-16pt.svg`).
- **Fallback:** `AppIcon.appiconset/` holds the Default appearance as PNGs, 16 to 1024 px, with `Contents.json`, for builds without Icon Composer.
- **Microphone prompt string** (`NSMicrophoneUsageDescription`): "EasyVox hears you only while you hold ⌥Space. Audio is processed on this Mac and never saved."

## 3 · Menu bar (`assets/menubar/`, boards 04–06)
- **Footprint:** one 30 × 16 pt `NSStatusItem` image for every state, so the item never changes width.
- **Ready:** the knot alone, centred.
- **Every other state:** the knot, then a separate glyph beside it.
  - Listening: three vermilion bars that follow your voice. This is the only non-template image.
  - Finishing: three dots.
  - Pasted: a check, shown for 1 s.
  - Paused: two bars.
  - Needs attention: an exclamation mark.
  - Getting ready: a determinate ring.
- **Transitions:** 150 ms crossfades. Nothing loops.
- **Menu** (board 06):
  - A status header: "Ready" with "Hold ⌥Space to talk, let go to paste."
  - Paste Last Dictation ⌃⌥V, History…
  - Microphone ▸, Language ▸, Pause EasyVox
  - Settings… ⌘,, Check for Updates…, About EasyVox
  - Quit EasyVox ⌘Q
  - In Needs attention, the first rows explain the problem and offer the fix, for example "Microphone access is off" with "Open System Settings…".

## 4 · Overlay pill (boards 07–09)
- **Placement:** bottom centre, 24 pt above the Dock (24 pt above the screen edge when the Dock is hidden), on the display with the focused window.
- **Shape and material:**
  - A capsule 44 pt tall (64 pt with two lines) and 240–560 pt wide.
  - Padding: 14 pt left, 12 pt gap, 20 pt right. 0.5 pt hairline and a soft shadow.
  - Light: white at 92% with blur. Dark: `#1C1C1E` at 92%.
- **Content:**
  - Meter: 3 vermilion strokes, 3 pt wide and 6 pt apart, 3–20 pt tall, following the input level with 60 ms easing. They turn into grey dots after release.
  - Live text: SF Pro 15. Older words step down to secondary (`#636366` light, `#98989D` dark). The pill grows to two lines at most and the bottom edge stays put.
  - Hint: 13 pt, secondary.
- **States and copy:**
  - Listening: "Listening…" with the hint "Release to paste".
  - Live text, with the hint "Esc to cancel".
  - Finishing: "Finishing…", static with no spinner.
  - Pasted: "Pasted into ‹App›" with a check.
  - Copied instead: "Copied. Press ⌘V to paste." Used when no text field had focus.
  - Cancelled: "Cancelled — Nothing was pasted".
- **Notices** (same capsule, two lines, at most one button):
  - Microphone access needed / "Allow EasyVox in Privacy & Security to start dictating." [Open Settings]
  - Accessibility needed to paste / "Your text was copied instead. Press ⌘V to paste it." [Open Settings]
  - No microphone found / "Connect a microphone, or choose one in EasyVox Settings."
  - Didn't catch that / "No speech was heard. Hold ⌥Space and try again."
  - Can't type into password fields / "Password fields stay private, so EasyVox isn't listening." In this case the mic is never turned on.
  - Getting ready · "loading the speech model on this Mac", with determinate %.
  - Notices stay 4 s, or 8 s with a button, and leave on Esc, a click or the next ⌥Space.
- **Motion** (`tokens.json → motion`; curves: ease-out `0.2,0.8,0.2,1`, ease-in `0.4,0,1,1`, ease-in-out `0.4,0,0.2,1`):
  - Appear: 160 ms ease-out, fade + 6 pt rise + scale 0.96 → 1.
  - Each new word fades in over 120 ms; width changes take 200 ms.
  - Release → Finishing: 120 ms.
  - The check draws in 180 ms.
  - Holds: Pasted 700 ms, Copied 2 s, Cancelled 500 ms.
  - Dismiss: 140 ms ease-in, fade + 4 pt drop.
  - **Reduce Motion:** fades only, same timings.
  - **No looping animation anywhere.**
- **Contrast:** worst case (pill over pure black or white content): text 12.4:1 or better, secondary 4.7:1 or better, meter 3.1:1 or better. All meet AA.

## 5 · App windows (boards 10–17)
All windows are standard macOS 26 windows using SF Pro 13 for body text.
- **Set up** (first run, 560 × 540 pt, 5 steps with dot progress, one primary button per screen):
  1. Welcome.
  2. Microphone: shows the system prompt, then "Microphone allowed" with a mic picker and an input meter.
  3. Accessibility: waits while the user turns EasyVox on in System Settings. Can be skipped, in which case text is copied instead of pasted.
  4. Your shortcut: Hold to talk / Press to toggle, with a practice field. Includes a variant for when Accessibility was skipped.
  5. Ready: menu bar recap, Open at login, Done.
- **Settings** (760 × 540 pt, inset glass sidebar, grouped forms, changes apply immediately):
  - General: Shortcut with Change…, Mode, Open at login, Show in menu bar, Start and stop sounds.
  - Dictation: Microphone, Language, Live text in the pill, Speech model (Standard or Precise, both stored on this Mac), After you let go (Paste or Copy only), Keep what was on the clipboard.
  - History & Privacy: Keep history (Off / 1 day / 7 days / 30 days / Forever), Show History…, Clear History… with a confirmation alert, and "Audio is processed on this Mac and never saved."
  - About.
- **History** (760 × 520 pt):
  - Toolbar search.
  - Rows grouped by day, showing text, app name, word count and time.
  - The selected row expands with Copy, Paste Again and Delete.
  - Two empty states: "No dictations yet" and "History is off".

## 6 · Distribution (`assets/web/`, `assets/dmg/`, boards 18–20)
- **DMG:** ship the **light** background. Finder keeps labels black over a background picture. Window settings and commands are in `assets/dmg/README.md`.
- **Favicon:** `favicon.svg` is the knot; its ink switches with `prefers-color-scheme`. There's also a `.ico` (16/32/48), `apple-touch-icon.png`, `icon-192/512.png` and `site.webmanifest`. Paste `head-snippet.html` into `<head>`.
- **Social images:** `og-image.png` (1200 × 630) and `github-social-preview.png` (1280 × 640), dark by default, with light variants.

## 7 · Website (boards 21–25)
- **Pages:** homepage and docs (example page: "Permissions"), each at 1440 desktop and 390 phone, light and dark.
- **Type:** headings 64/48 px on desktop and 36/32 px on phone, semibold at −2% tracking. Body 17 px on desktop and 16 px on phone. Content max width 1120 px.
- **Vermilion use:** only the primary button, links and the wave.

---

## Rules from the brief (apply everywhere)
1. Light and dark versions of everything on screen.
2. Pill text meets WCAG AA 4.5:1; graphics meet 3:1.
3. Reduce Motion means fades only.
4. Native SF Pro, standard macOS windows and menus, thin-stroke icons (about 1.5 pt, round caps).
5. No looping animations.
6. Nothing should suggest the cloud: no cloud shapes, sync or accounts. Say "on this Mac".

## Placeholders to confirm before shipping
- **Website:** the domain (`easyvox.app` in mockups, `easyvox.github.io` in `head-snippet.html`), the GitHub URL `github.com/easyvox/easyvox`, the bundle ID `app.easyvox`, "macOS 14 or later", and version 1.0.
- **Speech models:** the sizes (Standard 142 MB, Precise 466 MB) and whether Precise ships in the app.
- **Shortcut:** ⌃⌥V for Paste Last Dictation.
- **FAQ claims on the site:** languages, and Apple-silicon speed.
- **Check for Updates…:** it needs the network. Keep it explicit and user-initiated, or drop it to stay fully offline.
- **Finder (to verify on macOS 26):** whether it really keeps file names black over a DMG background picture.
