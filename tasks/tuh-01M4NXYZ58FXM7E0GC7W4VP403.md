# Hands-on: the restyled app in light and dark (menu bar, pill, notices, Settings, setup, History, Reduce motion)

`tuh-01M4NXYZ58FXM7E0GC7W4VP403`

- **Status:** done
- **Priority:** 0
- **Labels:** `human` `hands-on` `design`
- **Created:** 2026-10-11 16:50 UTC by `brandon`

## Description

Brandon's check at an unlocked screen, built from c214eb7 (dist/EasyVox.app in the main checkout; re-grant Microphone and Accessibility first, since the ad-hoc signature changed). One round for behaviour and look together.
1. The app icon in Finder and the Dock; the microphone prompt's new wording.
2. Menu bar: the knot when ready; vermilion bars while you hold the shortcut; dots while finishing; a check for about 1 s after the paste. Turn the microphone off in System Settings: an exclamation mark, and the menu explains it with a fix. Turn Accessibility off too: does the menu read well with two problems?
3. The pill: 24 pt above the Dock, on the screen with the focused window (try two displays if you have them). The pop, the bars, words fading in, the provisional words fainter; on release, grey dots, then a check with your final text. Is the shadow too tight?
4. Notices: hold the shortcut and say nothing ('Didn't catch that'), and with the mic off ('Microphone access needed'). Two lines, 4 s.
5. Settings: the sidebar, each page, recording a new shortcut (the ring, a refused combination, Cancel), the model menu, downloads, About's header and credits.
6. Setup (Settings > Open setup… or a clean slate) and History (grouped by day, word counts, copy).
7. Switch light and dark while windows are open; then Reduce motion (only fades).
Anything that looks off: say which screen, light or dark.

Added 11 Oct (from Brandon's first feedback; fixed for the next build): Settings › Permissions shows Accessibility allowed within a second of the switch; turning on Hold right ⌥ shows macOS's Input Monitoring prompt and EasyVox appears in that list; after allowing it, right ⌥ works in TextEdit without a relaunch (if not, the copy must ask for a relaunch again).

Brandon, 11 Oct, hands-on round on b31d2c4 (permissions reset, fresh install), walked through step by step: all ten steps fine: setup (Mic prompt wording, Accessibility detected live), the menu bar icon and menu, a dictation (pill position, pop, bars, words, dots, check; menu bar states), the silent-hold notice (4 s feels right), Settings General (shortcut refusal and Cancel), right ⌥ with Input Monitoring (EasyVox listed after the prompt; worked without a relaunch), Models/Permissions/About, History, light and dark (light pill looked right), Reduce motion. Copy changes he asked for: About tagline 'Simple, local voice-to-text.'; History footer 'N dictations · Stored locally'.

## History

### 2026-10-11 21:18 UTC — edit by `brandon`

description edited

### 2026-10-11 21:49 UTC — edit by `brandon`

description edited

### 2026-10-11 21:49 UTC — edit by `brandon`

status held→done
