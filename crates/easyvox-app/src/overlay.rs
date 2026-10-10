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
//! TODO(feature work): the other animations in DESIGN.md "Overlay animation" (open and close,
//! new words easing in).

use crate::platform::OverlayPanel;
use gpui_kit::*;

const WIDTH: f32 = 640.;
const HEIGHT: f32 = 56.;
/// Distance from the bottom of the screen.
const BOTTOM_MARGIN: f32 = 140.;

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
    /// Levels received, redraws this view asked for, draws it got, and whether it last drew
    /// dark (read by the self-test).
    pub levels: u32,
    pub redraws_asked: u32,
    pub renders: u32,
    pub drawn_dark: Option<bool>,
    /// Self-test `--replay --draw`: draw this window at each redraw asked for, as GPUI would on
    /// screen (it doesn't while the screen is locked), to measure what drawing costs.
    #[cfg(feature = "self-test")]
    pub draw_each_redraw: Option<AnyWindowHandle>,
    _appearance: Option<Subscription>,
}

impl OverlayView {
    /// Asks for a redraw. Everything that changes the pill goes through here.
    fn redraw(&mut self, cx: &mut Context<Self>) {
        self.redraws_asked += 1;
        cx.notify();
        #[cfg(feature = "self-test")]
        if let Some(window) = self.draw_each_redraw {
            cx.defer(move |cx| {
                let _ = window.update(cx, |_, window, cx| window.draw(cx).clear(cx));
            });
        }
    }

    pub fn set_partial(&mut self, confirmed: String, provisional: String, cx: &mut Context<Self>) {
        self.confirmed = confirmed;
        self.provisional = provisional;
        self.mode = Mode::Live;
        self.redraw(cx);
    }

    pub fn set_final(&mut self, text: String, cx: &mut Context<Self>) {
        self.confirmed = text;
        self.provisional.clear();
        self.mode = Mode::Final;
        self.redraw(cx);
    }

    pub fn set_notice(&mut self, message: String, cx: &mut Context<Self>) {
        self.confirmed = message;
        self.provisional.clear();
        self.mode = Mode::Notice;
        self.redraw(cx);
    }

    /// A note after the text (an error), until the next preview replaces it.
    pub fn set_note(&mut self, note: String, cx: &mut Context<Self>) {
        self.provisional = note;
        self.redraw(cx);
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

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.confirmed.clear();
        self.provisional.clear();
        self.mode = Mode::Listening;
        self.level = 0.;
        self.cant_hear = false;
        self.redraw(cx);
    }

    /// The transcript so far: confirmed, then provisional. The core sends them as two runs of
    /// words, so they need a space between them.
    pub fn text(&self) -> String {
        let joined = self.confirmed.is_empty()
            || self.provisional.is_empty()
            || self.confirmed.ends_with(' ')
            || self.provisional.starts_with(' ');
        format!("{}{}{}", self.confirmed, if joined { "" } else { " " }, self.provisional)
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

    /// The text in the pill (the last `max_chars` of [`text`](Self::text), so the newest words
    /// stay in view), and the byte range of the provisional part.
    fn display_text(&self, max_chars: usize) -> (String, std::ops::Range<usize>) {
        match self.mode {
            Mode::Listening | Mode::Live if self.cant_hear => return (CANT_HEAR.into(), 0..0),
            Mode::Listening => return ("Listening…".into(), 0..0),
            Mode::Notice => return (self.confirmed.clone(), 0..0),
            Mode::Live | Mode::Final => {}
        }
        let full = self.text();
        let total = full.chars().count();
        let (prefix, skip) = if total > max_chars { ("…", total - max_chars) } else { ("", 0) };
        let text: String = prefix.chars().chain(full.chars().skip(skip)).collect();
        // The provisional part is the last `n` characters (fewer if the cut reached into it).
        let n = self.provisional.chars().count().min(total - skip);
        let start = match n {
            0 => text.len(),
            n => text.char_indices().rev().nth(n - 1).map_or(0, |(i, _)| i),
        };
        let end = text.len();
        (text, start..end)
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

impl Render for OverlayView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The app's appearance, read live: the window's own copy only updates when AppKit tells
        // it, which a hidden panel may miss.
        let palette = Palette::for_appearance(cx.window_appearance());
        self.drawn_dark = Some(*palette == Palette::DARK);
        self.renders += 1;
        let max_chars = cx.global::<crate::state::AppSettings>().app.overlay_max_chars;
        let (text, provisional) = self.display_text(max_chars as usize);
        // `HighlightStyle::color` blends over the base colour (white over white stays white),
        // so fading needs `fade_out`.
        let faded = HighlightStyle { fade_out: Some(palette.provisional_fade), ..Default::default() };
        let highlights = if provisional.is_empty() { vec![] } else { vec![(provisional, faded)] };

        div().size_full().p(px(4.)).child(
            div()
                .size_full()
                .rounded_full()
                .bg(rgba(palette.background))
                .border_1()
                .border_color(rgba(palette.border))
                .px(px(20.))
                .flex()
                .items_center()
                .gap(px(12.))
                .text_color(rgb(palette.text))
                .text_size(px(16.))
                .child(self.indicator(palette))
                .child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(StyledText::new(text).with_highlights(highlights)),
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
}

impl Overlay {
    pub fn show(&self) {
        self.panel.show();
    }

    pub fn hide(&self) {
        self.panel.hide();
    }
}

/// Creates the overlay window, hidden, at the bottom centre of the primary display.
pub fn create(cx: &mut App) -> anyhow::Result<Overlay> {
    let display = cx.primary_display().ok_or_else(|| anyhow::anyhow!("no display"))?;
    let bounds = display.bounds();
    let origin = point(
        bounds.origin.x + (bounds.size.width - px(WIDTH)) / 2.,
        bounds.origin.y + bounds.size.height - px(HEIGHT) - px(BOTTOM_MARGIN),
    );
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin, size: size(px(WIDTH), px(HEIGHT)) })),
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
        panel = Some(OverlayPanel::attach(window));
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
    })
}

#[cfg(test)]
mod tests {
    // Not `super::*`: that brings in GPUI's own `test` macro, which shadows `#[test]`.
    use super::{BAR_MAX, BAR_MIN, CANT_HEAR, Mode, OverlayView, Palette};

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
            self.display_text(MAX_CHARS)
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
        // Error notes come with their own leading space.
        assert_eq!(view("Hello", "  ⚠ oops").shown().0, "Hello  ⚠ oops");
    }

    #[test]
    fn long_text_keeps_the_newest_words() {
        let confirmed = "é".repeat(100);
        let (text, prov) = view(&confirmed, " naïve").shown();
        assert_eq!(text.chars().count(), MAX_CHARS + 1);
        assert!(text.starts_with('…'));
        assert_eq!(&text[prov], " naïve");
        let (text, _) = view(&confirmed, " naïve").display_text(10);
        assert_eq!(text, "…éééé naïve");
    }

    #[test]
    fn listening_and_notices_show_no_provisional_text() {
        let mut v = view("Allow the microphone", "");
        v.mode = Mode::Notice;
        assert_eq!(v.shown(), ("Allow the microphone".to_string(), 0..0));
        v.mode = Mode::Listening;
        assert_eq!(v.shown().0, "Listening…");
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
