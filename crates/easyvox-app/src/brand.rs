//! The EasyVox brand in the app: the color tokens from `design/tokens.json`, the GPUI Kit theme
//! built on them, and the brand's images (`assets/brand/`).
//!
//! - **Colors** ([`Colors`], one per appearance). Vermilion is the only brand color: the wave,
//!   switches, focus rings, the listening state and the primary button. On light, `accent` is for
//!   graphics only (3.7:1 on white); text in vermilion uses `accent_text` (4.6:1).
//! - **The theme.** GPUI Kit's own light and dark themes with the brand's keys on top
//!   ([`install_theme`]): the accent, and the design's paper and ink for the windows. Most of its
//!   neutral grays stay. Every `Theme::sync_system_appearance` call then picks the branded pair,
//!   so no window has to know.
//! - **Window parts** the three windows share: grouped forms ([`group`], [`row`]), section titles
//!   and footnotes, keycaps, pill buttons and thin glyphs.
//! - **Images**, served by [`Assets`] next to GPUI Kit's icons: the app icon at 128 and 256 px,
//!   the mark and the knot as SVG. GPUI's `svg()` paints an SVG as a one-color mask (in the text
//!   color), so `KNOT` and `MARK` are drawn that way; the two-tone mark is an `img()`, one file per
//!   appearance. The menu bar's glyphs live in `assets/brand/menubar/` and go to the status item
//!   through `tray.rs`, not through GPUI.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, Theme, ThemeConfig, ThemeRegistry, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AssetSource, Div, ElementId, FontWeight, IntoElement, ParentElement as _, Pixels, SharedString,
    Styled as _, Svg, WindowAppearance, div, px, rgb, svg,
};
use std::borrow::Cow;
use std::rc::Rc;

/// The brand's colors in one appearance, as `0xRRGGBB`, or `0xRRGGBBAA` where what's behind
/// shows through. Names follow `design/tokens.json`. A whole palette: not every window reads every
/// token.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colors {
    /// Text, and the knot of the mark.
    pub ink: u32,
    /// Window backgrounds.
    pub paper: u32,
    /// Secondary text: captions, hints (5:1 or better on paper).
    pub ink2: u32,
    /// Separators and hairlines.
    pub sep: u32,
    /// Grouped surfaces on paper.
    pub surface: u32,
    /// Vermilion: the wave, switches, focus rings, the listening state. Graphics only on light.
    pub accent: u32,
    /// Vermilion for links and small accents in text (4.6:1 light, 5.5:1 dark).
    pub accent_text: u32,
    /// Text or a glyph drawn on `accent`.
    pub on_accent: u32,
    /// Primary buttons, the same in both appearances, with `button_text` (4.59:1).
    pub button_fill: u32,
    pub button_text: u32,
    /// Destructive words (an error, Delete).
    pub destructive_text: u32,
    /// The overlay pill: a flat 92 % fill (`0xRRGGBBAA`), its text, and its secondary text.
    pub pill_fill: u32,
    pub pill_text: u32,
    pub pill_text2: u32,
    /// The windows' grouped forms: a group's fill and outline, and the hairline between its rows.
    /// On light the window is paper too, so a group is paper with an outline, which keeps
    /// secondary text and links on paper (vermilion text is 4.2:1 on `surface`). On dark a group
    /// is `surface`, and needs no outline.
    pub group: u32,
    pub group_border: u32,
    pub hairline: u32,
    /// Settings' inset sidebar: a tinted panel (the design's glass, flat) and its selected row.
    pub sidebar: u32,
    pub sidebar_selected: u32,
    /// A keycap ("⌥Space"): its fill and outline.
    pub keycap: u32,
    pub keycap_border: u32,
}

pub const LIGHT: Colors = Colors {
    ink: 0x1d1d1f,
    paper: 0xffffff,
    ink2: 0x6e6e73,
    sep: 0xd2d2d7,
    surface: 0xf5f5f7,
    accent: 0xf0462d,
    accent_text: 0xd93a22,
    on_accent: 0xffffff,
    button_fill: 0xd93a22,
    button_text: 0xffffff,
    destructive_text: 0xd93a22,
    // 0xeb: 235 / 255, the design's 92 %.
    pill_fill: 0xffffffeb,
    pill_text: 0x1d1d1f,
    pill_text2: 0x636366,
    group: 0xffffff,
    // The design's rgba(0,0,0,0.1) and rgba(0,0,0,0.09) hairlines, on paper.
    group_border: 0xe3e3e8,
    hairline: 0xe8e8ed,
    // The design's rgba(232,232,236,0.92) panel on paper, and its rgba(0,0,0,0.07) selection.
    sidebar: 0xeaeaee,
    sidebar_selected: 0xdadade,
    keycap: 0xefeff2,
    keycap_border: 0xd2d2d7,
};

