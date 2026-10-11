//! The live-text overlay: a pill near the bottom of the screen that shows the transcript while
//! you talk. Plain GPUI (no GPUI Kit components) in a `WindowKind::PopUp` window, which GPUI
//! makes a non-activating panel; `platform::OverlayPanel` adds click-through and the key guard
//! so it can never take focus from the app you're dictating into.
//!
//! While you talk, a small level meter (five bars) moves with your voice and rests flat in
//! silence; when no audio reaches EasyVox it shows "can't hear you" instead (`DESIGN.md`,
//! "Overlay animation"). It redraws only when the core says the level moved visibly
//! (`TranscriptEvent::Level`), which happens only during a dictation, while the pill is up.
//!
//! The colours follow the macOS light or dark appearance ([`Palette`]): read at every draw, and
//! redrawn when the appearance changes, mid-dictation too. Both palettes keep confirmed and
//! provisional text at WCAG AA contrast whatever is behind the pill (see the tests).
//!
//! **Animation** (`DESIGN.md`, "Overlay animation"). Each part is done the cheapest way that
//! does its job, with its timings in the settings file's `[advanced]` table ([`Motion`]):
//! - **Open and close:** a fade and a short slide that Core Animation runs on the panel's layer
//!   (`OverlayPanel::show` and `fade_out`), outside the app: no redraws and no wake-ups. A press
//!   during the close reverses it from wherever it is.
//! - **Inside the pill:** new words fade in, provisional words firm up once confirmed, and the
//!   pill grows and long text scrolls in a glide instead of a jump. Only GPUI can draw these, so
//!   while one runs the pill draws every `overlay_frame_ms`, and once they end it asks for
//!   nothing more (`ask_frame`).
//! - **Reduce motion**, read at each open, zeroes the slide and the glide and keeps the fades:
//!   the same code with other numbers.

use crate::platform::{self, OverlayPanel};
use crate::state::{AppSettings, AppTuning};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::ops::Range;
use std::time::{Duration, Instant};

const WIDTH: f32 = 640.;
const HEIGHT: f32 = 56.;
/// Distance from the bottom of the screen to the bottom of the pill's row.
const BOTTOM_MARGIN: f32 = 140.;
/// Room below the pill's row, inside its window, for the open and close's slide: the most
/// `overlay_slide_pt` allows, so the window's edge never cuts the pill off.
pub const SLIDE_ROOM: f32 = 16.;
/// Around the pill in its row; inside it, the padding at each end, the border, and the gap
/// between the meter and the text.
const MARGIN: f32 = 4.;
const PAD_X: f32 = 20.;
const BORDER: f32 = 1.;
const GAP: f32 = 12.;
const FONT_SIZE: f32 = 16.;
/// The pill less its text: what an empty pill measures.
const CHROME: f32 = 2. * (PAD_X + BORDER) + METER_WIDTH + GAP;
/// The widest the pill grows: its row, less the margin.
const PILL_MAX: f32 = WIDTH - 2. * MARGIN;
/// Text scrolled out at the left fades out over this width, rather than being cut.
const EDGE_FADE: f32 = 16.;
/// Room after the text's measured width, so its last glyph is never clipped.
const TEXT_SLACK: f32 = 2.;

/// The level meter: five bars, `BAR_WIDTH` wide with `BAR_GAP` between them, from
/// `BAR_MIN` (a dot: flat) to `BAR_MAX` tall, the middle one tallest.
const BARS: usize = 5;
const BAR_WIDTH: f32 = 3.;
const BAR_GAP: f32 = 2.;
const BAR_MIN: f32 = 3.;
const BAR_MAX: f32 = 18.;
const BAR_SHAPE: [f32; BARS] = [0.45, 0.75, 1.0, 0.75, 0.45];
const METER_WIDTH: f32 = BARS as f32 * BAR_WIDTH + (BARS - 1) as f32 * BAR_GAP;
/// The dot that stands in for the meter once the text is final, or for a notice.
const DOT: f32 = 10.;

/// Self-test: the time spent in the draws `draw_each_redraw` makes (layout and paint, on the
/// CPU), for `--replay --draw` to report what a draw costs.
#[cfg(feature = "self-test")]
pub static DRAWING_NANOS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// What the pill says when no audio reaches EasyVox.
pub const CANT_HEAR: &str = "Can't hear you. Check your microphone.";

