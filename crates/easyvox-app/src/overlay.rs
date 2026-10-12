//! The live-text overlay: a pill near the bottom of the screen that shows the transcript while
//! you talk. Plain GPUI (no GPUI Kit components) in a `WindowKind::PopUp` window, which GPUI
//! makes a non-activating panel; `platform::OverlayPanel` adds click-through and the key guard
//! so it can never take focus from the app you're dictating into.
//!
//! **The look:** a capsule 44 pt tall and 240–560 pt wide, in a flat 92 % fill with a hairline
//! and a soft shadow. On its left, one slot: a level meter of three vermilion strokes while you
//! talk (dots in silence, gray dots after release), a check on the final text, or a notice's
//! glyph ([`Glyph`]). Then one line of text, with a hint after it in the secondary color while it
//! helps ("Release to paste"). A notice is two lines: what happened, then what to do ([`Notice`]).
//! When no audio reaches EasyVox the pill says it can't hear you. The meter moves only when the
//! core says the level moved visibly (`TranscriptEvent::Level`), which happens only during a
//! dictation, while the pill is up, and its bars then ease to the new level.
//!
//! The colors follow the macOS light or dark appearance ([`Palette`], from `brand.rs`): read at
//! every draw, and redrawn when the appearance changes, mid-dictation too. Both palettes keep
//! confirmed and provisional text at WCAG AA contrast whatever is behind the pill (see the tests).
//!
//! **Where:** bottom center, `overlay_dock_gap_pt` above the Dock, on the screen with the
//! keyboard focus, worked out each time the pill opens ([`Overlay::show`]).
//!
//! **Animation.** Each part is done the cheapest way that does its job, with its timings in the
//! settings file's `[advanced]` table ([`Motion`]):
//! - **Open and close:** a fade and a pop that Core Animation runs on the panel's layer
//!   (`OverlayPanel::show` and `fade_out`), outside the app: no redraws and no wake-ups. The pill
//!   grows from a little small to full size with a slight overshoot as it fades in, and shrinks a
//!   little as it fades out. A press during the close reverses it from wherever it is.
//! - **Inside the pill:** new words fade in, provisional words firm up once confirmed, the
//!   final text's check fades in with them, the pill grows and long text scrolls in a glide
//!   instead of a jump, and the meter's bars ease to each new level. Only GPUI can draw these,
//!   so while one runs the pill draws every `overlay_frame_ms` (every `overlay_meter_frame_ms`
//!   while only the meter moves), and once they end it asks for nothing more (`ask_frame`).
//!   They follow [`curve::EASE_OUT`].
//! - **Reduce motion**, read at each open, zeroes the pop's scale, the glide and the meter's
//!   easing, and keeps the fades: the same code with other numbers.

use crate::brand;
use crate::curve::{self, EASE_OUT};
use crate::platform::{self, OverlayPanel};
use crate::state::{AppSettings, AppTuning};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::ops::Range;
use std::time::{Duration, Instant};

/// The pill: 44 pt tall (60 for a notice's two lines), and 240 to 560 pt wide.
const PILL_HEIGHT: f32 = 44.;
const NOTICE_HEIGHT: f32 = 60.;
const PILL_MIN: f32 = 240.;
const PILL_MAX: f32 = 560.;
/// Inside the pill: the padding at each end, the hairline, and the gap after the meter (and
/// before a hint).
const PAD_LEFT: f32 = 14.;
const PAD_RIGHT: f32 = 20.;
const HAIRLINE: f32 = 0.5;
const GAP: f32 = 12.;
/// The text and its line; a hint, or a notice's second line, and its line.
const FONT_SIZE: f32 = 15.;
const LINE_HEIGHT: f32 = 20.;
const SMALL_SIZE: f32 = 13.;
const SMALL_LINE_HEIGHT: f32 = 18.;
/// The slot left of the text, for the meter, its dots or a glyph. A glyph is 20 pt, centered on
/// it, reaching 2 pt into the padding: the text never moves when one replaces another.
const SLOT: f32 = 16.;
const GLYPH: f32 = 20.;
/// The pill less its text: what an empty pill measures.
const CHROME: f32 = PAD_LEFT + SLOT + GAP + PAD_RIGHT + 2. * HAIRLINE;
/// The soft shadow under the pill, two layers: each is down by its first number and blurred by
/// its second (a Gaussian's sigma, so it fades out within three of them).
const SHADOW: (f32, f32) = (4., 4.);
const CONTACT_SHADOW: (f32, f32) = (1., 1.5);
/// Room around the pill inside its window, for the shadow and the open's overshoot. At the most
/// `overlay_shrink_pct` allows, the widest pill grows 8 pt a side past full size at the pop's
/// peak, shadow and all, so the window's edge never cuts either off (a unit test checks). The
/// pill sits at the window's center, so the pop, which scales about the window's center, is
/// about the pill's, and a notice's taller pill grows 8 pt up and 8 pt down.
const ROOM_X: f32 = 22.;
const ROOM_Y: f32 = 18.;
/// The window: the widest and the tallest pill, and the room around them.
const WINDOW_WIDTH: f32 = PILL_MAX + 2. * ROOM_X;
const WINDOW_HEIGHT: f32 = NOTICE_HEIGHT + 2. * ROOM_Y;
/// Text scrolled out at the left fades out over this width, rather than being cut.
const EDGE_FADE: f32 = 16.;
/// Room after the text's measured width, so its last glyph is never clipped.
const TEXT_SLACK: f32 = 2.;

/// The level meter (`design/tokens.json`, `pill.meter`): three strokes `BAR_WIDTH` wide, their
/// centers `BAR_PITCH` apart, from `BAR_MIN` (a dot: flat) to `BAR_MAX` tall, in the shape the
/// design gives the menu bar's bars (the middle one tallest).
const BARS: usize = 3;
const BAR_WIDTH: f32 = 3.;
const BAR_PITCH: f32 = 6.;
const BAR_MIN: f32 = 3.;
const BAR_MAX: f32 = 20.;
const BAR_SHAPE: [f32; BARS] = [0.45, 1.0, 0.7];

/// Self-test: the time spent in the draws `draw_each_redraw` makes (layout and paint, on the
/// CPU), for `--replay --draw` to report what a draw costs.
#[cfg(feature = "self-test")]
pub static DRAWING_NANOS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// What the pill says when no audio reaches EasyVox, and the hint after it.
pub const CANT_HEAR: &str = "Can't hear you";
const CANT_HEAR_HINT: &str = "Check your microphone.";

/// The overlay's colors in one appearance (`design/tokens.json`, through `brand.rs`). Colors
/// are `0xRRGGBB`, or `0xRRGGBBAA` where they let what's behind show through.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// The pill (`0xRRGGBBAA`): a flat 92 % fill, no blur.
    pub background: u32,
    /// Its 0.5 pt hairline (`0xRRGGBBAA`).
    pub hairline: u32,
    /// The text, and the glyphs.
    pub text: u32,
    /// A hint, a notice's second line, and the meter's dots once you let go.
    pub secondary: u32,
    /// How much provisional text fades (GPUI's `fade_out`: it's drawn at `1 - this` opacity),
    /// as far as it can while keeping AA contrast.
    pub provisional_fade: f32,
    /// The level meter while it listens: the accent.
    pub meter: u32,
    /// The shadow's two layers, and the light along the top edge, inside (`0xRRGGBBAA`).
    pub shadow: u32,
    pub contact_shadow: u32,
    pub highlight: u32,
}

impl Palette {
    /// `#1C1C1E` at 92 %, light text, the dark accent.
    pub const DARK: Palette = Palette {
        background: brand::DARK.pill_fill,
        hairline: 0xffffff26,
        text: brand::DARK.pill_text,
        secondary: brand::DARK.pill_text2,
        provisional_fade: 0.45,
        meter: brand::DARK.accent,
        shadow: 0x00000073,
        contact_shadow: 0x0000004d,
        highlight: 0xffffff1a,
    };
    /// White at 92 %, near-black text, the light accent (3.1:1 at worst: a graphic's 3:1).
    pub const LIGHT: Palette = Palette {
        background: brand::LIGHT.pill_fill,
        hairline: 0x0000001c,
        text: brand::LIGHT.pill_text,
        secondary: brand::LIGHT.pill_text2,
        provisional_fade: 0.35,
        meter: brand::LIGHT.accent,
        shadow: 0x00000024,
        contact_shadow: 0x00000014,
        highlight: 0xffffffff,
    };

