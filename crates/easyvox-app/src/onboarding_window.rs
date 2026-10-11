//! The onboarding window ("Set Up EasyVox"): the four steps of `onboarding.rs`, each with where
//! it stands and its buttons, and under the model step the same row Settings → Models shows
//! (`model_row.rs`), with its download, progress, Cancel and Try again. It opens at launch when a
//! required step is missing, and from Settings → Permissions.
//!
//! It's the design's setup (`design/`, boards 10–11) as a checklist: the steps can be done in any
//! order and the download runs while you answer the prompts, so it stays one page. The header has
//! the app icon and the design's voice; the steps are rows of one group, the step to do next with
//! a vermilion number and the only vermilion button, a done step with a tick, a problem in words
//! with a thin warning glyph; the footer recaps where things stand.
//!
//! It only ever asks: the Microphone and Accessibility prompts come up when their button is
//! clicked, and nothing here opens the microphone.
//!
//! The permissions are read from the system as the window draws. While one is missing, the
//! window reads them again every second, and whenever it's brought forward, so a switch turned on
//! in System Settings shows here without a relaunch; the timer stops once both are allowed, and
//! goes with the window. The download and the engine come from `AppStatus`, and the controller
//! redraws the window when they change.

use crate::brand::{self, Colors};
use crate::controller;
use crate::model_row::{ModelRow, ModelState};
use crate::onboarding::{Action, Busy, Facts, LaunchFacts, Onboarding, Pane, State, Step, StepId};
use crate::platform::{PRIVACY_ACCESSIBILITY, PRIVACY_MICROPHONE};
use crate::state::{AppSettings, AppStatus};
use easyvox_core::MicPermission;
use easyvox_core::models::{self, ModelInfo};
use gpui_kit::component::button::Button;
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, Theme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::time::Duration;

/// How often the permissions are read again while one is missing.
const PERMISSION_POLL: Duration = Duration::from_secs(1);

/// What the system says about the two permissions onboarding asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Permissions {
    pub mic: MicPermission,
    pub accessibility: bool,
}

impl Permissions {
    /// Never prompts.
    pub fn read() -> Self {
        Self { mic: easyvox_core::mic_permission_status(), accessibility: easyvox_core::accessibility_trusted() }
    }

    fn all_allowed(self) -> bool {
        self.mic == MicPermission::Authorized && self.accessibility
    }
}

/// The model onboarding sets up: the chosen one, which is the default unless the settings file
/// picks another offered model.
pub fn chosen_model(cx: &App) -> Option<&'static ModelInfo> {
    let chosen = models::find(&cx.global::<AppSettings>().model);
    chosen.or_else(|_| models::find(easyvox_core::RECOMMENDED_MODEL)).ok()
}

/// What decides at launch whether onboarding opens: the system's answers and the models folder.
pub fn launch_facts(cx: &App) -> LaunchFacts {
    let permissions = Permissions::read();
    let models_dir = &cx.global::<AppStatus>().models_dir;
    LaunchFacts {
        mic: permissions.mic,
        accessibility: permissions.accessibility,
        model_downloaded: chosen_model(cx).is_some_and(|m| m.is_available(models_dir)),
    }
}

/// How many times the window was drawn: the self-test checks that it really draws.
#[cfg(feature = "self-test")]
pub static DRAWN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub struct OnboardingView {
    /// The permissions when last read, to redraw when they change.
    seen: Permissions,
    /// Reads them again every [`PERMISSION_POLL`] while one is missing.
    poll: Option<Task<()>>,
    /// Self-test only: permissions to show instead of the system's, which it never changes.
    #[cfg(feature = "self-test")]
    pinned: Option<Permissions>,
    _subscriptions: [Subscription; 2],
}

