//! The Settings window, in the design's look (`design/`, boards 12–15) with EasyVox's own four
//! pages: an inset sidebar with each page's icon, and on the right the page as grouped forms.
//! Every change applies at once.
//!
//! - **General**: the shortcut (a keycap and Change…, see [`ShortcutRecorder`]), Hold right ⌥ to
//!   talk, the speech model in use and Keep model loaded, and the settings file.
//! - **Models**: every offered model, to download, use or delete (`model_row.rs`), and the
//!   models folder.
//! - **Permissions**: the privacy line, the three macOS permissions, and onboarding again.
//! - **About** (`about.rs`).
//!
//! The pages and their groups are plain elements, drawn from the app's state at each render, so
//! the window needs no state of its own beyond the page shown, the shortcut recorder, and its
//! watch on the permissions (`permissions::Watch`, as in setup): it reads them when it opens,
//! whenever it comes forward, and every second while one is missing, so a switch turned on in
//! System Settings shows here without a relaunch. The timer stops once all three are allowed,
//! and goes with the window.

use crate::brand::{self, Colors};
use crate::controller;
use crate::model_row::{ModelRow, download_words};
use crate::permissions::{AllPermissions, Watch};
use crate::platform::{InputMonitoring, PRIVACY_ACCESSIBILITY, PRIVACY_INPUT_MONITORING, PRIVACY_MICROPHONE};
use crate::state::{AppSettings, AppStatus, EngineStatus, RightOptionProblem};
use easyvox_core::MicPermission;
use easyvox_core::download::format_size;
use gpui_kit::component::button::Button;
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme, Sizable as _, Theme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// The window's pages, in sidebar order. Code names one to open it: the tray's About item, the
/// menu bar's fixes, the self-test.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    General,
    Models,
    Permissions,
    About,
}

impl Page {
    const ALL: [Self; 4] = [Self::General, Self::Models, Self::Permissions, Self::About];

    fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Models => "Models",
            Self::Permissions => "Permissions",
            Self::About => "About",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::General => brand::ICON_GENERAL,
            Self::Models => brand::ICON_MODELS,
            Self::Permissions => brand::ICON_PERMISSIONS,
            Self::About => brand::ICON_ABOUT,
        }
    }
}

/// The sidebar's width with its 8 pt inset on each side: a 180 pt panel.
const SIDEBAR: Pixels = px(196.);

pub struct SettingsView {
    recorder: Entity<ShortcutRecorder>,
    /// The page shown: picked in the sidebar, or asked for through [`show`](Self::show).
    page: Page,
    /// The three permissions, read again while one is missing.
    permissions: Watch<AllPermissions>,
    _appearance: Subscription,
}