/// The overlay's colours in one appearance. Colours are `0xRRGGBB`, or `0xRRGGBBAA` where they
/// let what's behind show through.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// The pill (`0xRRGGBBAA`): slightly see-through.
    pub background: u32,
    /// Its outline (`0xRRGGBBAA`).
    pub border: u32,
    pub text: u32,
    /// How much provisional text fades (GPUI's `fade_out`: it's drawn at `1 - this` opacity),
    /// as far as it can while keeping AA contrast.
    pub provisional_fade: f32,
    /// The level meter while recording; the dot once the text is final; the dot of a notice and
    /// the meter that can't hear you.
    pub recording: u32,
    pub done: u32,
    pub notice: u32,
}

impl Palette {
    /// The dark pill EasyVox started with, provisional text a little less faded (it was under
    /// AA). Apple's dark-mode system colours.
    pub const DARK: Palette = Palette {
        background: 0x1c1c1ef0,
        border: 0xffffff22,
        text: 0xffffff,
        provisional_fade: 0.45,
        recording: 0xff453a,
        done: 0x30d158,
        notice: 0xffd60a,
    };
    /// A light pill with near-black text. Apple's accessible (high-contrast) light-mode red,
    /// green and orange: the usual ones are too pale on white for UI contrast.
    pub const LIGHT: Palette = Palette {
        background: 0xf5f5f7f0,
        border: 0x0000001a,
        text: 0x1d1d1f,
        provisional_fade: 0.33,
        recording: 0xd70015,
        done: 0x248a3d,
        notice: 0xb25000,
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
/// (the slide, the glide) and keeps the fades, so no animation needs a second code path.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Motion {
    /// The open's fade (and slide), and the close's.
    pub open: Duration,
    pub close: Duration,
    /// How far the pill slides up as it opens and down as it closes, in points.
    pub slide: f32,
    /// New words fading in, and provisional words firming up.
    pub words: Duration,
    /// The pill growing and long text scrolling.
    pub glide: Duration,
    /// Between two frames of the words' fade or the glide.
    pub frame: Duration,
}

impl Motion {
    pub fn new(app: &AppTuning, reduce_motion: bool) -> Self {
        let ms = |ms: u32| Duration::from_millis(ms.into());
        let moving = if reduce_motion { 0 } else { 1 };
        Self {
            open: ms(app.overlay_open_ms),
            close: ms(app.overlay_close_ms),
            slide: (app.overlay_slide_pt * moving) as f32,
            words: ms(app.overlay_words_ms),
            glide: ms(app.overlay_glide_ms * moving),
            frame: ms(app.overlay_frame_ms),
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

/// A quick start that settles gently (cubic ease-out), for `t` from 0 to 1.
fn ease_out(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(3)
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

/// How the new transcript eases in, as the stretches that change: what's new fades in from
/// nothing, provisional words that are confirmed now firm up, and words part-way through an
/// earlier fade carry on from where they are (`old_fade`, with how far it got). The first
/// `kept` bytes are unchanged; the provisional parts start at `old_provisional` and
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

/// The pill's width, and where its text starts: points from the left of the text's box, 0 or
/// less (scrolled).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Geometry {
    width: f32,
    left: f32,
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
        Geometry { width: lerp(self.from.width, self.to.width), left: lerp(self.from.left, self.to.left) }
    }
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
    /// The final text, just before the pill hides.
    Final,
    /// A message (missing permission, model still loading, ...) instead of a transcript.
    Notice,
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
    /// How this showing moves: set at each open ([`Overlay::show`]).
    pub motion: Motion,
    /// The words fading in or firming up, while they do.
    fade: Option<Fade>,
    /// The pill growing or the text scrolling, while it does.
    glide: Option<Glide>,
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
    /// would on screen (it doesn't while the screen is locked), to measure what drawing costs.
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

    /// Whether a fade or glide is running, or a frame for one is due.
    pub fn is_animating(&self) -> bool {
        self.fade.is_some() || self.glide.is_some() || self.frame.is_some()
    }

    /// Whether words are fading, and whether the pill is gliding.
    #[cfg(feature = "self-test")]
    pub fn animations(&self) -> (bool, bool) {
        (self.fade.is_some(), self.glide.is_some())
    }

    /// Asks for the next frame, `motion.frame` from now, unless one is already due. Only
    /// `render` calls it, and only while a fade or glide runs, so frames stop when they end.
    fn ask_frame(&mut self, cx: &mut Context<Self>) {
        if self.frame.is_some() {
            return;
        }
        let wait = self.motion.frame;
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
        let old_k = old_fade.as_ref().map_or(1., |f| ease_out(progress(f.start, self.motion.words, now)));
        (self.confirmed, self.provisional, self.mode) = (confirmed, provisional, mode);
        let (new, provisional) = self.transcript();
        let kept = word_start(&new, common_prefix(&old, &new));
        self.kept = Some(self.kept.map_or(kept, |k| k.min(kept)));
        let eases = eases(kept, old_provisional, old_fade.as_ref().map(|f| (f, old_k)), provisional, new.len());
        self.fade = (!eases.is_empty() && !self.motion.words.is_zero()).then_some(Fade { start: now, eases });
        self.redraw(cx);
    }

    pub fn set_partial(&mut self, confirmed: String, provisional: String, cx: &mut Context<Self>) {
        self.set_text(confirmed, provisional, Mode::Live, cx);
    }

    pub fn set_final(&mut self, text: String, cx: &mut Context<Self>) {
        self.set_text(text, String::new(), Mode::Final, cx);
    }

    pub fn set_notice(&mut self, message: String, cx: &mut Context<Self>) {
        self.set_text(message, String::new(), Mode::Notice, cx);
    }

    /// A note after the text (an error), until the next preview replaces it.
    pub fn set_note(&mut self, note: String, cx: &mut Context<Self>) {
        self.set_text(self.confirmed.clone(), note, self.mode, cx);
    }

    /// The input level. Redraws only if the meter is showing and a bar would change height.
    pub fn set_level(&mut self, level: f32, cx: &mut Context<Self>) {
        self.levels += 1;
        let before = self.meter();
        self.level = level.clamp(0., 1.);
        if self.meter() != before {
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

    /// Whether the pill is recording: it shows the meter rather than a dot.
    fn recording(&self) -> bool {
        matches!(self.mode, Mode::Listening | Mode::Live)
    }

    /// The meter's bars as drawn: their heights in half points (a device pixel on a Retina
    /// screen), or `None` while a dot shows instead. The "can't hear you" meter lies flat.
    fn meter(&self) -> Option<[u8; BARS]> {
        let level = if self.cant_hear { 0. } else { self.level };
        self.recording()
            .then(|| BAR_SHAPE.map(|shape| ((BAR_MIN + (BAR_MAX - BAR_MIN) * level * shape) * 2.).round() as u8))
    }

    /// The text in the pill: the last `max_chars` of [`text`](Self::text) (the pill scrolls to
    /// show the end of it), with the provisional part marked. It's one line: a line break (in
    /// an error, say) shows as a space, which keeps every byte offset.
    fn display_text(&self, max_chars: usize) -> Shown {
        let placeholder = |text: &str| Shown { text: text.into(), provisional: 0..0, origin: None };
        let whole = Origin { start: 0, prefix: 0 };
        let one_line = |text: &str| text.replace('\n', " ");
        match self.mode {
            Mode::Listening | Mode::Live if self.cant_hear => return placeholder(CANT_HEAR),
            Mode::Listening => return placeholder("Listening…"),
            Mode::Notice => return Shown { text: one_line(&self.confirmed), provisional: 0..0, origin: Some(whole) },
            Mode::Live | Mode::Final => {}
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

    /// The pill's geometry now. A glide that has ended is dropped.
    fn geometry_at(&mut self, now: Instant) -> Geometry {
        let Some(glide) = self.glide else { return self.geometry };
        let k = progress(glide.start, self.motion.glide, now);
        if k >= 1. {
            self.glide = None;
            return glide.to;
        }
        glide.at(ease_out(k))
    }

    /// Where the pill and its text go for `shown` (laid out as `line`), gliding there when the
    /// text moved them. The pill only grows during a showing (a word rewritten shorter doesn't
    /// shrink it), and a transcript too long for it sits flush right, so the newest words show. When
    /// the text changes, the words it kept stay where they were on screen and glide from there,
    /// wherever the cut at `overlay_max_chars` falls.
    fn place(&mut self, shown: &Shown, line: &ShapedLine, now: Instant) -> Geometry {
        let text_width = f32::from(line.width()) + TEXT_SLACK;
        let grown = if self.laid.is_some() { self.geometry.width } else { 0. };
        let width = (CHROME + text_width).max(grown).min(PILL_MAX);
        // Only the transcript scrolls; a notice too long for the pill shows its start.
        let flush_right = shown.origin.is_some() && matches!(self.mode, Mode::Live | Mode::Final);
        let left = if flush_right { (width - CHROME - text_width).min(0.) } else { 0. };
        let target = Geometry { width, left };
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
        let from = Geometry { width: current.width, left };
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
        let k = k.map_or(1., ease_out);
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

    /// The meter, or the dot that replaces it, in a box of the meter's width so the text
    /// doesn't move when one replaces the other.
    fn indicator(&self, palette: &Palette) -> Div {
        let slot = div().w(px(METER_WIDTH)).h(px(BAR_MAX)).flex_none().flex().items_center();
        let Some(bars) = self.meter() else {
            let color = if self.mode == Mode::Final { palette.done } else { palette.notice };
            return slot.justify_center().child(div().size(px(DOT)).rounded_full().bg(rgb(color)));
        };
        let color = rgb(if self.cant_hear { palette.notice } else { palette.recording });
        slot.justify_between().children(
            bars.map(|half_points| div().w(px(BAR_WIDTH)).h(px(half_points as f32 / 2.)).rounded_full().bg(color)),
        )
    }
}

/// `text` in the pill's font, to measure it before it's drawn.
fn shape(text: &str, window: &mut Window) -> ShapedLine {
    let mut style = window.text_style();
    style.font_size = px(FONT_SIZE).into();
    let run = style.to_run(text.len());
    window.text_system().shape_line(SharedString::from(text.to_string()), px(FONT_SIZE), &[run], None)
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
        let line = shape(&shown.text, window);
        let geometry = self.place(&shown, &line, now);
        // `HighlightStyle::color` blends over the base colour (white over white stays white),
        // so fading needs `fade_out`.
        let highlights = self.highlights(&shown, palette, now);
        if self.fade.is_some() || self.glide.is_some() {
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
        // The pill's row at the top; below it, the room the open and close slide through.
        div().size_full().child(
            div().w_full().h(px(HEIGHT)).p(px(MARGIN)).flex().justify_center().child(
                div()
                    .w(px(geometry.width))
                    .h_full()
                    .flex_none()
                    .rounded_full()
                    .bg(background)
                    .border(px(BORDER))
                    .border_color(rgba(palette.border))
                    .px(px(PAD_X))
                    .flex()
                    .items_center()
                    .gap(px(GAP))
                    .text_color(rgb(palette.text))
                    .text_size(px(FONT_SIZE))
                    .child(self.indicator(palette))
                    .child(
                        div()
                            .relative()
                            .w(px((geometry.width - CHROME).max(0.)))
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
                    ),
            ),
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
}

impl Overlay {
    /// Shows the pill at once: it fades in and slides up from wherever it is, hidden or part-way
    /// through a close. Reads Reduce motion and the timings for this showing.
    pub fn show(&mut self, cx: &mut App) {
        self.closing = None;
        let motion = Motion::new(&cx.global::<AppSettings>().app, platform::reduce_motion());
        let fresh = !self.panel.is_visible();
        self.view.update(cx, |view, _| {
            view.motion = motion;
            if fresh {
                view.start_over();
            }
        });
        self.panel.show(motion.open, motion.slide.into());
    }

    /// Hides the pill: it fades out and slides down, then is ordered out. Returns at once, so
    /// nothing waits for it, and a show meanwhile takes it back up from where it is.
    pub fn hide(&mut self, cx: &mut App) {
        if !self.panel.is_visible() {
            return;
        }
        let motion = self.view.read(cx).motion;
        let fading = self.panel.fade_out(motion.close, motion.slide.into());
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

/// Creates the overlay window, hidden, at the bottom centre of the primary display.
pub fn create(cx: &mut App) -> anyhow::Result<Overlay> {
    let display = cx.primary_display().ok_or_else(|| anyhow::anyhow!("no display"))?;
    let bounds = display.bounds();
    // The pill's row keeps its place; the slide's room hangs below it.
    let origin = point(
        bounds.origin.x + (bounds.size.width - px(WIDTH)) / 2.,
        bounds.origin.y + bounds.size.height - px(HEIGHT) - px(BOTTOM_MARGIN),
    );
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin, size: size(px(WIDTH), px(HEIGHT + SLIDE_ROOM)) })),
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
    })
}

#[cfg(test)]
mod tests {
    // Not `super::*`: that brings in GPUI's own `test` macro, which shadows `#[test]`.
    use super::{
        BAR_MAX, BAR_MIN, CANT_HEAR, CONFIRMED, Ease, Fade, HIDDEN, Mode, Motion, OverlayView, PROVISIONAL, Palette,
        SLIDE_ROOM, common_prefix, eases, opacity, word_start,
    };
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
            // The meter lies flat.
            assert_eq!(v.meter(), Some([(BAR_MIN * 2.) as u8; 5]));
        }
        let mut v = view("Hello", "");
        (v.mode, v.cant_hear) = (Mode::Final, true);
        assert_eq!(v.shown().0, "Hello");
        assert_eq!(v.meter(), None, "a dot, not the meter");
    }

    #[test]
    fn the_meter_follows_the_level_and_rests_flat() {
        let mut v = view("", "");
        v.mode = Mode::Listening;
        assert_eq!(v.meter(), Some([(BAR_MIN * 2.) as u8; 5]), "flat at 0");
        v.level = 1.;
        let full = v.meter().unwrap();
        assert_eq!(full[2], (BAR_MAX * 2.) as u8, "the middle bar fills the meter");
        assert!(full[0] < full[1] && full[1] < full[2] && full[2] > full[3] && full[3] > full[4]);
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
        assert_eq!(moving.open, Duration::from_millis(150));
        assert_eq!(moving.close, Duration::from_millis(120));
        assert_eq!((moving.slide, moving.glide), (8., Duration::from_millis(150)));
        let still = Motion::new(&app, true);
        assert_eq!((still.slide, still.glide), (0., Duration::ZERO));
        assert_eq!(
            (still.open, still.close, still.words, still.frame),
            (moving.open, moving.close, moving.words, moving.frame)
        );
        // Every timing at 0 turns its animation off.
        let off = AppTuning {
            overlay_open_ms: 0,
            overlay_close_ms: 0,
            overlay_slide_pt: 0,
            overlay_words_ms: 0,
            overlay_glide_ms: 0,
            ..app
        };
        let off = Motion::new(&off, false);
        assert!(off.open.is_zero() && off.close.is_zero() && off.words.is_zero() && off.glide.is_zero());
        assert_eq!(off.slide, 0.);
    }

    #[test]
    fn the_window_has_room_for_the_longest_slide() {
        let slide = AppTuning::default().fields().into_iter().find(|f| f.key == "overlay_slide_pt").unwrap();
        assert_eq!(slide.max.parse::<f32>().unwrap(), SLIDE_ROOM);
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

    /// Contrast ratios in a palette: confirmed text, provisional text and the indicator colours
    /// against the pill. The pill lets 6 % of what's behind it through, so each is the worst
    /// over a black and a white backdrop.
    fn ratios(p: &Palette) -> [f64; 5] {
        let alpha = (p.background & 0xff) as f64 / 255.;
        let mut worst = [f64::INFINITY; 5];
        for backdrop in [[0.; 3], [1.; 3]] {
            let pill = over(channels(p.background >> 8), alpha, backdrop);
            let text = channels(p.text);
            let provisional = over(text, 1. - p.provisional_fade as f64, pill);
            let all = [text, provisional, channels(p.recording), channels(p.done), channels(p.notice)];
            for (w, colour) in worst.iter_mut().zip(all) {
                *w = w.min(contrast(colour, pill));
            }
        }
        worst
    }

    #[test]
    fn text_meets_wcag_aa_contrast_in_light_and_dark() {
        for (name, palette) in [("dark", Palette::DARK), ("light", Palette::LIGHT)] {
            let [text, provisional, recording, done, notice] = ratios(&palette);
            // AA for normal text (the pill's 16 px is under "large text"): 4.5:1.
            assert!(text >= 7.0, "{name}: confirmed text {text:.2}:1");
            assert!(provisional >= 4.5, "{name}: provisional text {provisional:.2}:1");
            // Provisional text still reads as fainter than confirmed text.
            assert!(provisional < text / 2., "{name}: {provisional:.2} vs {text:.2}");
            // The meter and dots, as UI graphics (WCAG 1.4.11): 3:1.
            for (what, ratio) in [("recording", recording), ("done", done), ("notice", notice)] {
                assert!(ratio >= 3.0, "{name}: {what} {ratio:.2}:1");
            }
        }
    }

    #[test]
    fn the_ratios_are_computed_right() {
        // Black on white is 21:1, and a colour against itself 1:1.
        assert!((contrast([0.; 3], [1.; 3]) - 21.).abs() < 1e-9);
        assert!((contrast(channels(0x777777), channels(0x777777)) - 1.).abs() < 1e-9);
        // #767676 on white is the classic 4.54:1.
        assert!((contrast(channels(0x767676), [1.; 3]) - 4.54).abs() < 0.01);
    }
}