pub const DARK: Colors = Colors {
    ink: 0xf5f5f7,
    paper: 0x1c1c1e,
    ink2: 0x98989d,
    sep: 0x38383a,
    surface: 0x2c2c2e,
    accent: 0xff5c3f,
    accent_text: 0xff5c3f,
    on_accent: 0x1c1c1e,
    button_fill: 0xd93a22,
    button_text: 0xffffff,
    destructive_text: 0xff8470,
    pill_fill: 0x1c1c1eeb,
    pill_text: 0xf5f5f7,
    pill_text2: 0x98989d,
    group: 0x2c2c2e,
    group_border: 0x2c2c2e,
    // The design's rgba(255,255,255,0.09), on a group.
    hairline: 0x3d3d40,
    // The design's rgba(44,44,48,0.92) panel on paper, and its rgba(255,255,255,0.08) selection.
    sidebar: 0x2b2b2f,
    sidebar_selected: 0x3c3c40,
    keycap: 0x3f3f42,
    keycap_border: 0x545458,
};

/// Dark only: GPUI Kit keys whose stock values (near-black, made for its `#0a0a0a` background)
/// vanish on paper `#1C1C1E` or on a group: an outline button's edge, a menu's or tooltip's
/// surface, a hovered menu row, and a switch's track when off. Light keeps GPUI Kit's.
const DARK_INPUT: u32 = 0x545458;
const DARK_POPOVER: u32 = 0x2c2c2e;
const DARK_HOVER: u32 = 0x3a3a3c;
const DARK_SWITCH_OFF: u32 = 0x5a5a5e;

/// The primary button when pointed at and when pressed: the fill, a little darker each time, so
/// the white label only gains contrast.
const BUTTON_HOVER: u32 = 0xc6341e;
const BUTTON_PRESSED: u32 = 0xb02e1a;

/// The brand's colors for `appearance`.
#[allow(dead_code)]
pub fn colors(appearance: WindowAppearance) -> &'static Colors {
    if is_dark(appearance) { &DARK } else { &LIGHT }
}

/// The brand's colors for the theme in use: what a window draws with, in step with GPUI Kit's
/// controls.
pub fn current(cx: &App) -> &'static Colors {
    if cx.theme().is_dark() { &DARK } else { &LIGHT }
}

pub fn is_dark(appearance: WindowAppearance) -> bool {
    matches!(appearance, WindowAppearance::Dark | WindowAppearance::VibrantDark)
}

/// The names of the branded themes, as `Theme::theme_name` reports them.
pub const THEME_LIGHT: &str = "EasyVox Light";
pub const THEME_DARK: &str = "EasyVox Dark";

/// Makes the branded light and dark themes the ones GPUI Kit switches between, and applies the
/// one for the system's appearance. Call it once, after `gpui_kit::init`. Each is GPUI Kit's own
/// theme of that appearance with the brand's keys on top ([`branded`]), so everything the brand
/// doesn't name keeps GPUI Kit's look.
pub fn install_theme(cx: &mut App) {
    let registry = ThemeRegistry::global(cx);
    let light = branded(registry.default_light_theme(), THEME_LIGHT, &LIGHT);
    let dark = branded(registry.default_dark_theme(), THEME_DARK, &DARK);
    let theme = Theme::global_mut(cx);
    theme.light_theme = Rc::new(light);
    theme.dark_theme = Rc::new(dark);
    // Loads the pair's theme for the appearance now, even if the mode didn't change.
    Theme::sync_system_appearance(None, cx);
}