    pub fn for_appearance(appearance: WindowAppearance) -> &'static Palette {
        match appearance {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => &Self::DARK,
            WindowAppearance::Light | WindowAppearance::VibrantLight => &Self::LIGHT,
        }
    }
}

/// How the overlay moves during one showing: the `overlay_*` timings of the settings file's
/// `[advanced]` table, with macOS's Reduce motion applied. Reduce motion zeroes the movement
/// (the pop's scale, the glide, the meter's easing) and keeps the fades, so no animation needs a
/// second code path.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Motion {
    /// The open's fade and pop, and the close's.
    pub open: Duration,
    pub close: Duration,
    /// How much smaller than full size the pill pops open from and closes to, as a fraction
    /// (0.15 is 85 % of its size).
    pub shrink: f32,
    /// New words fading in, and provisional words firming up.
    pub words: Duration,
    /// The pill growing and long text scrolling.
    pub glide: Duration,
    /// The level meter's bars easing to a new height.
    pub meter: Duration,
    /// Between two frames of the words' fade or the glide; and while only the meter moves.
    pub frame: Duration,
    pub meter_frame: Duration,
}

impl Motion {
    pub fn new(app: &AppTuning, reduce_motion: bool) -> Self {
        let ms = |ms: u32| Duration::from_millis(ms.into());
        let moving = if reduce_motion { 0 } else { 1 };
        Self {
            open: ms(app.overlay_open_ms),
            close: ms(app.overlay_close_ms),
            shrink: (app.overlay_shrink_pct * moving) as f32 / 100.,
            words: ms(app.overlay_words_ms),
            glide: ms(app.overlay_glide_ms * moving),
            meter: ms(app.overlay_meter_ms * moving),
            frame: ms(app.overlay_frame_ms),
            meter_frame: ms(app.overlay_meter_frame_ms),
        }
    }
}

/// How far through `duration` the time since `start` is, from 0 to 1 (1 for no duration).
fn progress(start: Instant, duration: Duration, now: Instant) -> f32 {
    if duration.is_zero() {
        return 1.;
    }
    (now.saturating_duration_since(start).as_secs_f32() / duration.as_secs_f32()).min(1.)
}

/// How far along `curve` an animation is at `t`, its share of the time.
fn along(curve: curve::Curve, t: f32) -> f32 {
    curve::along(curve, t.into()) as f32
}

/// How strongly a stretch of text is drawn: hidden, provisional (faded) or confirmed (full).
/// Fades ease between them, so the values in between are part-way.
type Strength = f32;
const HIDDEN: Strength = 0.;
const PROVISIONAL: Strength = 1.;
const CONFIRMED: Strength = 2.;

/// The opacity of text drawn at `strength`, provisional text being faded by `provisional_fade`.
fn opacity(strength: Strength, provisional_fade: f32) -> f32 {
    let provisional = 1. - provisional_fade;
    if strength <= PROVISIONAL {
        strength.max(HIDDEN) * provisional
    } else {
        provisional + (strength.min(CONFIRMED) - PROVISIONAL) * provisional_fade
    }
}

/// One stretch of the transcript (a byte range of [`OverlayView::text`]) easing between two
/// strengths.
#[derive(Clone, Debug, PartialEq)]
struct Ease {
    range: Range<usize>,
    from: Strength,
    to: Strength,
}

/// The words fading since `start`: only the stretches that change.
#[derive(Clone, Debug)]
struct Fade {
    start: Instant,
    eases: Vec<Ease>,
}

impl Fade {
    /// The strength of transcript byte `at` once `k` (eased) of the fade has passed, if it's
    /// fading.
    fn strength(&self, at: usize, k: f32) -> Option<Strength> {
        let e = self.eases.iter().find(|e| e.range.contains(&at))?;
        Some(e.from + (e.to - e.from) * k)
    }
}

/// The byte length of the longest start `a` and `b` share, at a character boundary.
fn common_prefix(a: &str, b: &str) -> usize {
    a.char_indices().zip(b.chars()).find(|((_, x), y)| x != y).map_or(a.len().min(b.len()), |((i, _), _)| i)
}

/// `at` (a byte offset into `text`), moved back to the start of the word it falls inside, if it
/// does, so a word that changed fades in whole.
fn word_start(text: &str, at: usize) -> usize {
    let (before, after) = text.split_at(at);
    let inside = before.ends_with(|c: char| !c.is_whitespace()) && after.starts_with(|c: char| !c.is_whitespace());
    if !inside {
        return at;
    }
    before.char_indices().rev().find(|(_, c)| c.is_whitespace()).map_or(0, |(i, c)| i + c.len_utf8())
}

/// The stretches of the new transcript that fade: new text from nothing, newly confirmed words
/// up to full, and words mid-fade from where they were (`old_fade`, with how far it got). The
/// first `kept` bytes are unchanged; the provisional parts start at `old_provisional` and
/// `provisional`.
fn eases(
    kept: usize,
    old_provisional: usize,
    old_fade: Option<(&Fade, f32)>,
    provisional: usize,
    len: usize,
) -> Vec<Ease> {
    let mut cuts = vec![0, kept, old_provisional.min(kept), provisional, len];
    if let Some((fade, _)) = old_fade {
        cuts.extend(fade.eases.iter().flat_map(|e| [e.range.start, e.range.end]).map(|c| c.min(kept)));
    }
    cuts.iter_mut().for_each(|c| *c = (*c).min(len));
    cuts.sort_unstable();
    cuts.dedup();
    let mut out: Vec<Ease> = vec![];
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let from = match old_fade.and_then(|(fade, k)| fade.strength(a, k)) {
            _ if a >= kept => HIDDEN,
            Some(strength) => strength,
            None if a < old_provisional => CONFIRMED,
            None => PROVISIONAL,
        };
        let to = if a >= provisional { PROVISIONAL } else { CONFIRMED };
        if from == to {
            continue;
        }
        match out.last_mut() {
            Some(last) if last.range.end == a && last.from == from && last.to == to => last.range.end = b,
            _ => out.push(Ease { range: a..b, from, to }),
        }
    }
    out
}

/// The pill's width and height, where its text starts (points from the left of the text's
/// box, 0 or less when scrolled), and the width its hint takes after the text (0 for none).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Geometry {
    width: f32,
    left: f32,
    height: f32,
    hint: f32,
}

/// The pill gliding from one geometry to another since `start`.
#[derive(Clone, Copy, Debug)]
struct Glide {
    start: Instant,
    from: Geometry,
    to: Geometry,
}

impl Glide {
    fn at(&self, k: f32) -> Geometry {
        let lerp = |a: f32, b: f32| a + (b - a) * k;
        Geometry {
            width: lerp(self.from.width, self.to.width),
            left: lerp(self.from.left, self.to.left),
            height: lerp(self.from.height, self.to.height),
            hint: lerp(self.from.hint, self.to.hint),
        }
    }
}

/// The meter's bars easing to the level since `start`, from the level they showed then.
#[derive(Clone, Copy, Debug)]
struct MeterEase {
    start: Instant,
    from: f32,
}

/// Where the shown text starts in the transcript.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Origin {
    /// The transcript byte the shown text starts at, after the cut.
    start: usize,
    /// The bytes of the "…" that marks the cut, if there is one.
    prefix: usize,
}

/// The text in the pill.
#[derive(Clone, Debug, PartialEq)]
struct Shown {
    text: String,
    /// The provisional part, as a byte range of `text`.
    provisional: Range<usize>,
    /// Where `text` comes from in the transcript, or `None` for a placeholder ("Listening…",
    /// "can't hear you").
    origin: Option<Origin>,
}

impl Shown {
    /// Transcript byte `at` as a byte of `text`, unless it's cut off (or a placeholder shows).
    fn index_of(&self, at: usize) -> Option<usize> {
        let o = self.origin?;
        Some(at.checked_sub(o.start)? + o.prefix).filter(|&i| i <= self.text.len())
    }

