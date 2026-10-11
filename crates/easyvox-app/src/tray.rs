//! The menu bar item and its menu, through the `tray-icon` crate (Tauri team, used without
//! Tauri). Menu callbacks run on the main thread from AppKit; `main.rs` forwards them into GPUI
//! through the app's event channel.
//!
//! The item says what EasyVox is doing (`DESIGN.md`, "Menu bar"): one 30 × 16 pt image per state
//! ([`Glyph`]), so it never changes width, and a status at the top of the menu that names a
//! problem and its fix. The controller works out the [`Status`] on events (a press, a release,
//! the final text, a click on the item, a change in the app's state) and hands it to
//! [`Tray::show`], which changes only what differs. Nothing here runs on a timer.

use crate::platform;
use crate::settings::Page;
use tray_icon::menu::accelerator::{Accelerator, Code, Modifiers};
use tray_icon::menu::{IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

/// Debug builds only: replays a test clip as if it were spoken.
#[cfg(debug_assertions)]
pub const MENU_TEST_DICTATION: &str = "test-dictation";
pub const MENU_SETTINGS: &str = "settings";
pub const MENU_HISTORY: &str = "history";
pub const MENU_ABOUT: &str = "about";
pub const MENU_QUIT: &str = "quit";
/// The rows of a problem in the status at the top: `fix-0` for the first problem, and so on.
pub const MENU_FIX: &str = "fix-";
pub const SETTINGS_TITLE: &str = "Settings…";
pub const HISTORY_TITLE: &str = "History…";
/// Opens Settings on its About page: the version, the models' credits and the third-party
/// notices.
pub const ABOUT_TITLE: &str = "About EasyVox";
pub const QUIT_TITLE: &str = "Quit EasyVox";

/// The item's image: what EasyVox is doing, or that it needs something.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Glyph {
    /// The knot alone: hold the shortcut to talk.
    #[default]
    Ready,
    /// The knot and three vermilion bars, still (not a live meter): the shortcut is held.
    Listening,
    /// Three dots: released, the final text is on its way.
    Finishing,
    /// A check, for `menubar_pasted_ms` after the text is pasted.
    Pasted,
    /// A ring: the speech model is loading or downloading.
    GettingReady,
    /// An exclamation mark: a problem in the menu says what to do.
    Attention,
}

/// A 30 × 16 pt image at 1× and 2×.
struct Image {
    x1: &'static [u8],
    x2: &'static [u8],
    /// Black on transparent, which macOS inks for the menu bar. Only Listening isn't: its
    /// bars stay vermilion.
    template: bool,
}

/// The 1× and 2× PNGs of `assets/brand/menubar/<name>`, rendered from the design hand-off's
/// SVGs (`design/assets/menubar/`), which sit next to them.
macro_rules! image {
    ($name:literal, $template:expr) => {
        Image {
            x1: include_bytes!(concat!("../assets/brand/menubar/", $name, ".png")),
            x2: include_bytes!(concat!("../assets/brand/menubar/", $name, "@2x.png")),
            template: $template,
        }
    };
}

/// Every image's size, in points.
pub const IMAGE_SIZE: (f64, f64) = (30., 16.);

impl Glyph {
    /// What VoiceOver reads for the item.
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "EasyVox, ready",
            Self::Listening => "EasyVox, listening",
            Self::Finishing => "EasyVox, finishing",
            Self::Pasted => "EasyVox, pasted",
            Self::GettingReady => "EasyVox, getting ready",
            Self::Attention => "EasyVox, needs attention",
        }
    }

    /// The image, for a dark or a light menu bar (only Listening's differs).
    fn image(self, dark_menu_bar: bool) -> Image {
        match self {
            Self::Ready => image!("ready", true),
            Self::Listening if dark_menu_bar => image!("listening-dark", false),
            Self::Listening => image!("listening-light", false),
            Self::Finishing => image!("finishing", true),
            Self::Pasted => image!("pasted", true),
            Self::GettingReady => image!("getting-ready", true),
            Self::Attention => image!("needs-attention", true),
        }
    }
}

/// What a problem's rows do when chosen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fix {
    /// Opens System Settings at this deep link (`platform::PRIVACY_*`).
    SystemSettings(&'static str),
    /// Opens the setup window.
    Setup,
    /// Opens Settings on this page.
    Settings(Page),
}

/// Something EasyVox needs before it can work fully, as the menu's status shows it: a title, a
/// line on what it means, and the fix.
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub title: String,
    pub detail: String,
    /// The fix's row, e.g. "Open System Settings…".
    pub action: &'static str,
    pub fix: Fix,
}