impl SettingsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let recorder = cx.new(|cx| ShortcutRecorder::new(window, cx));
        let _appearance =
            cx.observe_window_appearance(window, |_, window, cx| Theme::sync_system_appearance(Some(window), cx));
        let permissions = Watch::new(window, cx, Self::watch, AllPermissions::read, |p| !p.all_allowed());
        Self { recorder, page: Page::default(), permissions, _appearance }
    }

    fn watch(&mut self) -> &mut Watch<AllPermissions> {
        &mut self.permissions
    }

    /// Switches to `page`, as if it were clicked in the sidebar.
    pub fn show(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }

    /// The page shown.
    #[cfg(feature = "self-test")]
    pub fn page(&self) -> Page {
        self.page
    }

    /// Shows these permissions instead of the system's (`None`: the system's again).
    #[cfg(feature = "self-test")]
    pub fn pin_permissions(&mut self, permissions: Option<AllPermissions>, cx: &mut Context<Self>) {
        Watch::pin(self, Self::watch, permissions, cx);
    }

    /// How many times the window asked the system for the permissions.
    #[cfg(feature = "self-test")]
    pub fn permission_reads(&self) -> usize {
        self.permissions.reads()
    }

    /// The inset sidebar: a tinted panel (the design's glass, which GPUI can't draw) with a row
    /// per page.
    fn sidebar(&self, c: &'static Colors, cx: &mut Context<Self>) -> impl IntoElement {
        let hover = Hsla::from(rgb(c.sidebar_selected)).opacity(0.5);
        let rows = Page::ALL.map(|page| {
            let selected = page == self.page;
            h_flex()
                .id(ElementId::Name(format!("settings-sidebar-{page:?}").into()))
                .h(px(30.))
                .px(px(8.))
                .gap(px(8.))
                .rounded(px(8.))
                .cursor_pointer()
                .map(|this| {
                    if selected {
                        this.bg(rgb(c.sidebar_selected)).font_weight(FontWeight::MEDIUM)
                    } else {
                        this.hover(|style| style.bg(hover))
                    }
                })
                .child(brand::glyph(page.icon(), px(16.), if selected { c.ink } else { c.ink2 }))
                .child(page.title())
                .on_click(cx.listener(move |this, _, _, cx| this.show(page, cx)))
        });
        div().flex_shrink_0().w(SIDEBAR).h_full().p(px(8.)).child(
            v_flex()
                .size_full()
                .rounded(px(12.))
                .bg(rgb(c.sidebar))
                .text_color(rgb(c.ink))
                .px(px(10.))
                .pt(px(12.))
                .gap(px(2.))
                .children(rows),
        )
    }

    fn general_page(&self, c: &'static Colors, cx: &App) -> Div {
        let settings = cx.global::<AppSettings>();
        let status = cx.global::<AppStatus>();
        let unload_after = duration_words(settings.app.unload_after_secs);

        let right_option = match &status.right_option_problem {
            Some(RightOptionProblem::NeedsInputMonitoring) => {
                brand::problem(c, "Waiting for Input Monitoring. Allow it under Permissions.", brand::TEXT_SMALL)
                    .into_any_element()
            }
            Some(RightOptionProblem::Failed(e)) => brand::problem(c, e.clone(), brand::TEXT_SMALL).into_any_element(),
            None => brand::line(c, "Works alongside the shortcut. Needs Input Monitoring."),
        };
        let shortcut = brand::group(
            c,
            [
                self.recorder.clone().into_any_element(),
                brand::row(
                    "Hold right ⌥ to talk",
                    Some(right_option),
                    Some(switch("hold-right-option", settings.hold_right_option, |on, cx| {
                        controller::with(cx, |c, cx| c.set_hold_right_option(on, cx));
                    })),
                )
                .into_any_element(),
            ],
        );

        let chosen = settings.model.as_str();
        let engine = match &status.engine {
            EngineStatus::NotLoaded => Ok("Not loaded yet.".to_string()),
            EngineStatus::Missing => Ok(match status.downloads.get(chosen) {
                Some(download) => download_words(download),
                None => {
                    let size = easyvox_core::models::find(chosen).map(|m| format_size(m.download_bytes));
                    format!("Not downloaded yet ({}).", size.unwrap_or_default())
                }
            }),
            EngineStatus::Loading => Ok("Loading… The first load after installing takes about 15 s.".into()),
            EngineStatus::Ready { load_ms } => Ok(format!(
                "Loaded in {:.1} s. It writes both the live preview and the final text.",
                *load_ms as f32 / 1000.
            )),
            EngineStatus::Unloaded => {
                Ok(format!("Freed after {unload_after} unused. It loads again as your next dictation starts."))
            }
            EngineStatus::Failed { error } => Err(format!("Couldn't load: {error}")),
        };
        let engine = match engine {
            Ok(words) => {
                let end = if words.ends_with(['.', '…']) { "" } else { "." };
                brand::line(c, format!("{words}{end} Download, switch or delete models under Models."))
            }
            Err(problem) => brand::problem(c, problem, brand::TEXT_SMALL).into_any_element(),
        };
        let keep_loaded = format!(
            "Off: after {unload_after} without dictation, EasyVox frees the model's ~600 MB. Your next \
             dictation still starts at once and loses nothing while it loads again (about 0.15 s). On: it \
             stays in memory."
        );
        let model = brand::group(
            c,
            [
                brand::row("Speech model", Some(engine), Some(model_picker(c, cx))).into_any_element(),
                brand::row(
                    "Keep model loaded",
                    Some(brand::line(c, keep_loaded)),
                    Some(switch("keep-model-loaded", settings.keep_model_loaded, |on, cx| {
                        controller::with(cx, |c, cx| c.set_keep_model_loaded(on, cx));
                    })),
                )
                .into_any_element(),
            ],
        );

        let config_path = status.config_path.clone();
        let reveal = config_path.clone();
        let file = brand::group(
            c,
            [brand::row(
                "Settings file",
                Some(brand::line(c, home_relative(&config_path))),
                Some(
                    brand::button("config-file", "Show in Finder")
                        .on_click(move |_, _, cx| {
                            // The folder always exists (the watcher makes it); the file only once written.
                            let target = if reveal.exists() {
                                reveal.clone()
                            } else {
                                reveal.parent().unwrap_or(&reveal).into()
                            };
                            cx.reveal_path(&target);
                        })
                        .into_any_element(),
                ),
            )
            .into_any_element()],
        );
        let config_note = match (&status.config_error, status.config_warnings.as_slice()) {
            (Some(e), _) => brand::problem(
                c,
                format!("{e}. Changes made here apply until EasyVox quits, but aren't saved until the file is fixed."),
                brand::TEXT_SMALL,
            )
            .mt(px(6.))
            .mx(px(12.)),
            (None, []) => brand::footnote(
                c,
                "Every setting is in this file, including advanced ones this window doesn't show (docs/settings-reference.md \
                 lists them). Edits to it apply as soon as you save, and changes here are written to it.",
            ),
            (None, warnings) => {
                brand::problem(c, format!("Some of this file was ignored: {}", warnings.join(" · ")), brand::TEXT_SMALL)
                    .mt(px(6.))
                    .mx(px(12.))
            }
        };

        v_flex()
            .child(shortcut)
            .child(div().h(px(20.)))
            .child(model)
            .child(div().h(px(20.)))
            .child(file)
            .child(config_note)
    }
}