    /// A byte range of the transcript as one of `text`, as far as it's shown.
    fn range_of(&self, range: &Range<usize>) -> Option<Range<usize>> {
        let o = self.origin?;
        let end = (range.end.checked_sub(o.start)? + o.prefix).min(self.text.len());
        let start = range.start.saturating_sub(o.start) + o.prefix;
        (start < end).then_some(start..end)
    }
}

/// The text the pill last laid out, so the words the next text keeps can stay put on screen.
struct Laid {
    shown: Shown,
    line: ShapedLine,
}

#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    /// Recording, no text yet.
    #[default]
    Listening,
    /// Live preview text.
    Live,
    /// Released: the last preview, the meter settled to gray dots, until the final text.
    Finishing,
    /// The final text, with a check, just before the pill hides.
    Final,
    /// A message (missing permission, model still loading, ...) instead of a transcript.
    Notice,
}

/// The glyph in the pill's slot: the final text's check, and each notice's. Thin strokes in the
/// text color (`brand::GLYPH_*`); the knot for "Getting ready".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Warning,
    NoMicrophone,
    Info,
    Pasted,
    Copied,
    GettingReady,
}

impl Glyph {
    fn path(self) -> &'static str {
        match self {
            Self::Warning => brand::GLYPH_WARNING,
            Self::NoMicrophone => brand::GLYPH_NO_MICROPHONE,
            Self::Info => brand::GLYPH_INFO,
            Self::Pasted => brand::GLYPH_PASTED,
            Self::Copied => brand::GLYPH_COPIED,
            Self::GettingReady => brand::KNOT,
        }
    }
}

/// A notice: what happened, then what to do, after a glyph. Words carry the meaning, never
/// color alone. Text only: the pill can't take a click (it's click-through and never takes
/// focus), so a notice names where the fix is.
#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    pub glyph: Glyph,
    pub title: String,
    pub detail: String,
}

/// Every notice the pill shows. `shortcut` is the dictation shortcut as its keys read (`⌥Space`).
impl Notice {
    fn new(glyph: Glyph, title: &str, detail: impl Into<String>) -> Self {
        Self { glyph, title: title.into(), detail: detail.into() }
    }

    /// A press before macOS has asked about the microphone: the menu bar's setup asks.
    pub fn microphone_not_asked() -> Self {
        Self::new(Glyph::Warning, "Microphone access needed", "Choose Set Up EasyVox… in the menu bar to allow it.")
    }

    /// A press with the microphone refused or restricted.
    pub fn microphone_off() -> Self {
        Self::new(
            Glyph::Warning,
            "Microphone access needed",
            "Allow EasyVox in System Settings › Privacy & Security › Microphone.",
        )
    }

    pub fn no_microphone() -> Self {
        Self::new(
            Glyph::NoMicrophone,
            "No microphone found",
            "Connect a microphone, or choose another input in System Settings › Sound.",
        )
    }

    /// The recording couldn't start for another reason: its first sentence.
    pub fn cant_start(error: &str) -> Self {
        Self::new(Glyph::Warning, "Can't start dictating", first_sentence(error))
    }

    pub fn nothing_heard(shortcut: &str) -> Self {
        Self::new(Glyph::Info, "Didn't catch that", format!("No speech was heard. Hold {shortcut} and try again."))
    }

    /// The text is on the clipboard: Accessibility is off, so ⌘V couldn't be sent.
    pub fn accessibility_off() -> Self {
        Self::new(
            Glyph::Warning,
            "Accessibility needed to paste",
            "Your text was copied instead. Press ⌘V to paste it.",
        )
    }

    /// The text is on the clipboard, and the paste failed (the reason goes to the log).
    pub fn couldnt_paste() -> Self {
        Self::new(Glyph::Copied, "Couldn't paste", "Your text was copied instead. Press ⌘V to paste it.")
    }

    pub fn loading() -> Self {
        Self::new(Glyph::GettingReady, "Getting ready", "Loading the speech model on this Mac.")
    }

    pub fn downloading() -> Self {
        Self::new(Glyph::GettingReady, "Getting ready", "The speech model is still downloading.")
    }

    pub fn no_model() -> Self {
        Self::new(Glyph::Warning, "No speech model yet", "Download one in EasyVox Settings › Models.")
    }

    pub fn model_failed() -> Self {
        Self::new(Glyph::Warning, "The speech model didn't load", "See EasyVox Settings › Models.")
    }
}

/// The first sentence of an error, starting with a capital and ending with a full stop, for a
/// notice's second line. The whole error goes to the log.
fn first_sentence(error: &str) -> String {
    let error = error.trim();
    let end = error.find(". ").map_or(error.len(), |i| i + 1);
    let sentence = error[..end].trim_end_matches('.');
    let mut chars = sentence.chars();
    let capital = chars.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default();
    format!("{capital}{}.", chars.as_str())
}

#[derive(Default)]
pub struct OverlayView {
    /// Stable text. In a long dictation it can still change once, when the core swaps in the
    /// polished text of a finished chunk (see `TranscriptEvent::Partial`).
    pub confirmed: String,
    /// Text that may still change; drawn faded.
    pub provisional: String,
    pub mode: Mode,
    /// The input level, 0 (flat) to 1, from `TranscriptEvent::Level`.
    pub level: f32,
    /// No audio reaches EasyVox (`TranscriptEvent::NoInput`): "can't hear you".
    pub cant_hear: bool,
    /// The notice up now (`Mode::Notice`).
    pub notice: Option<Notice>,
    /// How this showing moves: set at each open ([`Overlay::show`]).
    pub motion: Motion,
    /// The words fading in or firming up, while they do.
    fade: Option<Fade>,
    /// The pill growing or the text scrolling, while it does.
    glide: Option<Glide>,
    /// The meter's bars easing to a new level, while they do.
    meter_ease: Option<MeterEase>,
    /// The final text's check fading in, since then, while it does.
    glyph_fade: Option<Instant>,
    /// Where the pill rests: where the glide ends, or where it is.
    geometry: Geometry,
    /// What the pill last laid out this showing (`None` before its first draw).
    laid: Option<Laid>,
    /// Bytes at the start of the transcript unchanged since the last draw, if it changed.
    kept: Option<usize>,
    /// The next frame of a fade or glide, while one is due.
    frame: Option<Task<()>>,
    /// Levels received, redraws this view asked for (changes), frames its animations asked for,
    /// draws it got, and whether it last drew dark (read by the self-test).
    pub levels: u32,
    pub redraws_asked: u32,
    pub frames_asked: u32,
    pub renders: u32,
    pub drawn_dark: Option<bool>,
    /// Self-test `--replay --draw`: draw this window at each redraw and frame asked for, as GPUI
    /// would on screen, to measure what drawing costs.
    #[cfg(feature = "self-test")]
    pub draw_each_redraw: Option<AnyWindowHandle>,
    _appearance: Option<Subscription>,
}

impl OverlayView {
    /// Asks for a redraw because something changed. Every change to the pill goes through here.
    fn redraw(&mut self, cx: &mut Context<Self>) {
        self.redraws_asked += 1;
        self.invalidate(cx);
    }

    /// The next frame of a fade or glide, unless it has ended since it was asked for.
    fn tick(&mut self, cx: &mut Context<Self>) {
        if self.is_animating() {
            self.frames_asked += 1;
            self.invalidate(cx);
        }
    }

    fn invalidate(&mut self, cx: &mut Context<Self>) {
        cx.notify();
        #[cfg(feature = "self-test")]
        if let Some(window) = self.draw_each_redraw {
            cx.defer(move |cx| {
                let started = Instant::now();
                let _ = window.update(cx, |_, window, cx| window.draw(cx).clear(cx));
                let nanos = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
                DRAWING_NANOS.fetch_add(nanos, std::sync::atomic::Ordering::Relaxed);
            });
        }
    }

    /// Whether a fade, glide or the meter's easing is running, or a frame for one is due.
    pub fn is_animating(&self) -> bool {
        self.fading() || self.meter_ease.is_some() || self.frame.is_some()
    }

    /// Whether words, the check or the pill's size are on the move: they draw at every frame.
    fn fading(&self) -> bool {
        self.fade.is_some() || self.glide.is_some() || self.glyph_fade.is_some()
    }

