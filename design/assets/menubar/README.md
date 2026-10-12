# EasyVox menu bar item

The design hand-off's glyphs for the EasyVox status item, one file per state, and how they were meant to behave.
The app's copies, with PNG renders, are in `crates/easyvox-app/assets/brand/menubar/`. The app differs: it has no
Paused state, its Listening bars are still, and its Getting ready ring shows no progress.

## Size and placement
- Every file is **30 × 16 pt** (`viewBox="0 0 30 16"`, 1 SVG unit = 1 pt). Because every state has the same size, a
  variable-length `NSStatusItem` never changes width, so nothing else in the menu bar moves.
- Stroke: **1.471 pt**, round caps and joins (the mark's own line, scaled to 16 pt).
- **Ready**: the knot alone (the compact mark, 16.78 × 16 pt), 6.5 pt from the left. That centers it to within
  0.1 pt and starts it on a whole pixel at 2×.
- **Every other state**: the knot at x = 0, a 2.75 pt gap, then the state glyph in a 10 pt zone
  starting at x = 19.532. Glyphs center on the mid-line, y = 8.
- Import into the asset catalog with **Preserve Vector Data** on and a single scale.

## Files
| File | State | Template image | VoiceOver label |
|---|---|---|---|
| `ready.svg` | Ready | Yes | “EasyVox, ready” |
| `listening-light.svg`, `listening-dark.svg` | Listening | No (plain image) | “EasyVox, listening” |
| `finishing.svg` | Finishing | Yes | “EasyVox, finishing” |
| `pasted.svg` | Pasted | Yes | “EasyVox, pasted” |
| `paused.svg` | Paused | Yes | “EasyVox, paused” |
| `needs-attention.svg` | Needs attention | Yes | “EasyVox, needs attention” |
| `getting-ready.svg` | Getting ready | Yes | “EasyVox, getting ready, 45 percent” |
| `knot-left.svg` | (part) | Yes | none: the knot at its state position, for drawing Listening and Getting ready live in code |

- **Template images** (`image.isTemplate = true`): black `#000000` on transparent. macOS inks them black or white for
  the menu bar and handles the pressed highlight. The Getting ready track uses 30% opacity, which templates keep.
- **Listening is not a template image**, so its bars stay vermilion. Choose the file from the status button's
  `effectiveAppearance`: `listening-light.svg` (knot `#000000`, bars `#F0462D`) on a light menu bar and
  `listening-dark.svg` (knot `#FFFFFF`, bars `#FF5C3F`) on a dark one. Update the choice when the appearance changes.
  Both files show the bars at the mid level.
- Set the label with `button.setAccessibilityLabel(…)` on every state change. Getting ready reads the
  live percentage, for example “EasyVox, getting ready, 45 percent”.

## Listening bars (live meter)
- **Count:** 3. **Pitch:** 3 pt, with centers at x = 20.268, 23.268, 26.268. **Width:** the stroke (1.471 pt), round caps,
  vermilion (`#F0462D` on a light bar, `#FF5C3F` on a dark bar).
- **Height** (ink, caps included): **min 1.471 pt**, which is a dot (a zero-length line with round caps), up to
  **max 14 pt**. Each bar is centered on y = 8. Center-line length = value × 12.529 pt.
- **Level:** take the input RMS in dBFS and map −50 dB to 0 and −10 dB to 1, clamped. Bar values = level × (0.45, 1, 0.7),
  left to right, so the meter keeps a calm, recognizable shape. The exported frames are level 0.75 (mid).
- **Smoothing:** one-pole, with a 50 ms attack and 200 ms release. Update at most once per display refresh, and only
  while listening.
- **Rest as dots:** below −50 dB, all three bars are dots. Nothing moves in silence; there is no idle or looping
  animation.
- **Reduce Motion:** hold the mid-level shape (the exported files) for the whole time. The pill still shows the words.

## Getting ready (determinate)
- A ring with an ink diameter of 10 pt: center (24.532, 8), center-line radius 4.265 pt.
- The track is the stroke at 30% opacity. Progress is an arc in full ink from 12 o'clock, clockwise, sweeping
  progress × 360°. It never spins and has no indeterminate mode.
- At 100%, crossfade to Ready. `getting-ready.svg` shows 45%.

## Other states
- **Finishing:** three 2 pt dots on the 3 pt pitch. Shown after release, until the text is pasted.
- **Pasted:** a check, shown for 1 s, then Ready.
- **Paused:** two 10 pt bars, 3.5 pt apart. Shown until the user resumes.
- **Needs attention:** an exclamation mark. Shown while Microphone or Accessibility access is off, or no microphone is
  connected. The menu explains the problem and offers the fix.

## Transitions
- 150 ms crossfade between state images. Leaving Ready, the knot slides 6.5 pt left (ease-out) while the
  glyph fades in, and the reverse happens on return. With Reduce Motion: crossfades only.
- Nothing loops anywhere.

## Contrast (WCAG graphics, 3:1)
- Template ink on the menu bar: 17.9:1 light, 15.7:1 dark.
- Listening bars: `#F0462D` 3.2:1 on the light bar, `#FF5C3F` 5.1:1 on
  the dark bar (lowest value over the default desktop gradient).