/// Every model the app offers, downloaded or not, with what you can do with each; then the
/// folder they're saved in.
fn models_page(c: &'static Colors, cx: &App) -> Div {
    let models_dir = cx.global::<AppStatus>().models_dir.clone();
    let rows = easyvox_core::models::offered()
        .into_iter()
        .map(|m| div().py(px(10.)).child(ModelRow::new(m)).into_any_element());
    let reveal = models_dir.clone();
    let folder = brand::row(
        "Models folder",
        Some(brand::line(c, home_relative(&models_dir))),
        Some(
            brand::button("models-folder", "Show in Finder")
                .on_click(move |_, _, cx| {
                    // The folder exists once something was downloaded into it.
                    let target =
                        if reveal.exists() { reveal.clone() } else { reveal.parent().unwrap_or(&reveal).into() };
                    cx.reveal_path(&target);
                })
                .into_any_element(),
        ),
    );
    v_flex()
        .child(brand::group(c, rows))
        .child(brand::footnote(
            c,
            "EasyVox runs one model at a time, on this Mac. A switch applies from your next dictation.",
        ))
        .child(div().h(px(20.)))
        .child(brand::group(c, [folder.into_any_element()]))
        .child(brand::footnote(c, "Each model is saved here with a LICENSE.txt that credits it."))
}