    /// Whether words are fading, whether the pill is gliding, and whether the meter is easing.
    #[cfg(feature = "self-test")]
    pub fn animations(&self) -> (bool, bool, bool) {
        (self.fade.is_some(), self.glide.is_some(), self.meter_ease.is_some())
    }

    /// Asks for the next frame, unless one is already due: `motion.frame` after the last while
    /// words fade or the pill glides, `motion.meter_frame` while only the meter moves. Only
    /// `render` calls it, and only while one of them runs, so frames stop when they end.
    fn ask_frame(&mut self, cx: &mut Context<Self>) {
        if self.frame.is_some() {
            return;
        }
        let every = if self.fading() { self.motion.frame } else { self.motion.meter_frame };
        // A quarter of a frame early: the draw waits for the screen's next refresh, so a timer
        // of a whole frame, started at a refresh, lands just after the next one and waits for
        // the one after (every other refresh, at 16 ms and 60 Hz: 30 frames a second).
        let wait = every * 3 / 4;
        self.frame = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |view, cx| {
                view.frame = None;
                view.tick(cx);
            });
        }));
    }

    /// Ends any fade or glide where it would end, and drops the frame they asked for. The
    /// overlay calls it once hidden, so nothing runs while it is.
    pub fn settle(&mut self) {
        self.frame = None;
        self.fade = None;
        self.glyph_fade = None;
        self.meter_ease = None;
        if let Some(glide) = self.glide.take() {
            self.geometry = glide.to;
        }
    }

    /// Shows new text. What it keeps of the old stays as it is, what's new fades in, and what
    /// was provisional and is now confirmed firms up.
    fn set_text(&mut self, confirmed: String, provisional: String, mode: Mode, cx: &mut Context<Self>) {
        let now = Instant::now();
        let (old, old_provisional) = self.transcript();
        let old_fade = self.fade.take();
        let old_k = old_fade.as_ref().map_or(1., |f| along(EASE_OUT, progress(f.start, self.motion.words, now)));
        (self.confirmed, self.provisional, self.mode) = (confirmed, provisional, mode);
        let (new, provisional) = self.transcript();
        let kept = word_start(&new, common_prefix(&old, &new));
        self.kept = Some(self.kept.map_or(kept, |k| k.min(kept)));
        let eases = eases(kept, old_provisional, old_fade.as_ref().map(|f| (f, old_k)), provisional, new.len());
        self.fade = (!eases.is_empty() && !self.motion.words.is_zero()).then_some(Fade { start: now, eases });
        self.redraw(cx);
    }

    pub fn set_partial(&mut self, confirmed: String, provisional: String, cx: &mut Context<Self>) {
        // A preview that comes after the release keeps the pill finishing.
        let mode = if self.mode == Mode::Finishing { Mode::Finishing } else { Mode::Live };
        self.set_text(confirmed, provisional, mode, cx);
    }

    /// The shortcut is released: the meter settles to gray dots, the text stays until the final
    /// text replaces it. One redraw, nothing moves.
    pub fn set_finishing(&mut self, cx: &mut Context<Self>) {
        if self.recording() {
            self.mode = Mode::Finishing;
            self.meter_ease = None;
            self.redraw(cx);
        }
    }

    /// The final text, with its check fading in as its words do.
    pub fn set_final(&mut self, text: String, cx: &mut Context<Self>) {
        self.set_text(text, String::new(), Mode::Final, cx);
        self.glyph_fade = (!self.motion.words.is_zero()).then(Instant::now);
    }

    pub fn set_notice(&mut self, notice: Notice, cx: &mut Context<Self>) {
        let title = notice.title.clone();
        self.notice = Some(notice);
        self.set_text(title, String::new(), Mode::Notice, cx);
    }

    /// A note after the text (an error), until the next preview replaces it.
    pub fn set_note(&mut self, note: String, cx: &mut Context<Self>) {
        self.set_text(self.confirmed.clone(), note, self.mode, cx);
    }

    /// The input level. Redraws only if the meter is showing and a bar would change height.
    pub fn set_level(&mut self, level: f32, cx: &mut Context<Self>) {
        self.levels += 1;
        let before = self.meter();
        let now = Instant::now();
        let from = self.level_at(now);
        self.level = level.clamp(0., 1.);
        if self.meter() != before {
            // The bars ease there from where they are, unless motion is reduced: then they jump.
            self.meter_ease = (!self.motion.meter.is_zero()).then_some(MeterEase { start: now, from });
            self.redraw(cx);
        }
    }

    pub fn set_cant_hear(&mut self, cant_hear: bool, cx: &mut Context<Self>) {
        if self.cant_hear != cant_hear {
            self.cant_hear = cant_hear;
            self.redraw(cx);
        }
    }

    /// Back to an empty, listening pill for a new showing.
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.confirmed.clear();
        self.provisional.clear();
        self.mode = Mode::Listening;
        self.level = 0.;
        self.cant_hear = false;
        self.notice = None;
        self.start_over();
        self.redraw(cx);
    }

    /// Forgets the last showing's layout, so a new one starts at its own size: nothing glides
    /// or fades in from before. The pill as a whole fades in anyway.
    fn start_over(&mut self) {
        self.settle();
        self.laid = None;
        self.kept = None;
    }

    /// The transcript so far: confirmed, then provisional. The core sends them as two runs of
    /// words, so they need a space between them.
    pub fn text(&self) -> String {
        self.transcript().0
    }

    /// [`text`](Self::text), and the byte where its provisional part starts.
    fn transcript(&self) -> (String, usize) {
        let joined = self.confirmed.is_empty()
            || self.provisional.is_empty()
            || self.confirmed.ends_with(' ')
            || self.provisional.starts_with(' ');
        let text = format!("{}{}{}", self.confirmed, if joined { "" } else { " " }, self.provisional);
        let provisional = text.len() - self.provisional.len();
        (text, provisional)
    }

    /// Whether the meter moves (it's listening), for the self-test.
    #[cfg(feature = "self-test")]
    pub fn meter_moves(&self) -> bool {
        self.recording()
    }

    /// Whether the pill is recording: its meter moves.
    fn recording(&self) -> bool {
        matches!(self.mode, Mode::Listening | Mode::Live)
    }

    /// The meter's bars at the level they ease to: their heights in half points (a device pixel
    /// on a Retina screen), or `None` while the meter doesn't move. A change that doesn't move
    /// them by half a point isn't drawn.
    fn meter(&self) -> Option<[u8; BARS]> {
        self.bars(self.level).map(|bars| bars.map(|h| (h * 2.).round() as u8))
    }

    /// The bars' heights in points at `level`, or `None` while the meter doesn't move. The
    /// "can't hear you" meter lies flat.
    fn bars(&self, level: f32) -> Option<[f32; BARS]> {
        let level = if self.cant_hear { 0. } else { level };
        self.recording().then(|| BAR_SHAPE.map(|shape| BAR_MIN + (BAR_MAX - BAR_MIN) * level * shape))
    }

    /// The level the bars show at `now`: part-way to `level` while they ease there.
    fn level_at(&self, now: Instant) -> f32 {
        match self.meter_ease {
            Some(e) => e.from + (self.level - e.from) * along(EASE_OUT, progress(e.start, self.motion.meter, now)),
            None => self.level,
        }
    }

    /// The text in the pill: the last `max_chars` of [`text`](Self::text) (the pill scrolls to
    /// show the end of it), with the provisional part marked. It's one line: a line break (in
    /// an error, say) shows as a space, which keeps every byte offset.
    fn display_text(&self, max_chars: usize) -> Shown {
        let placeholder = |text: &str| Shown { text: text.into(), provisional: 0..0, origin: None };
        let one_line = |text: &str| text.replace('\n', " ");
        match self.mode {
            Mode::Listening | Mode::Live if self.cant_hear => return placeholder(CANT_HEAR),
            Mode::Listening => return placeholder("Listening…"),
            Mode::Finishing if self.text().is_empty() => return placeholder("Finishing…"),
            Mode::Notice => return placeholder(&one_line(&self.confirmed)),
            Mode::Live | Mode::Finishing | Mode::Final => {}
        }
        let full = self.text();
        let total = full.chars().count();
        let (prefix, skip) = if total > max_chars { ("…", total - max_chars) } else { ("", 0) };
        let start = full.char_indices().nth(skip).map_or(full.len(), |(i, _)| i);
        let text = format!("{prefix}{}", one_line(&full[start..]));
        // The provisional part is the last `n` characters (fewer if the cut reached into it).
        let n = self.provisional.chars().count().min(total - skip);
        let provisional_start = match n {
            0 => text.len(),
            n => text.char_indices().rev().nth(n - 1).map_or(0, |(i, _)| i),
        };
        let end = text.len();
        Shown { text, provisional: provisional_start..end, origin: Some(Origin { start, prefix: prefix.len() }) }
    }

    /// The hint after the text, and whether the pill may grow to fit it: how to finish before the
    /// first words, what to check while it can't hear you, and "Finishing…" once you let go, only
    /// where it fits already, so the pill doesn't grow for the moment before the final text.
    fn hint(&self) -> Option<(&'static str, bool)> {
        match self.mode {
            Mode::Listening | Mode::Live if self.cant_hear => Some((CANT_HEAR_HINT, true)),
            Mode::Listening => Some(("Release to paste", true)),
            Mode::Finishing if !self.text().is_empty() => Some(("Finishing…", false)),
            _ => None,
        }
    }

    /// The pill's geometry now. A glide that has ended is dropped.
    fn geometry_at(&mut self, now: Instant) -> Geometry {
        let Some(glide) = self.glide else { return self.geometry };
        let k = progress(glide.start, self.motion.glide, now);
        if k >= 1. {
            self.glide = None;
            return glide.to;
        }
        glide.at(along(EASE_OUT, k))
    }

    /// Lays out the pill for `shown` (shaped as `line`, `text_width` wide) and returns its size
    /// and where its text starts, gliding there if the text moved. `hint` is the hint's width and
    /// whether the pill may grow for it. The pill only grows within a showing, and text too long
    /// for it sits flush right so the newest words show. Words kept from the last text stay where
    /// they were on screen.
    fn place(
        &mut self,
        shown: &Shown,
        line: &ShapedLine,
        text_width: f32,
        hint: Option<(f32, bool)>,
        height: f32,
        now: Instant,
    ) -> Geometry {
        let text_width = text_width + TEXT_SLACK;
        let grown = if self.laid.is_some() { self.geometry.width } else { 0. };
        let hint = hint.map_or(0., |(width, may_grow)| {
            let room = GAP + width;
            if may_grow || grown >= CHROME + text_width + room { room } else { 0. }
        });
        let width = (CHROME + text_width + hint).max(grown).clamp(PILL_MIN, PILL_MAX);
        // Only the transcript scrolls; a placeholder or a notice too long for the pill shows its
        // start.
        let flush_right = shown.origin.is_some() && matches!(self.mode, Mode::Live | Mode::Finishing | Mode::Final);
        let left = if flush_right { (width - CHROME - hint - text_width).min(0.) } else { 0. };
        let target = Geometry { width, left, height, hint };
        let current = self.geometry_at(now);
        let kept = self.kept.take();
        let Some(laid) = &self.laid else {
            self.geometry = target;
            return target;
        };
        if laid.shown == *shown && target == self.geometry {
            return current;
        }
        let left = match (laid.shown.origin, shown.origin) {
            (Some(_), Some(_)) => {
                // The end of the words kept, in each layout: keep it where it is on screen.
                let anchor = kept.unwrap_or_else(|| self.text().len());
                let shift =
                    laid.shown.index_of(anchor).zip(shown.index_of(anchor)).map_or(0., |(old, new)| {
                        f32::from(laid.line.x_for_index(old)) - f32::from(line.x_for_index(new))
                    });
                current.left + shift
            }
            // A placeholder came or went: nothing to keep in place.
            _ => target.left,
        };
        let from = Geometry { left, ..current };
        self.geometry = target;
        self.glide = (from != target && !self.motion.glide.is_zero()).then_some(Glide { start: now, from, to: target });
        if self.glide.is_some() { from } else { target }
    }

    /// The faded stretches of `shown`: the provisional part, and while words fade, how far
    /// each has got. In order, without overlaps, as `StyledText` wants them.
    fn highlights(&mut self, shown: &Shown, palette: &Palette, now: Instant) -> Vec<(Range<usize>, HighlightStyle)> {
        let k = self.fade.as_ref().map(|f| progress(f.start, self.motion.words, now));
        // A fade that has ended, or has nothing to show it on (a placeholder is up), stops.
        if k.is_some_and(|k| k >= 1.) || shown.origin.is_none() {
            self.fade = None;
        }
        let fading: Vec<(Range<usize>, &Ease)> = match (&self.fade, k) {
            (Some(fade), Some(_)) => fade.eases.iter().filter_map(|e| Some((shown.range_of(&e.range)?, e))).collect(),
            _ => vec![],
        };
        let k = k.map_or(1., |k| along(EASE_OUT, k));
        let p = &shown.provisional;
        let mut cuts = vec![0, p.start, p.end, shown.text.len()];
        cuts.extend(fading.iter().flat_map(|(r, _)| [r.start, r.end]));
        cuts.sort_unstable();
        cuts.dedup();
        let mut out: Vec<(Range<usize>, HighlightStyle)> = vec![];
        for w in cuts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let strength = match fading.iter().find(|(r, _)| r.contains(&a)) {
                Some((_, e)) => e.from + (e.to - e.from) * k,
                None if p.contains(&a) => PROVISIONAL,
                None => CONFIRMED,
            };
            let fade_out = 1. - opacity(strength, palette.provisional_fade);
            if fade_out < 0.002 {
                continue;
            }
            match out.last_mut() {
                Some((last, style)) if last.end == a && style.fade_out == Some(fade_out) => last.end = b,
                _ => out.push((a..b, HighlightStyle { fade_out: Some(fade_out), ..Default::default() })),
            }
        }
        out
    }

    /// The slot left of the text: the meter while it listens (flat while it can't hear you),
    /// gray dots once you let go, the final text's check (at `check` opacity, fading in), or the
    /// notice's glyph. One width for all, so the text doesn't move when one replaces another.
    fn indicator(&self, palette: &Palette, check: f32, now: Instant) -> Div {
        let slot = div().w(px(SLOT)).h_full().flex_none().flex().items_center().justify_center();
        let glyph = |glyph: Glyph, opacity: f32| {
            let alpha = (opacity.clamp(0., 1.) * 255.).round() as u32;
            svg().path(glyph.path()).flex_none().size(px(GLYPH)).text_color(rgba(palette.text << 8 | alpha))
        };
        let (bars, colour) = match (self.mode, self.bars(self.level_at(now))) {
            (Mode::Final, _) => return slot.child(glyph(Glyph::Pasted, check)),
            (Mode::Notice, _) => return slot.when_some(self.notice.as_ref(), |s, n| s.child(glyph(n.glyph, 1.))),
            (_, Some(bars)) if !self.cant_hear => (bars, palette.meter),
            _ => ([BAR_MIN; BARS], palette.secondary),
        };
        slot.gap(px(BAR_PITCH - BAR_WIDTH)).children(
            bars.map(|height| div().w(px(BAR_WIDTH)).h(px(height)).flex_none().rounded_full().bg(rgb(colour))),
        )
    }
}