/// What the item shows: its image, and the status at the top of its menu. With no problem, the
/// status is a headline ("Ready") and a line under it (how to dictate); with problems, it's
/// each problem and its fix.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    pub glyph: Glyph,
    pub headline: String,
    pub detail: String,
    pub problems: Vec<Problem>,
}

/// What a dictation is doing, for the item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Activity {
    #[default]
    Idle,
    /// The shortcut is held.
    Recording,
    /// Released; the final text is on its way.
    Finishing,
}

/// What the item's status is worked out from: the controller reads these on events.
#[derive(Clone, Debug)]
pub struct Facts<'a> {
    pub activity: Activity,
    /// The text was pasted moments ago (`menubar_pasted_ms`).
    pub just_pasted: bool,
    pub mic: easyvox_core::MicPermission,
    pub accessibility: bool,
    pub engine: &'a crate::state::EngineStatus,
    /// The chosen model is downloading.
    pub downloading: bool,
    /// The shortcut as the keys read (`⌥Space`), whether right ⌥ dictates too, and whether
    /// the shortcut couldn't be registered.
    pub shortcut: String,
    pub hold_right_option: bool,
    pub shortcut_failed: bool,
}

impl Status {
    /// The item's status. Problems come first, in the order they stop a dictation: the
    /// microphone, the paste, the model, the shortcut. The image shows what a dictation is
    /// doing while one runs, and a problem only between dictations.
    pub fn of(facts: &Facts) -> Self {
        use crate::state::EngineStatus;
        use easyvox_core::MicPermission;
        let mut problems = vec![];
        let problem = |title: &str, detail: &str, action, fix| Problem {
            title: title.into(),
            detail: detail.into(),
            action,
            fix,
        };
        match facts.mic {
            MicPermission::NotDetermined => problems.push(problem(
                "Microphone access needed",
                "EasyVox can't hear you until you allow it.",
                "Set Up EasyVox…",
                Fix::Setup,
            )),
            MicPermission::Denied | MicPermission::Restricted => problems.push(problem(
                "Microphone access is off",
                "EasyVox can't hear you until it's on.",
                "Open System Settings…",
                Fix::SystemSettings(platform::PRIVACY_MICROPHONE),
            )),
            MicPermission::Authorized | MicPermission::Unknown => {}
        }
        if !facts.accessibility {
            problems.push(problem(
                "Accessibility access is off",
                "EasyVox hears you but can't paste into other apps.",
                "Open System Settings…",
                Fix::SystemSettings(platform::PRIVACY_ACCESSIBILITY),
            ));
        }
        match facts.engine {
            EngineStatus::Missing if !facts.downloading => problems.push(problem(
                "No speech model yet",
                "Download one in Settings, under Models.",
                "Open Settings…",
                Fix::Settings(Page::Models),
            )),
            EngineStatus::Failed { .. } => problems.push(problem(
                "The speech model didn't load",
                "Try again, or pick another, in Settings under Models.",
                "Open Settings…",
                Fix::Settings(Page::Models),
            )),
            _ => {}
        }
        if facts.shortcut_failed {
            problems.push(Problem {
                title: format!("{} isn't working", facts.shortcut),
                detail: "Another app may use it. Pick another shortcut in Settings.".into(),
                action: "Open Settings…",
                fix: Fix::Settings(Page::General),
            });
        }
        let getting_ready =
            facts.downloading || matches!(facts.engine, EngineStatus::Loading | EngineStatus::NotLoaded);
        let glyph = match facts.activity {
            Activity::Recording => Glyph::Listening,
            Activity::Finishing => Glyph::Finishing,
            Activity::Idle if facts.just_pasted => Glyph::Pasted,
            Activity::Idle if !problems.is_empty() => Glyph::Attention,
            Activity::Idle if getting_ready => Glyph::GettingReady,
            Activity::Idle => Glyph::Ready,
        };
        let headline = match glyph {
            Glyph::Ready | Glyph::Attention => "Ready",
            Glyph::Listening => "Listening…",
            Glyph::Finishing => "Finishing…",
            Glyph::Pasted => "Pasted",
            Glyph::GettingReady => "Getting ready…",
        };
        let detail = match glyph {
            Glyph::GettingReady if facts.downloading => "Downloading the speech model.".into(),
            Glyph::GettingReady => "Loading the speech model on this Mac.".into(),
            _ if facts.hold_right_option => format!("Hold {} or right ⌥ to talk, let go to paste.", facts.shortcut),
            _ => format!("Hold {} to talk, let go to paste.", facts.shortcut),
        };
        Self { glyph, headline: headline.into(), detail, problems }
    }
}

/// Dropping it removes the menu bar item.
pub struct Tray {
    /// Read only by the self-test.
    #[cfg_attr(not(feature = "self-test"), allow(dead_code))]
    pub icon: TrayIcon,
    #[cfg_attr(not(feature = "self-test"), allow(dead_code))]
    pub menu: Menu,
    /// What's shown now, and on which menu bar (Listening's image depends on it).
    shown: Status,
    dark_menu_bar: Option<bool>,
    /// How many items the status takes at the top of the menu, its separator included.
    status_items: usize,
}