/// What EasyVox does with your audio, the three macOS permissions it uses, as last read, and
/// the first launch's steps again.
fn permissions_page(permissions: AllPermissions, c: &'static Colors, cx: &App) -> Div {
    let status = cx.global::<AppStatus>();
    let mic = mic_state(permissions.mic, status.mic_asking);
    let accessibility = accessibility_state(permissions.accessibility, status.accessibility_prompted);
    let input_monitoring = input_monitoring_state(permissions.input_monitoring);
    let privacy = brand::group(
        c,
        [
            brand::row_with(
                div()
                    .text_size(brand::TEXT)
                    .child("Audio is processed on this Mac and never saved.")
                    .into_any_element(),
                None,
            )
            .into_any_element(),
            brand::row(
                "Microphone",
                Some(brand::line(c, "Needed to dictate. Used only while you hold the shortcut.")),
                Some(permission(c, "permission-microphone", mic)),
            )
            .into_any_element(),
            brand::row(
                "Accessibility",
                Some(brand::line(c, "Pastes your words where you're typing. Without it, you paste them with ⌘V.")),
                Some(permission(c, "permission-accessibility", accessibility)),
            )
            .into_any_element(),
            brand::row(
                "Input Monitoring",
                Some(brand::line(c, "Only for holding right ⌥ to talk.")),
                Some(permission(c, "permission-input-monitoring", input_monitoring)),
            )
            .into_any_element(),
        ],
    );
    let setup = brand::row(
        "Set up EasyVox",
        Some(brand::line(c, "The first launch's steps again: the Microphone, Accessibility and the speech model.")),
        Some(
            brand::button("open-onboarding", "Open setup…")
                .on_click(|_, _, cx| {
                    controller::with(cx, |c, cx| c.open_onboarding(cx));
                })
                .into_any_element(),
        ),
    );
    v_flex()
        .child(privacy)
        .child(brand::footnote(c, "You can change these permissions in System Settings › Privacy & Security."))
        .child(brand::section("Setup"))
        .child(brand::group(c, [setup.into_any_element()]))
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = brand::current(cx);
        let page = self.page;
        let content = match page {
            Page::General => self.general_page(c, cx),
            Page::Models => models_page(c, cx),
            Page::Permissions => permissions_page(self.permissions.get(), c, cx),
            Page::About => crate::about::page(c, cx),
        };
        h_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_size(brand::TEXT)
            .child(self.sidebar(c, cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(
                        div()
                            .flex_shrink_0()
                            .pl(px(8.))
                            .pr(px(20.))
                            .pt(px(14.))
                            .pb(px(10.))
                            .text_size(brand::TEXT_TITLE)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(page.title()),
                    )
                    .child(
                        // One scroll area per page, so each opens at its top.
                        v_flex()
                            .flex_1()
                            .w_full()
                            .pl(px(8.))
                            .pr(px(20.))
                            .pb(px(20.))
                            .child(content)
                            .overflow_y_scrollbar()
                            .id(ElementId::Name(format!("settings-page-{page:?}").into())),
                    ),
            )
    }
}

/// A switch that calls `set` with its new state.
fn switch(id: &'static str, on: bool, set: impl Fn(bool, &mut App) + 'static) -> AnyElement {
    Switch::new(id).checked(on).on_click(move |checked: &bool, _, cx| set(*checked, cx)).into_any_element()
}

/// A pop-up button with the downloaded models, the one in use ticked.
fn model_picker(c: &'static Colors, cx: &App) -> AnyElement {
    let models_dir = &cx.global::<AppStatus>().models_dir;
    // The models the app offers (default first) that are downloaded and runnable here.
    let models: Vec<(SharedString, SharedString)> = easyvox_core::models::offered()
        .into_iter()
        .filter(|m| m.is_available(models_dir))
        .map(|m| (m.id.into(), m.short_name().into()))
        .collect();
    if models.is_empty() {
        return div().text_color(rgb(c.ink2)).child("None downloaded yet").into_any_element();
    }
    let chosen: SharedString = cx.global::<AppSettings>().model.clone().into();
    let label = models.iter().find(|(id, _)| *id == chosen).map_or_else(|| chosen.clone(), |(_, name)| name.clone());
    Button::new("model-picker")
        .small()
        .outline()
        .rounded(px(6.))
        .label(label)
        .dropdown_caret(true)
        .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
            models.iter().fold(menu, |menu, (id, name)| {
                let id = id.clone();
                menu.item(PopupMenuItem::new(name.clone()).checked(id == chosen).on_click(move |_, _, cx| {
                    controller::with(cx, |c, cx| c.select_model(&id, cx));
                }))
            })
        })
        .into_any_element()
}