/// `text` in the pill's font at `size`, semibold if `bold`, to measure it before it's drawn.
fn shape(text: &str, size: f32, bold: bool, window: &mut Window) -> ShapedLine {
    let mut style = window.text_style();
    style.font_size = px(size).into();
    if bold {
        style.font_weight = FontWeight::SEMIBOLD;
    }
    let run = style.to_run(text.len());
    window.text_system().shape_line(SharedString::from(text.to_string()), px(size), &[run], None)
}

impl Render for OverlayView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        // The app's appearance, read live: the window's own copy only updates when AppKit tells
        // it, which a hidden panel may miss.
        let palette = Palette::for_appearance(cx.window_appearance());
        self.drawn_dark = Some(*palette == Palette::DARK);
        self.renders += 1;
        let max_chars = cx.global::<AppSettings>().app.overlay_max_chars;
        let shown = self.display_text(max_chars as usize);
        let notice = if self.mode == Mode::Notice { self.notice.clone() } else { None };
        let line = shape(&shown.text, FONT_SIZE, notice.is_some(), window);
        let (text_width, height) = match &notice {
            Some(n) => {
                let detail = f32::from(shape(&n.detail, SMALL_SIZE, false, window).width());
                (f32::from(line.width()).max(detail), NOTICE_HEIGHT)
            }
            None => (f32::from(line.width()), PILL_HEIGHT),
        };
        let hint = self.hint();
        let hint_width = hint
            .map(|(text, may_grow)| (f32::from(shape(text, SMALL_SIZE, false, window).width()) + TEXT_SLACK, may_grow));
        let geometry = self.place(&shown, &line, text_width, hint_width, height, now);
        // `HighlightStyle::color` blends over the base color (white over white stays white),
        // so fading needs `fade_out`.
        let highlights = self.highlights(&shown, palette, now);
        // The meter's easing ends when it gets there, or when there's no meter to ease.
        if let Some(e) = self.meter_ease
            && (progress(e.start, self.motion.meter, now) >= 1. || !self.recording() || self.cant_hear)
        {
            self.meter_ease = None;
        }
        // The check fades in with the final text's words.
        let check = match self.glyph_fade {
            Some(start) if progress(start, self.motion.words, now) < 1. => {
                along(EASE_OUT, progress(start, self.motion.words, now))
            }
            _ => {
                self.glyph_fade = None;
                1.
            }
        };
        if self.fading() || self.meter_ease.is_some() {
            self.ask_frame(cx);
        }
        let text = shown.text.clone();
        self.laid = Some(Laid { shown, line });

        let scrolled = geometry.left < -0.5;
        let background = rgba(palette.background);
        let edge = div().absolute().left_0().top_0().h_full().w(px(EDGE_FADE)).bg(linear_gradient(
            90.,
            linear_color_stop(background, 0.),
            linear_color_stop(rgba(palette.background & !0xff), 1.),
        ));
        let text_box = (geometry.width - CHROME - geometry.hint).max(0.);
        let shadow = |color: u32, (down, blur): (f32, f32), inset: bool| BoxShadow {
            color: rgba(color).into(),
            offset: point(px(0.), px(down)),
            blur_radius: px(blur),
            spread_radius: px(0.),
            inset,
        };
        let shadows = vec![
            shadow(palette.shadow, SHADOW, false),
            shadow(palette.contact_shadow, CONTACT_SHADOW, false),
            // A hairline of light along the top, inside.
            shadow(palette.highlight, (HAIRLINE, 0.), true),
        ];
        let body = match notice {
            // What happened, then what to do.
            Some(notice) => div()
                .w(px(text_box))
                .flex_none()
                .flex()
                .flex_col()
                .child(div().font_weight(FontWeight::SEMIBOLD).truncate().child(text))
                .child(
                    div()
                        .text_size(px(SMALL_SIZE))
                        .line_height(px(SMALL_LINE_HEIGHT))
                        .text_color(rgb(palette.secondary))
                        .truncate()
                        .child(notice.detail),
                ),
            None => div()
                .relative()
                .w(px(text_box))
                .h_full()
                .flex_none()
                .flex()
                .items_center()
                .overflow_hidden()
                .child(
                    div()
                        .flex_none()
                        .ml(px(geometry.left))
                        .whitespace_nowrap()
                        .child(StyledText::new(text).with_highlights(highlights)),
                )
                .when(scrolled, |this| this.child(edge)),
        };
        let hint = hint.filter(|_| geometry.hint > 0.5).map(|(hint, _)| {
            div()
                .flex_none()
                .whitespace_nowrap()
                .text_size(px(SMALL_SIZE))
                .text_color(rgb(palette.secondary))
                .child(hint)
        });
        // The pill at the window's center, with room around it for its shadow and the open's
        // overshoot.
        div().size_full().flex().items_center().justify_center().child(
            div()
                .w(px(geometry.width))
                .h(px(geometry.height))
                .flex_none()
                .rounded_full()
                .bg(background)
                .border(px(HAIRLINE))
                .border_color(rgba(palette.hairline))
                .shadow(shadows)
                .overflow_hidden()
                .pl(px(PAD_LEFT))
                .pr(px(PAD_RIGHT))
                .flex()
                .items_center()
                .gap(px(GAP))
                .text_color(rgb(palette.text))
                .text_size(px(FONT_SIZE))
                .line_height(px(LINE_HEIGHT))
                .child(self.indicator(palette, check, now))
                .child(body)
                .children(hint),
        )
    }
}