/// `stock` with the brand's keys:
/// - **primary** (switches, checkboxes, progress bars): the accent, with `on_accent` on it;
/// - **primary buttons**: `button_fill` with `button_text`, in both appearances;
/// - **focus rings**: the accent; **links**: `accent_text`;
/// - **danger** (error text, a destructive button): `destructive_text`;
/// - **the windows**: the background is `paper` (`#FFFFFF`, `#1C1C1E`) in place of GPUI Kit's
///   `#0a0a0a` in dark; text and secondary text are `ink` and `ink2`, the grays the design pairs
///   with paper (4.5:1 or better on paper and on a group, see the tests); borders are the
///   hairline; a switch's thumb is white, as on macOS;
/// - **dark only**, the few keys that vanish on the lighter background ([`DARK_INPUT`] and on).
///
/// Nothing else changes: GPUI Kit's other grays, its text selection and the menus' highlight
/// (the system's) stay.
fn branded(stock: &ThemeConfig, name: &str, c: &Colors) -> ThemeConfig {
    let hex = |rgb: u32| Some(SharedString::from(format!("#{rgb:06x}")));
    let mut theme = stock.clone();
    theme.name = name.to_string().into();
    theme.is_default = false;
    let k = &mut theme.colors;
    k.primary = hex(c.accent);
    k.primary_hover = hex(c.accent);
    k.primary_active = hex(c.accent_text);
    k.primary_foreground = hex(c.on_accent);
    k.button_primary = hex(c.button_fill);
    k.button_primary_hover = hex(BUTTON_HOVER);
    k.button_primary_active = hex(BUTTON_PRESSED);
    k.button_primary_foreground = hex(c.button_text);
    k.ring = hex(c.accent);
    k.link = hex(c.accent_text);
    k.link_hover = hex(c.accent_text);
    k.link_active = hex(c.accent_text);
    k.progress_bar = hex(c.accent);
    k.danger = hex(c.destructive_text);
    // White on light's #d93a22 (4.59:1), near-black on dark's #ff8470 (7.1:1).
    k.danger_foreground = hex(c.on_accent);
    k.background = hex(c.paper);
    k.foreground = hex(c.ink);
    k.muted_foreground = hex(c.ink2);
    k.border = hex(c.hairline);
    k.switch_thumb = hex(0xffffff);
    if *c == DARK {
        k.input = hex(DARK_INPUT);
        k.popover = hex(DARK_POPOVER);
        k.accent = hex(DARK_HOVER);
        k.switch = hex(DARK_SWITCH_OFF);
    }
    theme
}

// ───────────────────────── Window parts ─────────────────────────
//
// Sizes in points, from `design/tokens.json` (`type.app`) where it has them.

/// Body text in the windows.
pub const TEXT: Pixels = px(13.);
/// A row's second line, a footnote.
pub const TEXT_SMALL: Pixels = px(11.);
/// A keycap's label, a time.
pub const TEXT_KEY: Pixels = px(12.);
/// A page's title.
pub const TEXT_TITLE: Pixels = px(15.);
/// The setup window's title, and the wordmark on About.
pub const TEXT_HERO: Pixels = px(22.);

/// Rows on a card, with a hairline between each two: the windows' grouped forms. Rows come from
/// [`row`] or anything as wide.
pub fn group(c: &Colors, rows: impl IntoIterator<Item = AnyElement>) -> Div {
    let mut card =
        v_flex().w_full().px_3().rounded(px(10.)).bg(rgb(c.group)).border_1().border_color(rgb(c.group_border));
    for (i, row) in rows.into_iter().enumerate() {
        if i > 0 {
            card = card.child(div().h(px(1.)).w_full().bg(rgb(c.hairline)));
        }
        card = card.child(row);
    }
    card
}

/// A row of a group: its title, a second line under it if it has one, and its control on the
/// right (40 pt tall at least).
pub fn row(title: impl Into<SharedString>, line: Option<AnyElement>, control: Option<AnyElement>) -> Div {
    let left = v_flex()
        .child(div().text_size(TEXT).line_height(px(18.)).child(title.into()))
        .when_some(line, |this, line| this.child(div().mt(px(2.)).child(line)));
    row_with(left.into_any_element(), control)
}

/// A row whose left side is drawn by the caller.
pub fn row_with(left: AnyElement, control: Option<AnyElement>) -> Div {
    h_flex()
        .w_full()
        .min_h(px(40.))
        .py(px(7.))
        .gap_3()
        .items_center()
        .child(div().flex_1().min_w_0().child(left))
        .when_some(control, |this, control| this.child(div().flex_shrink_0().child(control)))
}

/// A row's second line: secondary text, 11 pt.
pub fn line(c: &Colors, text: impl Into<SharedString>) -> AnyElement {
    div().text_size(TEXT_SMALL).line_height(px(15.)).text_color(rgb(c.ink2)).child(text.into()).into_any_element()
}