/// `on_menu(id)` runs for a chosen menu item, `on_open()` when the item is clicked, just as its
/// menu opens (the status may be due a check: a permission granted meanwhile, say).
pub fn create(
    on_menu: impl Fn(String) + Send + Sync + 'static,
    on_open: impl Fn() + Send + Sync + 'static,
) -> anyhow::Result<Tray> {
    let menu = Menu::new();
    #[cfg(debug_assertions)]
    menu.append_items(&[
        &MenuItem::with_id(MENU_TEST_DICTATION, "Test dictation (WAV replay)", true, None),
        &PredefinedMenuItem::separator(),
    ])?;
    let command = |key| Some(Accelerator::new(Modifiers::META, key));
    menu.append_items(&[
        &MenuItem::with_id(MENU_HISTORY, HISTORY_TITLE, true, None),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id(MENU_SETTINGS, SETTINGS_TITLE, true, command(Code::Comma)),
        &MenuItem::with_id(MENU_ABOUT, ABOUT_TITLE, true, None),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id(MENU_QUIT, QUIT_TITLE, true, command(Code::KeyQ)),
    ])?;
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| on_menu(e.id.0)));
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Down, .. } = e {
            on_open();
        }
    }));

    // No icon here: tray-icon would draw it at its pixel size (see `Tray::set_image`).
    let icon = TrayIconBuilder::new().with_menu(Box::new(menu.clone())).with_tooltip("EasyVox").build()?;
    let mut tray = Tray { icon, menu, shown: Status::default(), dark_menu_bar: None, status_items: 0 };
    let dark = platform::menu_bar_is_dark(&tray.icon);
    tray.set_image(Glyph::Ready, dark);
    Ok(tray)
}

impl Tray {
    /// Shows `status`: the image if the glyph changed (or, for Listening, the menu bar's
    /// appearance did), the status rows if they changed.
    pub fn show(&mut self, status: Status) {
        let dark = platform::menu_bar_is_dark(&self.icon);
        let bar_changed = Some(dark) != self.dark_menu_bar;
        if status.glyph != self.shown.glyph || (status.glyph == Glyph::Listening && bar_changed) {
            self.set_image(status.glyph, dark);
        }
        let rows_changed = (&status.headline, &status.detail, &status.problems)
            != (&self.shown.headline, &self.shown.detail, &self.shown.problems)
            || self.status_items == 0;
        if rows_changed && let Err(e) = self.set_status_rows(&status) {
            log::error!("menu bar: couldn't update the menu's status: {e}");
        }
        self.shown = status;
    }

    /// What the item shows now.
    #[cfg(feature = "self-test")]
    pub fn status(&self) -> &Status {
        &self.shown
    }

    /// Sets the glyph's image on the status item, through `platform::set_menu_bar_image`
    /// rather than tray-icon: tray-icon sizes an image by its pixels, capped at 22 pt, so the
    /// 2× image (60 × 32 px) would come out 41 × 22 pt.
    fn set_image(&mut self, glyph: Glyph, dark_menu_bar: bool) {
        let image = glyph.image(dark_menu_bar);
        if let Err(e) =
            platform::set_menu_bar_image(&self.icon, IMAGE_SIZE, image.x1, image.x2, image.template, glyph.label())
        {
            log::error!("menu bar: couldn't set the {glyph:?} image: {e}");
            return;
        }
        if self.dark_menu_bar.is_none() {
            // tray-icon sizes the view that takes the item's clicks to the button only when it
            // sets an image, a title or a tooltip itself. Every image is the same size, so once
            // is enough.
            let _ = self.icon.set_tooltip(Some("EasyVox"));
        }
        self.dark_menu_bar = Some(dark_menu_bar);
    }

    /// Replaces the status rows at the top of the menu: the headline and its line, or each
    /// problem with its fix, then a separator.
    fn set_status_rows(&mut self, status: &Status) -> anyhow::Result<()> {
        for _ in 0..self.status_items {
            self.menu.remove_at(0);
        }
        self.status_items = 0;
        let mut items: Vec<Box<dyn IsMenuItem>> = vec![];
        let note = |text: &str| Box::new(MenuItem::new(text, false, None)) as Box<dyn IsMenuItem>;
        if status.problems.is_empty() {
            items.push(note(&status.headline));
            items.push(note(&status.detail));
        }
        for (i, problem) in status.problems.iter().enumerate() {
            if i > 0 {
                items.push(Box::new(PredefinedMenuItem::separator()));
            }
            let id = format!("{MENU_FIX}{i}");
            // Words and a thin glyph, never colour alone; the title does what the fix does.
            items.push(Box::new(MenuItem::with_id(id.clone(), format!("⚠\u{fe0e} {}", problem.title), true, None)));
            items.push(note(&problem.detail));
            items.push(Box::new(MenuItem::with_id(id, problem.action, true, None)));
        }
        items.push(Box::new(PredefinedMenuItem::separator()));
        let refs: Vec<&dyn IsMenuItem> = items.iter().map(|i| i.as_ref()).collect();
        self.menu.insert_items(&refs, 0)?;
        self.status_items = items.len();
        Ok(())
    }