pub struct Overlay {
    pub view: Entity<OverlayView>,
    pub panel: OverlayPanel,
    /// Read by the self-test, which draws the pill itself: GPUI doesn't while the screen is
    /// locked.
    #[cfg_attr(not(feature = "self-test"), allow(dead_code))]
    pub window: AnyWindowHandle,
    /// Orders the panel out once the close has faded it. A show drops (cancels) it.
    closing: Option<Task<()>>,
    /// Moves the panel to another place, then opens it, on the next turn of the run loop
    /// (`OverlayPanel::move_to` says why). A hide drops (cancels) it.
    opening: Option<Task<()>>,
}

impl Overlay {
    /// Shows the pill at once: it fades in and pops up to full size from wherever it is, hidden
    /// or part-way through a close. Reads Reduce motion and the timings for this showing. Shown
    /// from hidden, it first goes above the Dock of the screen with the keyboard focus; a pill
    /// already up stays where it is.
    pub fn show(&mut self, cx: &mut App) {
        (self.closing, self.opening) = (None, None);
        let app = &cx.global::<AppSettings>().app;
        let motion = Motion::new(app, platform::reduce_motion());
        let fresh = !self.panel.is_visible();
        let moved = fresh.then(|| self.panel.origin_above_dock(app.overlay_dock_gap_pt.into(), PILL_BOTTOM.into()));
        self.view.update(cx, |view, _| {
            view.motion = motion;
            if fresh {
                view.start_over();
            }
        });
        match moved.flatten() {
            // Usually it's where it was last time, and opens at once. Elsewhere (another screen,
            // the Dock moved), it opens a turn of the run loop later, from there.
            Some(origin) => {
                let panel = self.panel.clone();
                self.opening = Some(cx.spawn(async move |_| {
                    panel.move_to(origin);
                    panel.show(motion.open, motion.shrink.into());
                }));
            }
            None => self.panel.show(motion.open, motion.shrink.into()),
        }
    }

    /// Hides the pill: it fades out and shrinks a little, then is ordered out. Returns at once,
    /// so nothing waits for it, and a show meanwhile takes it back up from where it is.
    pub fn hide(&mut self, cx: &mut App) {
        self.opening = None;
        if !self.panel.is_visible() {
            return;
        }
        let motion = self.view.read(cx).motion;
        let fading = self.panel.fade_out(motion.close, motion.shrink.into());
        let (panel, view) = (self.panel.clone(), self.view.clone());
        let order_out = move |cx: &mut App| {
            panel.order_out();
            view.update(cx, |view, _| view.settle());
        };
        if fading.is_zero() {
            self.closing = None;
            return order_out(cx);
        }
        self.closing = Some(cx.spawn(async move |cx| {
            cx.background_executor().timer(fading).await;
            cx.update(order_out);
        }));
    }

    /// Whether the close is under way (faded or fading, not yet ordered out).
    #[cfg(feature = "self-test")]
    pub fn is_closing(&self) -> bool {
        self.panel.is_visible() && self.closing.is_some()
    }
}

/// How far the 44 pt pill's bottom edge is above its window's: the room, and half what a notice
/// adds.
pub const PILL_BOTTOM: f32 = ROOM_Y + (NOTICE_HEIGHT - PILL_HEIGHT) / 2.;