/// A line that reports a problem: a thin warning glyph and the words, in the text color, never
/// in color alone (the brand's rule for errors).
pub fn problem(c: &Colors, text: impl Into<SharedString>, size: Pixels) -> Div {
    h_flex().items_start().gap_1p5().child(glyph(GLYPH_WARNING, size + px(2.), c.ink).mt(px(1.))).child(
        div().flex_1().min_w_0().text_size(size).line_height(size + px(4.)).text_color(rgb(c.ink)).child(text.into()),
    )
}

/// A title above a group, 13 pt semibold.
pub fn section(text: impl Into<SharedString>) -> Div {
    div().mt(px(20.)).mb(px(8.)).ml(px(12.)).text_size(TEXT).font_weight(FontWeight::SEMIBOLD).child(text.into())
}

/// Words under a group: secondary text, 11 pt.
pub fn footnote(c: &Colors, text: impl Into<SharedString>) -> Div {
    div().mt(px(6.)).mx(px(12.)).text_size(TEXT_SMALL).line_height(px(15.)).text_color(rgb(c.ink2)).child(text.into())
}

/// A keycap: "⌥Space", 12 pt medium on a small key.
pub fn keycap(c: &Colors, text: impl Into<SharedString>) -> Div {
    h_flex()
        .flex_shrink_0()
        .h(px(20.))
        .px(px(7.))
        .rounded(px(5.))
        .bg(rgb(c.keycap))
        .border_1()
        .border_color(rgb(c.keycap_border))
        .text_size(TEXT_KEY)
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(c.ink))
        .child(text.into())
}

/// A pill button, 24 pt tall: the design's push button. Outlined; [`primary`] makes it vermilion.
pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).small().outline().rounded(px(12.)).px_3().label(label)
}

/// The vermilion pill button: the one main action on a screen.
pub fn primary(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).small().primary().rounded(px(12.)).px_3().label(label)
}

/// A thin one-color glyph (an SVG in `assets/brand/`), `size` square.
pub fn glyph(path: &'static str, size: Pixels, color: u32) -> Svg {
    svg().path(path).flex_none().size(size).text_color(rgb(color))
}

// ───────────────────────── Images ─────────────────────────

/// The app icon (`design/assets/app-icon`, Default appearance), for a header or About.
pub const ICON_128: &str = "brand/icon-128.png";
pub const ICON_256: &str = "brand/icon-256.png";
/// The full mark in two colors, for a light and a dark background (`img()`).
#[allow(dead_code)]
pub const MARK_ON_LIGHT: &str = "brand/mark-on-light.svg";
#[allow(dead_code)]
pub const MARK_ON_DARK: &str = "brand/mark-on-dark.svg";
/// The full mark and the knot alone (the compact mark, for 14–24 pt) in one color: `svg()`
/// paints them in the text color.
#[allow(dead_code)]
pub const MARK: &str = "brand/mark.svg";
pub const KNOT: &str = "brand/knot.svg";

/// The overlay pill's glyphs: 20 pt, a 1.5 pt stroke with round caps, in one color (`svg()`
/// paints them in the text color).
pub const GLYPH_WARNING: &str = "brand/pill/warning.svg";
pub const GLYPH_NO_MICROPHONE: &str = "brand/pill/no-microphone.svg";
pub const GLYPH_INFO: &str = "brand/pill/info.svg";
pub const GLYPH_PASTED: &str = "brand/pill/pasted.svg";
pub const GLYPH_COPIED: &str = "brand/pill/copied.svg";

/// The Settings sidebar's page icons, and the thin "allowed" glyph: 16 pt, a 1.4 pt stroke.
/// General and About are the design's; Models (a wave) and Permissions (a shield) are drawn the
/// same way, for pages the design doesn't have.
pub const ICON_GENERAL: &str = "brand/settings/general.svg";
pub const ICON_MODELS: &str = "brand/settings/models.svg";
pub const ICON_PERMISSIONS: &str = "brand/settings/permissions.svg";
pub const ICON_ABOUT: &str = "brand/settings/about.svg";
pub const GLYPH_ALLOWED: &str = "brand/settings/allowed.svg";