    /// The fix of the problem whose row `id` is, if it's one.
    pub fn fix(&self, id: &str) -> Option<Fix> {
        let i: usize = id.strip_prefix(MENU_FIX)?.parse().ok()?;
        self.shown.problems.get(i).map(|p| p.fix)
    }
}

#[cfg(test)]
mod tests {
    use super::{Activity, Facts, Fix, Glyph, Status};
    use crate::settings::Page;
    use crate::state::EngineStatus;
    use easyvox_core::MicPermission;

    fn facts(engine: &EngineStatus) -> Facts<'_> {
        Facts {
            activity: Activity::Idle,
            just_pasted: false,
            mic: MicPermission::Authorized,
            accessibility: true,
            engine,
            downloading: false,
            shortcut: "⌥Space".into(),
            hold_right_option: false,
            shortcut_failed: false,
        }
    }

    #[test]
    fn ready_says_how_to_dictate() {
        let ready = EngineStatus::Ready { load_ms: 200 };
        let status = Status::of(&facts(&ready));
        assert_eq!((status.glyph, status.headline.as_str()), (Glyph::Ready, "Ready"));
        assert_eq!(status.detail, "Hold ⌥Space to talk, let go to paste.");
        assert!(status.problems.is_empty());
        let status = Status::of(&Facts { hold_right_option: true, ..facts(&ready) });
        assert_eq!(status.detail, "Hold ⌥Space or right ⌥ to talk, let go to paste.");
        // A freed model reloads at the next press: nothing to show.
        assert_eq!(Status::of(&facts(&EngineStatus::Unloaded)).glyph, Glyph::Ready);
    }

    #[test]
    fn a_dictation_shows_over_a_problem_and_a_problem_over_the_rest() {
        let ready = EngineStatus::Ready { load_ms: 200 };
        let no_paste = Facts { accessibility: false, ..facts(&ready) };
        for (activity, pasted, glyph) in [
            (Activity::Recording, false, Glyph::Listening),
            (Activity::Finishing, false, Glyph::Finishing),
            (Activity::Idle, true, Glyph::Pasted),
            (Activity::Idle, false, Glyph::Attention),
        ] {
            let status = Status::of(&Facts { activity, just_pasted: pasted, ..no_paste.clone() });
            assert_eq!(status.glyph, glyph);
            assert_eq!(status.problems.len(), 1, "the menu names the problem all along");
        }
        let loading = Status::of(&facts(&EngineStatus::Loading));
        assert_eq!((loading.glyph, loading.headline.as_str()), (Glyph::GettingReady, "Getting ready…"));
        let missing = EngineStatus::Missing;
        let downloading = Status::of(&Facts { downloading: true, ..facts(&missing) });
        assert_eq!(
            (downloading.glyph, downloading.detail.as_str()),
            (Glyph::GettingReady, "Downloading the speech model.")
        );
        assert!(downloading.problems.is_empty(), "a model on its way isn't a problem");
    }

    #[test]
    fn each_problem_names_its_fix() {
        let failed = EngineStatus::Failed { error: "bad file".into() };
        let all = Facts { mic: MicPermission::Denied, accessibility: false, shortcut_failed: true, ..facts(&failed) };
        let status = Status::of(&all);
        let fixes: Vec<(&str, Fix)> = status.problems.iter().map(|p| (p.title.as_str(), p.fix)).collect();
        assert_eq!(
            fixes,
            [
                ("Microphone access is off", Fix::SystemSettings(crate::platform::PRIVACY_MICROPHONE)),
                ("Accessibility access is off", Fix::SystemSettings(crate::platform::PRIVACY_ACCESSIBILITY)),
                ("The speech model didn't load", Fix::Settings(Page::Models)),
                ("⌥Space isn't working", Fix::Settings(Page::General)),
            ]
        );
        let unasked = Status::of(&Facts { mic: MicPermission::NotDetermined, ..facts(&EngineStatus::Missing) });
        assert_eq!(unasked.problems[0].fix, Fix::Setup);
        assert_eq!(unasked.problems[1].fix, Fix::Settings(Page::Models));
    }
}