/// Creates the overlay window, hidden, at the bottom center of the primary display; each show
/// moves it to the screen with the keyboard focus.
pub fn create(cx: &mut App) -> anyhow::Result<Overlay> {
    let display = cx.primary_display().ok_or_else(|| anyhow::anyhow!("no display"))?;
    let bounds = display.bounds();
    // The window is the biggest pill and the room around it; the pill pops about its center,
    // which is the window's. It starts at the bottom of the primary display, hidden, and each
    // show moves it above the Dock of the screen with the keyboard focus.
    let (width, height) = (px(WINDOW_WIDTH), px(WINDOW_HEIGHT));
    let origin =
        point(bounds.origin.x + (bounds.size.width - width) / 2., bounds.origin.y + bounds.size.height - height);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin, size: size(width, height) })),
        titlebar: None,
        focus: false,
        show: false,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id: Some(display.id()),
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let mut panel = None;
    let mut view = None;
    let window = cx.open_window(options, |window, cx| {
        panel = Some(OverlayPanel::attach(window, cx));
        let v = cx.new(|cx: &mut Context<OverlayView>| OverlayView {
            // Light or dark changed: redraw in the other palette, even mid-dictation.
            _appearance: Some(cx.observe_window_appearance(window, |view: &mut OverlayView, _, cx| view.redraw(cx))),
            ..Default::default()
        });
        view = Some(v.clone());
        v
    })?;
    Ok(Overlay {
        view: view.ok_or_else(|| anyhow::anyhow!("overlay view not built"))?,
        panel: panel.ok_or_else(|| anyhow::anyhow!("overlay panel not built"))??,
        window: window.into(),
        closing: None,
        opening: None,
    })
}

#[cfg(test)]
mod tests {
    // Not `super::*`: that brings in GPUI's own `test` macro, which shadows `#[test]`.
    use super::{
        BAR_MAX, BAR_MIN, CANT_HEAR, CONFIRMED, Ease, Fade, HIDDEN, Mode, Motion, NOTICE_HEIGHT, Notice, OverlayView,
        PILL_MAX, PROVISIONAL, Palette, SHADOW, WINDOW_HEIGHT, WINDOW_WIDTH, common_prefix, eases, opacity, word_start,
    };
    use crate::curve::{POP, along};
    use crate::state::AppTuning;
    use std::time::{Duration, Instant};

    const MAX_CHARS: usize = 72;

    fn view(confirmed: &str, provisional: &str) -> OverlayView {
        OverlayView {
            confirmed: confirmed.into(),
            provisional: provisional.into(),
            mode: Mode::Live,
            ..Default::default()
        }
    }

    impl OverlayView {
        fn shown(&self) -> (String, std::ops::Range<usize>) {
            let shown = self.display_text(MAX_CHARS);
            (shown.text, shown.provisional)
        }
    }

    #[test]
    fn short_text_is_shown_whole_with_the_provisional_tail_marked() {
        let (text, prov) = view("Hello,", "world").shown();
        assert_eq!(text, "Hello, world");
        assert_eq!(&text[prov], "world");
        let (text, prov) = view("Hello", "").shown();
        assert_eq!(text, "Hello");
        assert!(prov.is_empty());
        let (text, prov) = view("", "Hello").shown();
        assert_eq!((text.as_str(), &text[prov]), ("Hello", "Hello"));
        // Error notes come with their own leading space, and show on one line.
        assert_eq!(view("Hello", "  ⚠ oops").shown().0, "Hello  ⚠ oops");
        assert_eq!(view("Hello", "  ⚠ oops:\nno model").shown().0, "Hello  ⚠ oops: no model");
    }

    #[test]
    fn long_text_keeps_the_newest_words() {
        let confirmed = "é".repeat(100);
        let (text, prov) = view(&confirmed, " naïve").shown();
        assert_eq!(text.chars().count(), MAX_CHARS + 1);
        assert!(text.starts_with('…'));
        assert_eq!(&text[prov], " naïve");
        let shown = view(&confirmed, " naïve").display_text(10);
        assert_eq!(shown.text, "…éééé naïve");
        // Where it starts in the transcript: 96 "é" (2 bytes each) were cut, and the "…" is 3.
        let full = view(&confirmed, " naïve").text();
        assert_eq!(shown.index_of(full.len()), Some(shown.text.len()));
        assert_eq!(shown.index_of(192), Some(3));
        assert_eq!(shown.index_of(190), None, "cut off");
        assert_eq!(shown.range_of(&(0..194)), Some(3..5));
    }

    #[test]
    fn listening_and_notices_show_no_provisional_text() {
        let mut v = view("Allow the microphone", "");
        v.mode = Mode::Notice;
        assert_eq!(v.shown(), ("Allow the microphone".to_string(), 0..0));
        v.mode = Mode::Listening;
        assert_eq!(v.shown().0, "Listening…");
        assert_eq!(v.display_text(MAX_CHARS).origin, None, "a placeholder");
    }

    #[test]
    fn cant_hear_you_replaces_the_text_while_recording_only() {
        for mode in [Mode::Listening, Mode::Live] {
            let mut v = view("Hello", "world");
            (v.mode, v.cant_hear) = (mode, true);
            assert_eq!(v.shown(), (CANT_HEAR.to_string(), 0..0), "{mode:?}");
            // The meter lies flat, and the hint says what to check.
            assert_eq!(v.meter(), Some([(BAR_MIN * 2.) as u8; 3]));
            assert_eq!(v.hint(), Some(("Check your microphone.", true)));
        }
        let mut v = view("Hello", "");
        (v.mode, v.cant_hear) = (Mode::Final, true);
        assert_eq!(v.shown().0, "Hello");
        assert_eq!(v.meter(), None, "a check, not the meter");
    }

    #[test]
    fn hints_help_before_the_first_words_and_after_the_release() {
        let mut v = view("", "");
        v.mode = Mode::Listening;
        assert_eq!(v.hint(), Some(("Release to paste", true)));
        let v = view("Hello", "world");
        assert_eq!(v.hint(), None, "words take the room");
        // Released: the words stay, the meter stops, and "Finishing…" shows only if it fits.
        let mut v = view("Hello", "world");
        v.mode = Mode::Finishing;
        assert_eq!(v.shown().0, "Hello world");
        assert_eq!((v.meter(), v.hint()), (None, Some(("Finishing…", false))));
        // Released before any words: the placeholder says it.
        let mut v = view("", "");
        v.mode = Mode::Finishing;
        assert_eq!((v.shown().0.as_str(), v.hint()), ("Finishing…", None));
        for mode in [Mode::Final, Mode::Notice] {
            let mut v = view("Hello", "");
            v.mode = mode;
            assert_eq!(v.hint(), None, "{mode:?}");
        }
    }

    #[test]
    fn the_meter_follows_the_level_and_rests_flat() {
        let mut v = view("", "");
        v.mode = Mode::Listening;
        assert_eq!(v.meter(), Some([(BAR_MIN * 2.) as u8; 3]), "flat at 0");
        v.level = 1.;
        let full = v.meter().unwrap();
        assert_eq!(full[1], (BAR_MAX * 2.) as u8, "the middle bar fills the meter");
        assert!(full[0] < full[2] && full[2] < full[1]);
        // A level change smaller than half a point on every bar draws the same bars, so it
        // doesn't redraw.
        v.level = 0.5;
        let half = v.meter();
        v.level = 0.51;
        assert_eq!(v.meter(), half);
        v.level = 0.6;
        assert_ne!(v.meter(), half);
    }

    #[test]
    fn reduce_motion_drops_the_movement_and_keeps_the_fades() {
        let app = AppTuning::default();
        let moving = Motion::new(&app, false);
        assert_eq!(moving.open, Duration::from_millis(200));
        assert_eq!(moving.close, Duration::from_millis(120));
        assert_eq!(
            (moving.shrink, moving.glide, moving.meter),
            (0.15, Duration::from_millis(250), Duration::from_millis(120))
        );
        let still = Motion::new(&app, true);
        assert_eq!((still.shrink, still.glide, still.meter), (0., Duration::ZERO, Duration::ZERO));
        assert_eq!(
            (still.open, still.close, still.words, still.frame, still.meter_frame),
            (moving.open, moving.close, moving.words, moving.frame, moving.meter_frame)
        );
        // Every timing at 0 turns its animation off.
        let off = AppTuning {
            overlay_open_ms: 0,
            overlay_close_ms: 0,
            overlay_shrink_pct: 0,
            overlay_words_ms: 0,
            overlay_glide_ms: 0,
            overlay_meter_ms: 0,
            ..app
        };
        let off = Motion::new(&off, false);
        assert!(
            off.open.is_zero()
                && off.close.is_zero()
                && off.words.is_zero()
                && off.glide.is_zero()
                && off.meter.is_zero()
        );
        assert_eq!(off.shrink, 0.);
    }