/// Where a permission stands, in words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Granted {
    Yes,
    No(&'static str),
    /// Not answered yet: no glyph, nothing is wrong.
    Waiting(&'static str),
}

/// What a permission's button does: ask (macOS's prompt, which also puts EasyVox in the
/// permission's list), or open System Settings at the list once asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fix {
    AskMicrophone,
    AskAccessibility,
    AskInputMonitoring,
    OpenSettings(&'static str),
}

impl Fix {
    fn label(self) -> &'static str {
        match self {
            Self::AskMicrophone | Self::AskAccessibility | Self::AskInputMonitoring => "Allow…",
            Self::OpenSettings(_) => "Open System Settings…",
        }
    }

    fn apply(self, cx: &mut App) {
        match self {
            Self::AskMicrophone => {
                controller::with(cx, |c, cx| c.request_mic(cx));
            }
            Self::AskAccessibility => {
                controller::with(cx, |c, cx| c.prompt_accessibility(cx));
            }
            Self::AskInputMonitoring => {
                controller::with(cx, |c, cx| c.request_input_monitoring(cx));
            }
            Self::OpenSettings(url) => cx.open_url(url),
        }
    }
}

/// The Microphone's row, from its answer and whether its prompt is up.
fn mic_state(mic: MicPermission, asking: bool) -> (Granted, Option<Fix>) {
    let open = Some(Fix::OpenSettings(PRIVACY_MICROPHONE));
    match (mic, asking) {
        (_, true) => (Granted::Waiting("Waiting for your answer…"), None),
        (MicPermission::Authorized, _) => (Granted::Yes, None),
        (MicPermission::NotDetermined, _) => (Granted::Waiting("Not asked yet"), Some(Fix::AskMicrophone)),
        (MicPermission::Denied, _) => (Granted::No("Not allowed"), open),
        (MicPermission::Restricted, _) => (Granted::No("Restricted on this Mac"), None),
        (MicPermission::Unknown, _) => (Granted::No("Unknown"), open),
    }
}

/// Accessibility's row. macOS has no "not asked" to read back, so the button asks until the
/// prompt was shown in this launch, then opens the list, as in setup.
fn accessibility_state(trusted: bool, prompted: bool) -> (Granted, Option<Fix>) {
    match (trusted, prompted) {
        (true, _) => (Granted::Yes, None),
        (false, false) => (Granted::No("Not allowed"), Some(Fix::AskAccessibility)),
        (false, true) => (Granted::No("Not allowed"), Some(Fix::OpenSettings(PRIVACY_ACCESSIBILITY))),
    }
}

/// Input Monitoring's row: macOS says whether it was asked, so the button asks first, then
/// opens the list.
fn input_monitoring_state(state: InputMonitoring) -> (Granted, Option<Fix>) {
    match state {
        InputMonitoring::Allowed => (Granted::Yes, None),
        InputMonitoring::NotAsked => (Granted::Waiting("Not asked yet"), Some(Fix::AskInputMonitoring)),
        InputMonitoring::Denied => (Granted::No("Not allowed"), Some(Fix::OpenSettings(PRIVACY_INPUT_MONITORING))),
    }
}