/// Every brand image GPUI can load, by its asset path.
const FILES: &[(&str, &[u8])] = &[
    (ICON_128, include_bytes!("../assets/brand/icon-128.png")),
    (ICON_256, include_bytes!("../assets/brand/icon-256.png")),
    (MARK_ON_LIGHT, include_bytes!("../assets/brand/mark-on-light.svg")),
    (MARK_ON_DARK, include_bytes!("../assets/brand/mark-on-dark.svg")),
    (MARK, include_bytes!("../assets/brand/mark.svg")),
    (KNOT, include_bytes!("../assets/brand/knot.svg")),
    (GLYPH_WARNING, include_bytes!("../assets/brand/pill/warning.svg")),
    (GLYPH_NO_MICROPHONE, include_bytes!("../assets/brand/pill/no-microphone.svg")),
    (GLYPH_INFO, include_bytes!("../assets/brand/pill/info.svg")),
    (GLYPH_PASTED, include_bytes!("../assets/brand/pill/pasted.svg")),
    (GLYPH_COPIED, include_bytes!("../assets/brand/pill/copied.svg")),
    (ICON_GENERAL, include_bytes!("../assets/brand/settings/general.svg")),
    (ICON_MODELS, include_bytes!("../assets/brand/settings/models.svg")),
    (ICON_PERMISSIONS, include_bytes!("../assets/brand/settings/permissions.svg")),
    (ICON_ABOUT, include_bytes!("../assets/brand/settings/about.svg")),
    (GLYPH_ALLOWED, include_bytes!("../assets/brand/settings/allowed.svg")),
];

/// The app's asset source: the brand's images (`brand/…`), then GPUI Kit's icons.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        match FILES.iter().find(|(p, _)| *p == path) {
            Some((_, bytes)) => Ok(Some(Cow::Borrowed(bytes))),
            None => gpui_kit::assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        let mut all = gpui_kit::assets::Assets.list(path)?;
        all.extend(FILES.iter().filter(|(p, _)| p.starts_with(path)).map(|(p, _)| SharedString::from(*p)));
        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use super::{Assets, BUTTON_HOVER, BUTTON_PRESSED, DARK, FILES, LIGHT};
    use gpui_kit::AssetSource as _;

    /// WCAG 2 contrast ratio of two opaque `0xRRGGBB` colors.
    fn contrast(a: u32, b: u32) -> f64 {
        let luminance = |rgb: u32| {
            let channel = |shift: u32| {
                let c = f64::from((rgb >> shift) & 0xff) / 255.;
                if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
            };
            0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
        };
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    /// The windows on the design's paper, in both appearances: text and secondary text meet AA
    /// (4.5:1) on paper and on a group, and so do links; switches, focus rings and a primary
    /// button's edge, graphics, meet 3:1.
    #[test]
    fn window_text_and_controls_are_legible_on_paper_and_on_groups() {
        for c in [LIGHT, DARK] {
            for (name, bg) in [("paper", c.paper), ("group", c.group)] {
                assert!(contrast(c.ink, bg) >= 7., "text on {name}");
                assert!(contrast(c.ink2, bg) >= 4.5, "secondary text on {name}: {:.2}", contrast(c.ink2, bg));
                assert!(contrast(c.accent_text, bg) >= 4.5, "links on {name}: {:.2}", contrast(c.accent_text, bg));
                assert!(contrast(c.accent, bg) >= 3., "a switch or focus ring on {name}");
                assert!(contrast(c.button_fill, bg) >= 3., "a primary button on {name}");
            }
            for (name, bg) in [("sidebar", c.sidebar), ("selected row", c.sidebar_selected), ("keycap", c.keycap)] {
                assert!(contrast(c.ink, bg) >= 7., "text on the {name}");
            }
            assert!(contrast(c.ink2, c.sidebar) >= 3., "the sidebar's icons");
            assert!(contrast(c.button_text, c.button_fill) >= 4.5, "a primary button's label");
        }
        for fill in [BUTTON_HOVER, BUTTON_PRESSED] {
            assert!(contrast(LIGHT.button_text, fill) >= 4.5);
        }
        assert_eq!((LIGHT.paper, DARK.paper), (0xffffff, 0x1c1c1e), "the design's paper");
    }

    #[test]
    fn every_brand_image_loads_and_gpui_kit_icons_still_do() {
        for (path, bytes) in FILES {
            assert!(!bytes.is_empty(), "{path}");
            assert_eq!(Assets.load(path).unwrap().as_deref(), Some(*bytes), "{path}");
        }
        assert!(Assets.load("icons/check.svg").unwrap().is_some(), "GPUI Kit's icons");
        assert_eq!(Assets.list("brand/").unwrap().len(), FILES.len());
    }

    #[test]
    fn primary_buttons_are_the_same_vermilion_in_both_appearances() {
        assert_eq!((LIGHT.button_fill, LIGHT.button_text), (DARK.button_fill, DARK.button_text));
        // In dark, the accent is light enough for text, so it's the text color too.
        assert_eq!(DARK.accent, DARK.accent_text);
    }
}