impl OnboardingView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let appearance =
            cx.observe_window_appearance(window, |_, window, cx| Theme::sync_system_appearance(Some(window), cx));
        // Back from System Settings: read the permissions now, and watch again if one is missing.
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() {
                this.watch_permissions(cx);
            }
        });
        let mut this = Self {
            seen: Permissions::read(),
            poll: None,
            #[cfg(feature = "self-test")]
            pinned: None,
            _subscriptions: [appearance, activation],
        };
        this.watch_permissions(cx);
        this
    }

    fn permissions(&self) -> Permissions {
        #[cfg(feature = "self-test")]
        if let Some(pinned) = self.pinned {
            return pinned;
        }
        Permissions::read()
    }

    /// Reads the permissions now, then every [`PERMISSION_POLL`] until both are allowed,
    /// redrawing when they change. Replaces any earlier timer.
    fn watch_permissions(&mut self, cx: &mut Context<Self>) {
        if !self.check_permissions(cx) {
            self.poll = None;
            return;
        }
        self.poll = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(PERMISSION_POLL).await;
                match this.update(cx, |view, cx| view.check_permissions(cx)) {
                    Ok(true) => {}
                    _ => break, // both allowed, or the window is gone
                }
            }
        }));
    }

    /// Redraws if the permissions changed. True while one is still missing.
    fn check_permissions(&mut self, cx: &mut Context<Self>) -> bool {
        let now = self.permissions();
        if now != self.seen {
            log::info!("onboarding: permissions now {now:?}");
            self.seen = now;
            cx.notify();
        }
        !now.all_allowed()
    }

    fn facts(&self, model: &ModelInfo, cx: &App) -> Facts {
        let permissions = self.permissions();
        let status = cx.global::<AppStatus>();
        Facts {
            mic: permissions.mic,
            mic_asking: status.mic_asking,
            accessibility: permissions.accessibility,
            accessibility_asked: status.accessibility_prompted,
            model: ModelState::of(model, &status.downloads, &status.models_dir),
            engine: status.engine.clone(),
        }
    }

    /// The steps as the window shows them now.
    #[cfg(feature = "self-test")]
    pub fn onboarding(&self, cx: &App) -> Option<Onboarding> {
        chosen_model(cx).map(|model| Onboarding::new(&self.facts(model, cx)))
    }

    /// Shows these permissions instead of the system's (`None`: the system's again).
    #[cfg(feature = "self-test")]
    pub fn pin_permissions(&mut self, permissions: Option<Permissions>, cx: &mut Context<Self>) {
        self.pinned = permissions;
        self.watch_permissions(cx);
        cx.notify();
    }
}

impl Render for OnboardingView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(feature = "self-test")]
        DRAWN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let c = brand::current(cx);
        let root =
            v_flex().size_full().bg(cx.theme().background).text_color(cx.theme().foreground).text_size(brand::TEXT);
        let Some(model) = chosen_model(cx) else {
            return root.child(div().p_4().child("This build offers no speech model."));
        };
        let onboarding = Onboarding::new(&self.facts(model, cx));
        let current = onboarding.current();

        let header = v_flex()
            .flex_shrink_0()
            .items_center()
            .px(px(32.))
            .pt(px(14.))
            .pb(px(14.))
            .child(img(brand::ICON_128).size(px(48.)))
            .child(
                div()
                    .mt(px(8.))
                    .text_size(brand::TEXT_HERO)
                    .line_height(px(28.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Talk, and EasyVox types."),
            )
            .child(div().mt(px(4.)).max_w(px(456.)).text_center().line_height(px(18.)).text_color(rgb(c.ink2)).child(
                format!(
                    "Hold {} in any app, talk, and let go to paste. Four things first, in any order; then \
                     everything runs on this Mac.",
                    shortcut_name(cx)
                ),
            ));
        let steps = onboarding
            .steps
            .iter()
            .enumerate()
            .map(|(i, step)| step_row(i + 1, step, current == Some(step.id), model, c, cx).into_any_element());
        let steps = v_flex()
            .flex_1()
            .w_full()
            .px(px(32.))
            .pb(px(12.))
            .child(brand::group(c, steps))
            .overflow_y_scrollbar()
            .id("onboarding-steps");
        root.child(header).child(steps).child(footer(&onboarding, c, cx))
    }
}

/// The push-to-talk shortcut as macOS writes it: "⌥Space".
fn shortcut_name(cx: &App) -> String {
    let shortcut = &cx.global::<AppSettings>().shortcut;
    Keystroke::parse(shortcut).map_or_else(|_| shortcut.clone(), |ks| Kbd::format(&ks))
}