    #[test]
    fn the_window_has_room_for_the_biggest_pop_and_its_shadow() {
        // The peak past full size, by the share the pill grew.
        let peak = (0..=1000).map(|i| along(POP, i as f64 / 1000.)).fold(0., f64::max);
        let shrink = AppTuning::default().fields().into_iter().find(|f| f.key == "overlay_shrink_pct").unwrap();
        let biggest = (1. + (peak - 1.) * shrink.max.parse::<f64>().unwrap() / 100.) as f32;
        // The widest pill and a notice's height, with the shadow's reach (three sigmas, and down
        // by its offset), at the peak: the pop scales about the window's center, which is the
        // pill's.
        let (down, blur) = SHADOW;
        let half_width = (PILL_MAX / 2. + 3. * blur) * biggest;
        let half_height = (NOTICE_HEIGHT / 2. + down + 3. * blur) * biggest;
        assert!(half_width <= WINDOW_WIDTH / 2., "{half_width}");
        assert!(half_height <= WINDOW_HEIGHT / 2., "{half_height}");
        // With little room to spare: the window is what the overlay's frames cost on screen.
        assert!(half_width > WINDOW_WIDTH / 2. - 2. && half_height > WINDOW_HEIGHT / 2. - 2.);
    }

    #[test]
    fn changed_words_are_found_from_their_start() {
        assert_eq!(common_prefix("Hello world", "Hello there"), 6);
        assert_eq!(common_prefix("Hello", "Hello world"), 5);
        assert_eq!(common_prefix("naïve", "naïf"), 4, "bytes: \"ï\" is two");
        assert_eq!(word_start("Hello there", 7), 6, "inside \"there\"");
        assert_eq!(word_start("Hello there", 6), 6, "at a word's start");
        assert_eq!(word_start("Hello world", 5), 5, "at a word's end");
        assert_eq!(word_start("Hellos", 5), 0);
    }

    #[test]
    fn new_words_fade_in_and_confirmed_words_firm_up() {
        // "Hello there" (all provisional) becomes "Hello there, world": "Hello there," is
        // confirmed now (it firms up), and " world" is new and provisional (it fades in).
        //   old: provisional from 0;  new: "Hello there, " confirmed, "world" provisional.
        let new = "Hello there, world";
        let kept = word_start(new, common_prefix("Hello there", new));
        assert_eq!(kept, 6, "\"there\" became \"there,\": the word fades in again whole");
        let e = eases(kept, 0, None, 13, new.len());
        assert_eq!(
            e,
            [
                Ease { range: 0..6, from: PROVISIONAL, to: CONFIRMED },
                Ease { range: 6..13, from: HIDDEN, to: CONFIRMED },
                Ease { range: 13..18, from: HIDDEN, to: PROVISIONAL },
            ]
        );
        // Nothing changed: nothing fades.
        assert!(eases(5, 5, None, 5, 5).is_empty());
        // A change half-way through a fade carries on from where each word is.
        let fade = Fade { start: Instant::now(), eases: vec![Ease { range: 0..5, from: HIDDEN, to: PROVISIONAL }] };
        let e = eases(5, 0, Some((&fade, 0.5)), 5, 11);
        assert_eq!(
            e,
            [Ease { range: 0..5, from: 0.5, to: CONFIRMED }, Ease { range: 5..11, from: HIDDEN, to: PROVISIONAL }]
        );
    }

    #[test]
    fn strengths_map_to_the_palettes_opacities() {
        for p in [Palette::DARK, Palette::LIGHT] {
            let fade = p.provisional_fade;
            assert_eq!(opacity(HIDDEN, fade), 0.);
            assert!((opacity(PROVISIONAL, fade) - (1. - fade)).abs() < 1e-6);
            assert_eq!(opacity(CONFIRMED, fade), 1.);
            // Half-way from provisional to confirmed is half-way between their opacities.
            assert!((opacity(1.5, fade) - (1. - fade / 2.)).abs() < 1e-6);
        }
    }

    /// WCAG relative luminance of `0xRRGGBB`, from its sRGB channels.
    fn luminance([r, g, b]: [f64; 3]) -> f64 {
        let lin = |c: f64| if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
        0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
    }

    fn channels(rgb: u32) -> [f64; 3] {
        [(rgb >> 16) & 0xff, (rgb >> 8) & 0xff, rgb & 0xff].map(|c| c as f64 / 255.)
    }

    /// `top` at `alpha` over `under`.
    fn over(top: [f64; 3], alpha: f64, under: [f64; 3]) -> [f64; 3] {
        [0, 1, 2].map(|i| top[i] * alpha + under[i] * (1. - alpha))
    }

    fn contrast(a: [f64; 3], b: [f64; 3]) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    /// Contrast ratios in a palette: confirmed text (and the glyphs, drawn in it), provisional
    /// text, the secondary text (hints, a notice's second line, the dots) and the meter, against
    /// the pill. The pill lets 8 % of what's behind it through, so each is the worst over a black
    /// and a white backdrop.
    fn ratios(p: &Palette) -> [f64; 4] {
        let alpha = (p.background & 0xff) as f64 / 255.;
        let mut worst = [f64::INFINITY; 4];
        for backdrop in [[0.; 3], [1.; 3]] {
            let pill = over(channels(p.background >> 8), alpha, backdrop);
            let text = channels(p.text);
            let provisional = over(text, 1. - p.provisional_fade as f64, pill);
            let all = [text, provisional, channels(p.secondary), channels(p.meter)];
            for (w, colour) in worst.iter_mut().zip(all) {
                *w = w.min(contrast(colour, pill));
            }
        }
        worst
    }

    #[test]
    fn text_meets_wcag_aa_contrast_in_light_and_dark() {
        for (name, palette) in [("dark", Palette::DARK), ("light", Palette::LIGHT)] {
            let [text, provisional, secondary, meter] = ratios(&palette);
            // AA for normal text (the pill's 15 and 13 pt are under "large text"): 4.5:1.
            assert!(text >= 7.0, "{name}: confirmed text {text:.2}:1");
            assert!(provisional >= 4.5, "{name}: provisional text {provisional:.2}:1");
            assert!(secondary >= 4.5, "{name}: secondary text {secondary:.2}:1");
            // Provisional text still reads as fainter than confirmed text.
            assert!(provisional < text / 2., "{name}: {provisional:.2} vs {text:.2}");
            // The meter, as a UI graphic (WCAG 1.4.11): 3:1. The glyphs are in the text color.
            assert!(meter >= 3.0, "{name}: meter {meter:.2}:1");
        }
    }

    #[test]
    fn notices_say_what_happened_then_what_to_do() {
        let all = [
            Notice::microphone_not_asked(),
            Notice::microphone_off(),
            Notice::no_microphone(),
            Notice::cant_start("the input device stopped. try again"),
            Notice::nothing_heard("⌥Space"),
            Notice::accessibility_off(),
            Notice::couldnt_paste(),
            Notice::loading(),
            Notice::downloading(),
            Notice::no_model(),
            Notice::model_failed(),
        ];
        for notice in &all {
            // A title without a full stop, then one or two sentences ending in one.
            assert!(!notice.title.is_empty() && !notice.title.ends_with('.'), "{notice:?}");
            assert!(notice.detail.ends_with('.') && notice.detail.starts_with(char::is_uppercase), "{notice:?}");
        }
        assert_eq!(all[3].detail, "The input device stopped.");
        assert_eq!(all[4].detail, "No speech was heard. Hold ⌥Space and try again.");
        assert_eq!(super::first_sentence("no default input device"), "No default input device.");
    }

    #[test]
    fn the_ratios_are_computed_right() {
        // Black on white is 21:1, and a color against itself 1:1.
        assert!((contrast([0.; 3], [1.; 3]) - 21.).abs() < 1e-9);
        assert!((contrast(channels(0x777777), channels(0x777777)) - 1.).abs() < 1e-9);
        // #767676 on white is the classic 4.54:1.
        assert!((contrast(channels(0x767676), [1.; 3]) - 4.54).abs() < 0.01);
    }
}