/// A permission's state, in words with a thin glyph (a tick, or a warning), and the button that
/// moves it on.
fn permission(c: &'static Colors, id: &'static str, (granted, fix): (Granted, Option<Fix>)) -> AnyElement {
    let (glyph, words) = match granted {
        Granted::Yes => (Some(brand::GLYPH_ALLOWED), "Allowed"),
        Granted::No(words) => (Some(brand::GLYPH_WARNING), words),
        Granted::Waiting(words) => (None, words),
    };
    let button = fix.map(|fix| brand::button(id, fix.label()).on_click(move |_, _, cx| fix.apply(cx)));
    h_flex()
        .gap_3()
        .child(
            h_flex()
                .gap(px(6.))
                .text_color(rgb(c.ink2))
                .when_some(glyph, |this, glyph| this.child(brand::glyph(glyph, px(15.), c.ink2)))
                .child(words),
        )
        .when_some(button, |this, button| this.child(button))
        .into_any_element()
}

/// "10 minutes", "45 seconds".
fn duration_words(secs: u64) -> String {
    match secs {
        1 => "1 second".into(),
        s if s < 120 => format!("{s} seconds"),
        s => format!("{} minutes", s.div_ceil(60)),
    }
}

/// `path` with the home folder shown as `~`.
fn home_relative(path: &std::path::Path) -> String {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    match home.as_deref().and_then(|h| path.strip_prefix(h).ok()) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// The Shortcut row of General: the shortcut as a keycap with Change…, which waits for a new
/// combination (Cancel or Esc keeps the old one). It becomes the push-to-talk shortcut as soon as
/// it's pressed; a taken or modifier-less one is refused in words, and the old one stays. The OS
/// hot key is suspended while waiting, or Carbon would swallow the combination.
pub struct ShortcutRecorder {
    focus: FocusHandle,
    recording: bool,
    error: Option<SharedString>,
    _subscriptions: [Subscription; 2],
}

impl ShortcutRecorder {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        // Stop recording if focus moves away or the window closes, or the shortcut stays off.
        let on_blur = cx.on_blur(&focus, window, |this, _, cx| this.stop(cx));
        let on_release = cx.on_release(|this, cx| {
            if this.recording {
                controller::with(cx, |c, _| c.hotkeys.resume());
            }
        });
        Self { focus, recording: false, error: None, _subscriptions: [on_blur, on_release] }
    }

    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.recording {
            self.recording = true;
            self.error = None;
            controller::with(cx, |c, _| c.hotkeys.suspend());
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        if self.recording {
            self.recording = false;
            controller::with(cx, |c, _| c.hotkeys.resume());
            cx.notify();
        }
    }

    fn handle_keystroke(&mut self, ks: &Keystroke, cx: &mut Context<Self>) {
        if ks.key == "escape" {
            self.error = None;
            return self.stop(cx);
        }
        let result = crate::hotkey::accept_recorded(ks)
            .and_then(|s| controller::with(cx, |c, cx| c.rebind(&s, cx)).unwrap_or(Err("no controller".into())));
        match result {
            Ok(()) => {
                // `rebind` registered the new shortcut, which also ends the suspension.
                self.recording = false;
                self.error = None;
            }
            Err(e) => self.error = Some(e.into()),
        }
        cx.notify();
    }
}

