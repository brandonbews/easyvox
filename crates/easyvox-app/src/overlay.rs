//! The live-text overlay: a pill near the bottom of the screen that shows the transcript while
//! you talk. Plain GPUI (no GPUI Kit components) in a `WindowKind::PopUp` window, which GPUI
//! makes a non-activating panel; `platform::OverlayPanel` adds click-through and the key guard
//! so it can never take focus from the app you're dictating into.
//!
//! TODO(feature work): follow the system light/dark appearance (the colours below are
//! hard-coded dark) and add the animations in DESIGN.md "Overlay animation" (open/close, the
//! level indicator, new words easing in).

use crate::platform::OverlayPanel;
use gpui_kit::*;

const WIDTH: f32 = 640.;
const HEIGHT: f32 = 56.;
/// Distance from the bottom of the screen.
const BOTTOM_MARGIN: f32 = 140.;
/// Longer text keeps only its tail, so the newest words stay in view.
const MAX_CHARS: usize = 72;

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
    /// Text that may still change; drawn dimmed.
    pub provisional: String,
    pub mode: Mode,
}

impl OverlayView {
    pub fn set_partial(&mut self, confirmed: String, provisional: String, cx: &mut Context<Self>) {
        self.confirmed = confirmed;
        self.provisional = provisional;
        self.mode = Mode::Live;
        cx.notify();
    }

    pub fn set_final(&mut self, text: String, cx: &mut Context<Self>) {
        self.confirmed = text;
        self.provisional.clear();
        self.mode = Mode::Final;
        cx.notify();
    }

    pub fn set_notice(&mut self, message: String, cx: &mut Context<Self>) {
        self.confirmed = message;
        self.provisional.clear();
        self.mode = Mode::Notice;
        cx.notify();
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.confirmed.clear();
        self.provisional.clear();
        self.mode = Mode::Listening;
        cx.notify();
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

    /// The text in the pill (the tail of [`text`](Self::text)), and the byte range of the
    /// provisional part.
    fn display_text(&self) -> (String, std::ops::Range<usize>) {
        match self.mode {
            Mode::Listening => return ("Listening…".into(), 0..0),
            Mode::Notice => return (self.confirmed.clone(), 0..0),
            Mode::Live | Mode::Final => {}
        }
        let full = self.text();
        let total = full.chars().count();
        let (prefix, skip) = if total > MAX_CHARS { ("…", total - MAX_CHARS) } else { ("", 0) };
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
}

impl Render for OverlayView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let (text, provisional) = self.display_text();
        // `HighlightStyle::color` blends over the base colour (white over white stays white),
        // so dimming needs `fade_out`.
        let dim = HighlightStyle { fade_out: Some(0.55), ..Default::default() };
        let highlights = if provisional.is_empty() { vec![] } else { vec![(provisional, dim)] };
        let dot = match self.mode {
            Mode::Final => rgb(0x34c759),
            Mode::Notice => rgb(0xffcc00),
            Mode::Listening | Mode::Live => rgb(0xff453a),
        };

        div().size_full().p(px(4.)).child(
            div()
                .size_full()
                .rounded_full()
                .bg(rgba(0x1c1c1ef0))
                .border_1()
                .border_color(rgba(0xffffff22))
                .px(px(20.))
                .flex()
                .items_center()
                .gap(px(12.))
                .text_color(rgb(0xffffff))
                .text_size(px(16.))
                .child(div().size(px(10.)).flex_none().rounded_full().bg(dot))
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
    cx.open_window(options, |window, cx| {
        panel = Some(OverlayPanel::attach(window));
        let v = cx.new(|_| OverlayView::default());
        view = Some(v.clone());
        v
    })?;
    Ok(Overlay {
        view: view.ok_or_else(|| anyhow::anyhow!("overlay view not built"))?,
        panel: panel.ok_or_else(|| anyhow::anyhow!("overlay panel not built"))??,
    })
}

#[cfg(test)]
mod tests {
    // Not `super::*`: that brings in GPUI's own `test` macro, which shadows `#[test]`.
    use super::{MAX_CHARS, Mode, OverlayView};

    fn view(confirmed: &str, provisional: &str) -> OverlayView {
        OverlayView { confirmed: confirmed.into(), provisional: provisional.into(), mode: Mode::Live }
    }

    #[test]
    fn short_text_is_shown_whole_with_the_provisional_tail_marked() {
        let (text, prov) = view("Hello,", "world").display_text();
        assert_eq!(text, "Hello, world");
        assert_eq!(&text[prov], "world");
        let (text, prov) = view("Hello", "").display_text();
        assert_eq!(text, "Hello");
        assert!(prov.is_empty());
        let (text, prov) = view("", "Hello").display_text();
        assert_eq!((text.as_str(), &text[prov]), ("Hello", "Hello"));
        // Error notes come with their own leading space.
        assert_eq!(view("Hello", "  ⚠ oops").display_text().0, "Hello  ⚠ oops");
    }

    #[test]
    fn long_text_keeps_the_newest_words() {
        let confirmed = "é".repeat(100);
        let (text, prov) = view(&confirmed, " naïve").display_text();
        assert_eq!(text.chars().count(), MAX_CHARS + 1);
        assert!(text.starts_with('…'));
        assert_eq!(&text[prov], " naïve");
    }

    #[test]
    fn listening_and_notices_show_no_provisional_text() {
        let mut v = view("Allow the microphone", "");
        v.mode = Mode::Notice;
        assert_eq!(v.display_text(), ("Allow the microphone".to_string(), 0..0));
        v.mode = Mode::Listening;
        assert_eq!(v.display_text().0, "Listening…");
    }
}
