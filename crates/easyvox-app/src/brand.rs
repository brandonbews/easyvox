//! The EasyVox brand in the app: the colour tokens from the design hand-off
//! (`design/tokens.json`), the GPUI Kit theme built on them, and the brand's images (`assets/brand/`).
//! `DESIGN.md`, "Brand", says where each one shows.
//!
//! - **Colours** ([`Colors`], one per appearance). Vermilion is the only brand colour: the wave,
//!   switches, focus rings, the listening state and the primary button. On light, `accent` is for
//!   graphics only (3.7:1 on white); text in vermilion uses `accent_text` (4.6:1).
//! - **The theme.** GPUI Kit's own light and dark themes with the brand's accent keys on top
//!   ([`install_theme`]). Its neutral greys stay. Every `Theme::sync_system_appearance` call then
//!   picks the branded pair, so no window has to know.
//! - **Images**, served by [`Assets`] next to GPUI Kit's icons: the app icon at 128 and 256 px,
//!   the mark and the knot as SVG. GPUI's `svg()` paints an SVG as a one-colour mask (in the text
//!   colour), so `KNOT` and `MARK` are drawn that way; the two-tone mark is an `img()`, one file per
//!   appearance. The menu bar's glyphs live in `assets/brand/menubar/` and go to the status item
//!   through `tray.rs`, not through GPUI.

use gpui_kit::component::{Theme, ThemeConfig, ThemeRegistry};
use gpui_kit::{App, AssetSource, SharedString, WindowAppearance};
use std::borrow::Cow;
use std::rc::Rc;

/// The brand's colours in one appearance, as `0xRRGGBB`, or `0xRRGGBBAA` where what's behind
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
};

/// The primary button when pointed at and when pressed: the fill, a little darker each time, so
/// the white label only gains contrast.
const BUTTON_HOVER: u32 = 0xc6341e;
const BUTTON_PRESSED: u32 = 0xb02e1a;

/// The brand's colours for `appearance`.
#[allow(dead_code)]
pub fn colors(appearance: WindowAppearance) -> &'static Colors {
    if is_dark(appearance) { &DARK } else { &LIGHT }
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
/// - **danger** (error text, a destructive button): `destructive_text`.
///
/// Nothing else changes: GPUI Kit's neutral greys, its text selection and the menus' highlight
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
    theme
}

// ───────────────────────── Images ─────────────────────────

/// The app icon (`design/assets/app-icon`, Default appearance), for a header or About.
#[allow(dead_code)]
pub const ICON_128: &str = "brand/icon-128.png";
#[allow(dead_code)]
pub const ICON_256: &str = "brand/icon-256.png";
/// The full mark in two colours, for a light and a dark background (`img()`).
#[allow(dead_code)]
pub const MARK_ON_LIGHT: &str = "brand/mark-on-light.svg";
#[allow(dead_code)]
pub const MARK_ON_DARK: &str = "brand/mark-on-dark.svg";
/// The full mark and the knot alone (the compact mark, for 14–24 pt) in one colour: `svg()`
/// paints them in the text colour.
#[allow(dead_code)]
pub const MARK: &str = "brand/mark.svg";
#[allow(dead_code)]
pub const KNOT: &str = "brand/knot.svg";

/// The overlay pill's glyphs (`design/`, board 08): 20 pt, a 1.5 pt stroke with round caps, in
/// one colour (`svg()` paints them in the text colour).
pub const GLYPH_WARNING: &str = "brand/pill/warning.svg";
pub const GLYPH_NO_MICROPHONE: &str = "brand/pill/no-microphone.svg";
pub const GLYPH_INFO: &str = "brand/pill/info.svg";
pub const GLYPH_PASTED: &str = "brand/pill/pasted.svg";
pub const GLYPH_COPIED: &str = "brand/pill/copied.svg";

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
    use super::{Assets, DARK, FILES, LIGHT};
    use gpui_kit::AssetSource as _;

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
        // In dark, the accent is light enough for text, so it's the text colour too.
        assert_eq!(DARK.accent, DARK.accent_text);
    }
}