impl Render for ShortcutRecorder {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = brand::current(cx);
        let current = Keystroke::parse(&cx.global::<AppSettings>().shortcut).ok();
        let error = self.error.clone().or_else(|| cx.global::<AppStatus>().shortcut_error.clone().map(Into::into));
        let line = match (error, self.recording) {
            (Some(e), _) => brand::problem(c, e, brand::TEXT_SMALL).into_any_element(),
            (None, true) => brand::line(c, "Press the keys you want to hold down. Esc keeps the current one."),
            (None, false) => brand::line(c, "Hold it in any app and talk; let go to paste."),
        };
        let field = h_flex()
            .id("shortcut-recorder")
            .track_focus(&self.focus)
            .cursor_pointer()
            .on_click(cx.listener(|this, _, window, cx| this.start(window, cx)))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                if this.recording {
                    cx.stop_propagation();
                    this.handle_keystroke(&ev.keystroke, cx);
                }
            }))
            .map(|this| match (self.recording, current) {
                // Waiting for keys: a field with the focus ring's vermilion.
                (true, _) => this
                    .h(px(22.))
                    .min_w(px(128.))
                    .px_2()
                    .justify_center()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(cx.theme().ring)
                    .text_color(rgb(c.ink2))
                    .child("Type a shortcut"),
                (false, Some(ks)) => this.child(brand::keycap(c, Kbd::format(&ks))),
                (false, None) => this.text_color(rgb(c.ink2)).child("Not set"),
            });
        let button = if self.recording {
            brand::button("shortcut-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| {
                this.error = None;
                this.stop(cx);
            }))
        } else {
            brand::button("shortcut-change", "Change…")
                .on_click(cx.listener(|this, _, window, cx| this.start(window, cx)))
        };
        brand::row("Shortcut", Some(line), Some(h_flex().gap_2().child(field).child(button).into_any_element()))
    }
}

#[cfg(test)]
mod tests {
    // Not `super::*`: that brings in GPUI's own `test` macro, which shadows `#[test]`.
    use super::{Fix, Granted, accessibility_state, input_monitoring_state, mic_state};
    use crate::platform::{InputMonitoring, PRIVACY_ACCESSIBILITY, PRIVACY_INPUT_MONITORING, PRIVACY_MICROPHONE};
    use easyvox_core::MicPermission;

    #[test]
    fn input_monitoring_asks_first_then_opens_the_list() {
        assert_eq!(input_monitoring_state(InputMonitoring::Allowed), (Granted::Yes, None));
        assert_eq!(
            input_monitoring_state(InputMonitoring::NotAsked),
            (Granted::Waiting("Not asked yet"), Some(Fix::AskInputMonitoring))
        );
        assert_eq!(
            input_monitoring_state(InputMonitoring::Denied),
            (Granted::No("Not allowed"), Some(Fix::OpenSettings(PRIVACY_INPUT_MONITORING)))
        );
    }

    #[test]
    fn accessibility_asks_until_prompted_then_opens_the_list() {
        assert_eq!(accessibility_state(true, false), (Granted::Yes, None));
        assert_eq!(accessibility_state(true, true), (Granted::Yes, None));
        assert_eq!(accessibility_state(false, false), (Granted::No("Not allowed"), Some(Fix::AskAccessibility)));
        assert_eq!(
            accessibility_state(false, true),
            (Granted::No("Not allowed"), Some(Fix::OpenSettings(PRIVACY_ACCESSIBILITY)))
        );
    }

    #[test]
    fn the_microphone_waits_while_asked_and_says_where_to_turn_it_on() {
        assert_eq!(mic_state(MicPermission::NotDetermined, true), (Granted::Waiting("Waiting for your answer…"), None));
        assert_eq!(mic_state(MicPermission::Authorized, false), (Granted::Yes, None));
        assert_eq!(mic_state(MicPermission::NotDetermined, false).1, Some(Fix::AskMicrophone));
        assert_eq!(mic_state(MicPermission::Denied, false).1, Some(Fix::OpenSettings(PRIVACY_MICROPHONE)));
        assert_eq!(mic_state(MicPermission::Restricted, false), (Granted::No("Restricted on this Mac"), None));
    }

    #[test]
    fn every_button_says_what_it_does() {
        for fix in [Fix::AskMicrophone, Fix::AskAccessibility, Fix::AskInputMonitoring] {
            assert_eq!(fix.label(), "Allow…");
        }
        assert_eq!(Fix::OpenSettings(PRIVACY_INPUT_MONITORING).label(), "Open System Settings…");
    }
}