/// One step: a badge (its number, vermilion if it's the one to do next; a tick; a warning; or a
/// spinner during the first load), its title and words, its buttons, and for the model step the
/// model's row.
fn step_row(
    number: usize,
    step: &Step,
    current: bool,
    model: &'static ModelInfo,
    c: &'static Colors,
    cx: &App,
) -> impl IntoElement {
    let badge = h_flex().size(px(22.)).flex_shrink_0().justify_center().items_center().map(|this| match &step.state {
        State::Done => this.child(brand::glyph(brand::GLYPH_PASTED, px(20.), c.ink)),
        State::Problem(_) => this.child(brand::glyph(brand::GLYPH_WARNING, px(18.), c.ink)),
        // A spinner redraws the window every frame, so only for the ~15 s load, which has no
        // other sign of life. The download has its progress bar, and the prompts wait on you.
        State::Busy(Busy::WarmingUp) => this.child(Spinner::new().small().color(cx.theme().primary)),
        // The button's vermilion, which keeps a white number legible (4.6:1).
        State::ToDo | State::Waiting | State::Busy(_) if current => this
            .rounded_full()
            .bg(rgb(c.button_fill))
            .text_color(rgb(c.button_text))
            .text_size(brand::TEXT_KEY)
            .font_weight(FontWeight::SEMIBOLD)
            .child(number.to_string()),
        State::ToDo | State::Waiting | State::Busy(_) => this
            .rounded_full()
            .border_1()
            .border_color(rgb(c.keycap_border))
            .text_color(rgb(c.ink2))
            .text_size(brand::TEXT_KEY)
            .child(number.to_string()),
    });
    // A problem's words are in the text colour, next to the warning glyph. The model row says
    // what went wrong with a download itself; this step's words say what to do.
    let words = match step.state {
        State::Problem(_) => c.ink,
        _ => c.ink2,
    };
    let buttons = h_flex()
        .flex_shrink_0()
        .gap_2()
        .children(step.actions.iter().enumerate().filter_map(|(i, action)| action_button(step, i, *action, current)));
    // The title shares its line with the buttons, and the words get the row's whole width.
    h_flex().w_full().items_start().gap_3().py(px(12.)).child(badge).child(
        v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(2.))
            .child(
                h_flex()
                    .w_full()
                    .min_h(px(22.))
                    .justify_between()
                    .gap_4()
                    .child(div().font_weight(FontWeight::MEDIUM).child(step.id.title()))
                    .child(buttons),
            )
            .child(
                div().text_size(brand::TEXT_KEY).line_height(px(16.)).text_color(rgb(words)).child(step.text.clone()),
            )
            .when(step.id == StepId::Model, |this| {
                this.child(div().mt(px(8.)).child(ModelRow::new(model).emphasised(current)))
            }),
    )
}

/// A button for one of a step's actions. The first action of the step to do next is the
/// vermilion one; the others are plain. `None` for the download's own actions, which the model
/// row draws.
fn action_button(step: &Step, index: usize, action: Action, current: bool) -> Option<Button> {
    let label = match action {
        Action::AskMicrophone | Action::AskAccessibility => "Allow…",
        Action::OpenSettings(_) => "Open System Settings…",
        Action::WarmUp if matches!(step.state, State::Problem(_)) => "Try again",
        Action::WarmUp => "Get ready",
        Action::Download | Action::CancelDownload | Action::RetryDownload => return None,
    };
    let id = ElementId::Name(format!("onboarding-{:?}-{index}", step.id).into());
    let button = if current && index == 0 { brand::primary(id, label) } else { brand::button(id, label) };
    Some(button.on_click(move |_, _, cx| act(action, cx)))
}

fn act(action: Action, cx: &mut App) {
    match action {
        Action::AskMicrophone => {
            controller::with(cx, |c, cx| c.request_mic(cx));
        }
        Action::AskAccessibility => {
            controller::with(cx, |c, cx| c.prompt_accessibility(cx));
        }
        Action::OpenSettings(Pane::Microphone) => cx.open_url(PRIVACY_MICROPHONE),
        Action::OpenSettings(Pane::Accessibility) => cx.open_url(PRIVACY_ACCESSIBILITY),
        Action::WarmUp => {
            controller::with(cx, |c, cx| c.warm_up(cx));
        }
        Action::Download | Action::CancelDownload | Action::RetryDownload => {}
    }
}

/// The recap: a dot per step (a done one dark, the one to do next wide), where things stand
/// and where this window is found again, or once all is done how to dictate; and the button
/// that closes the window.
fn footer(onboarding: &Onboarding, c: &'static Colors, cx: &App) -> impl IntoElement {
    let finished = onboarding.finished();
    let current = onboarding.current();
    let dots = h_flex().flex_shrink_0().gap(px(6.)).children(onboarding.steps.iter().map(|step| {
        let next = current == Some(step.id);
        let filled = step.is_done() || next;
        div().h(px(6.)).w(px(if next { 18. } else { 6. })).rounded(px(3.)).bg(rgb(if filled {
            c.ink
        } else {
            c.keycap_border
        }))
    }));
    let (title, words) = if finished {
        (
            "You're all set.".to_string(),
            format!(
                "EasyVox lives in the menu bar. Hold {} in any app to talk; click its icon for History and Settings.",
                shortcut_name(cx)
            ),
        )
    } else {
        (format!("{} of 4 done.", onboarding.done()), "Settings › Permissions opens this again.".to_string())
    };
    let close = if finished {
        brand::primary("onboarding-close", "Start dictating")
    } else {
        brand::button("onboarding-close", "Finish later")
    };
    h_flex()
        .flex_shrink_0()
        .px(px(32.))
        .py(px(14.))
        .gap_4()
        .items_center()
        .border_t_1()
        .border_color(rgb(c.hairline))
        .child(dots)
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(div().font_weight(FontWeight::MEDIUM).child(title))
                .child(div().text_size(brand::TEXT_KEY).line_height(px(16.)).text_color(rgb(c.ink2)).child(words)),
        )
        .child(close.on_click(|_, window, _| window.remove_window()))
}
