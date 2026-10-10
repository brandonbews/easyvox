//! The onboarding window ("Set Up EasyVox"): the four steps of `onboarding.rs`, each with where
//! it stands and its buttons, and under the model step the same row Settings → Models shows
//! (`model_row.rs`), with its download, progress, Cancel and Try again. It opens at launch when a
//! required step is missing, and from Settings → Permissions. It looks like Settings:
//! the same theme, header and type sizes.
//!
//! It only ever asks: the Microphone and Accessibility prompts come up when their button is
//! clicked, and nothing here opens the microphone.
//!
//! The permissions are read from the system as the window draws. While one is missing, the
//! window reads them again every second, and whenever it's brought forward, so a switch turned on
//! in System Settings shows here without a relaunch; the timer stops once both are allowed, and
//! goes with the window. The download and the engine come from `AppStatus`, and the controller
//! redraws the window when they change.

use crate::controller;
use crate::model_row::{ModelRow, ModelState};
use crate::onboarding::{Action, Busy, Facts, LaunchFacts, Onboarding, Pane, State, Step, StepId};
use crate::platform::{PRIVACY_ACCESSIBILITY, PRIVACY_MICROPHONE};
use crate::state::{AppSettings, AppStatus};
use easyvox_core::MicPermission;
use easyvox_core::models::{self, ModelInfo};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, Theme, h_flex, v_flex};
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
        let (background, foreground, border, muted) =
            (cx.theme().background, cx.theme().foreground, cx.theme().border, cx.theme().muted_foreground);
        let root = v_flex().size_full().bg(background).text_color(foreground);
        let Some(model) = chosen_model(cx) else {
            return root.child(div().p_4().text_sm().child("This build offers no speech model."));
        };
        let onboarding = Onboarding::new(&self.facts(model, cx));
        let current = onboarding.current();

        // Like a Settings page's header.
        let header = v_flex()
            .p_4()
            .gap_1()
            .border_b_1()
            .border_color(border)
            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("Set up EasyVox"))
            .child(div().text_sm().text_color(muted).child(
                "A few things before your first dictation. Speech turns into text on this Mac; the only \
                 download is the speech model.",
            ));
        let steps = v_flex().id("onboarding-steps").flex_1().min_h_0().overflow_y_scroll().children(
            onboarding
                .steps
                .iter()
                .enumerate()
                .map(|(i, step)| step_row(i + 1, step, current == Some(step.id), model, cx)),
        );
        root.child(header).child(steps).child(footer(&onboarding, cx))
    }
}

/// One step: a badge (its number, a tick, a warning, or a spinner during the first load), its
/// title and words, its buttons, and for the model step the model's row.
fn step_row(number: usize, step: &Step, current: bool, model: &'static ModelInfo, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let badge = h_flex().size(px(22.)).flex_shrink_0().justify_center().items_center().map(|this| match &step.state {
        State::Done => this.child(Icon::new(IconName::CircleCheck).text_color(theme.success)),
        State::Problem(_) => this.child(Icon::new(IconName::TriangleAlert).text_color(theme.danger)),
        // A spinner redraws the window every frame, so only for the ~15 s load, which has no
        // other sign of life. The download has its progress bar, and the prompts wait on you.
        State::Busy(Busy::WarmingUp) => this.child(Spinner::new().small().color(theme.primary)),
        State::ToDo | State::Waiting | State::Busy(_) => {
            let color = if current { theme.primary } else { muted };
            this.rounded_full().border_1().border_color(color).text_xs().text_color(color).child(number.to_string())
        }
    });
    // The model row says what went wrong with a download in red; elsewhere the step's words do.
    let words_color = match step.state {
        State::Problem(_) if step.id != StepId::Model => theme.danger,
        _ => muted,
    };
    let buttons = h_flex()
        .flex_shrink_0()
        .gap_2()
        .children(step.actions.iter().enumerate().filter_map(|(i, action)| action_button(step, i, *action)));
    h_flex().w_full().items_start().gap_3().px_4().py_4().border_b_1().border_color(theme.border).child(badge).child(
        v_flex()
            .flex_1()
            .min_w_0()
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .justify_between()
                    .gap_4()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(step.id.title()))
                            .child(div().text_sm().text_color(words_color).child(step.text.clone())),
                    )
                    .child(buttons),
            )
            .when(step.id == StepId::Model, |this| this.child(ModelRow::new(model))),
    )
}

/// A button for one of a step's actions; the first is the primary one. `None` for the download's
/// own actions, which the model row draws.
fn action_button(step: &Step, index: usize, action: Action) -> Option<Button> {
    let label = match action {
        Action::AskMicrophone | Action::AskAccessibility => "Allow…",
        Action::OpenSettings(_) => "Open System Settings…",
        Action::WarmUp if matches!(step.state, State::Problem(_)) => "Try again",
        Action::WarmUp => "Get ready",
        Action::Download | Action::CancelDownload | Action::RetryDownload => return None,
    };
    let button = Button::new(ElementId::Name(format!("onboarding-{:?}-{index}", step.id).into()))
        .small()
        .label(label)
        .on_click(move |_, _, cx| act(action, cx));
    Some(if index == 0 { button.primary() } else { button.outline() })
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

/// How to dictate once everything is done, or where to find this again; and the button that
/// closes the window.
fn footer(onboarding: &Onboarding, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let finished = onboarding.finished();
    let shortcut = Keystroke::parse(&cx.global::<AppSettings>().shortcut).ok();
    let note = if finished {
        h_flex()
            .gap_1()
            .child("All set. Hold")
            .when_some(shortcut, |this, ks| this.child(Kbd::new(ks)))
            .child("and speak; let go to paste.")
    } else {
        h_flex().child(format!("{} of 4 done. Settings → Permissions opens this again.", onboarding.done()))
    };
    let close = Button::new("onboarding-close")
        .small()
        .label(if finished { "Start dictating" } else { "Finish later" })
        .on_click(|_, window, _| window.remove_window());
    h_flex()
        .px_4()
        .py_3()
        .gap_4()
        .justify_between()
        .items_center()
        .border_t_1()
        .border_color(theme.border)
        .child(note.text_sm().text_color(theme.muted_foreground))
        .child(if finished { close.primary() } else { close.outline() })
}
