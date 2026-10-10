//! The Settings window, built from GPUI Kit's `Settings` component (pages, groups, items), plus a
//! shortcut recorder and permission rows.

use crate::controller;
use crate::state::{AppSettings, AppStatus, DownloadStatus, EngineStatus};
use easyvox_core::MicPermission;
use easyvox_core::download::{Progress, format_size};
use gpui_kit::component::button::Button;
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings};
use gpui_kit::component::{ActiveTheme, Sizable, Theme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::time::Duration;

const PRIVACY_MICROPHONE: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";
const PRIVACY_ACCESSIBILITY: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
const PRIVACY_INPUT_MONITORING: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent";

/// How long to wait for an answer to the Microphone prompt.
const MIC_PROMPT_TIMEOUT: Duration = Duration::from_secs(120);

pub struct SettingsView {
    recorder: Entity<ShortcutRecorder>,
    /// Set while the system Microphone prompt is up.
    mic_request: Option<Task<()>>,
    _appearance: Subscription,
}

impl SettingsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let recorder = cx.new(|cx| ShortcutRecorder::new(window, cx));
        let _appearance =
            cx.observe_window_appearance(window, |_, window, cx| Theme::sync_system_appearance(Some(window), cx));
        Self { recorder, mic_request: None, _appearance }
    }

    /// Shows the system Microphone prompt. `request_mic_permission` blocks until the user
    /// answers, so it runs on the background executor.
    fn request_mic(&mut self, cx: &mut Context<Self>) {
        if self.mic_request.is_some() {
            return;
        }
        log::info!("asking for Microphone access");
        self.mic_request = Some(cx.spawn(async move |this, cx| {
            let status = cx.background_spawn(async { easyvox_core::request_mic_permission(MIC_PROMPT_TIMEOUT) }).await;
            log::info!("Microphone access: {status:?}");
            let _ = this.update(cx, |this, cx| {
                this.mic_request = None;
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn general_page(&self, cx: &App) -> SettingPage {
        let recorder = self.recorder.clone();
        let models_dir = easyvox_core::paths::models_dir();
        // The models the app offers (default first) that are downloaded and runnable here.
        let models: Vec<(SharedString, SharedString)> = easyvox_core::models::offered()
            .into_iter()
            .filter(|m| m.is_available(&models_dir))
            .map(|m| (m.id.into(), m.name.into()))
            .collect();
        let model_field = if models.is_empty() {
            let message = format!("No models found in {}", models_dir.display());
            SettingField::render(move |_, _, _| div().text_sm().child(message.clone()))
        } else {
            SettingField::dropdown(
                models,
                |cx: &App| cx.global::<AppSettings>().model.clone().into(),
                |id: SharedString, cx: &mut App| {
                    controller::with(cx, |c, cx| c.select_model(&id, cx));
                },
            )
        };
        let status = cx.global::<AppStatus>();
        let unload_after = duration_words(cx.global::<AppSettings>().app.unload_after_secs);
        let chosen = cx.global::<AppSettings>().model.as_str();
        let engine = match &status.engine {
            EngineStatus::NotLoaded => "Not loaded.".to_string(),
            EngineStatus::Missing => match status.downloads.get(chosen) {
                Some(download) => download_words(download),
                None => {
                    let size = easyvox_core::models::find(chosen).map(|m| format_size(m.download_bytes));
                    format!("Not downloaded yet ({}).", size.unwrap_or_default())
                }
            },
            EngineStatus::Loading => "Loading… The first load after installing takes about 15 s.".into(),
            EngineStatus::Ready { load_ms } => {
                format!(
                    "Loaded in {:.1} s. Used for both the live preview and the final text.",
                    *load_ms as f32 / 1000.
                )
            }
            EngineStatus::Unloaded => {
                format!("Freed after {unload_after} without use. It loads again as you start your next dictation.")
            }
            EngineStatus::Failed { error } => format!("Couldn't load: {error}"),
        };
        let keep_loaded = format!(
            "Off: after {unload_after} without dictation, EasyVox frees the model's ~600 MB. Your next dictation \
             records at once while it loads again (about 0.15 s), so nothing you say is lost. On: it stays in \
             memory."
        );
        let right_option = status
            .right_option_error
            .clone()
            .unwrap_or_else(|| "Works alongside the shortcut. Needs Input Monitoring.".into());

        let config_path = status.config_path.clone();
        let config_note = match (&status.config_error, status.config_warnings.as_slice()) {
            (Some(e), _) => {
                format!("⚠ {e}. Changes made here apply until EasyVox quits, but aren't saved until the file is fixed.")
            }
            (None, []) => "Every setting is in this file, including advanced ones this window doesn't show \
                           (docs/settings.md lists them). Edits to it apply as soon as you save, and changes \
                           here are written to it."
                .into(),
            (None, warnings) => format!("⚠ Some of this file was ignored: {}", warnings.join(" · ")),
        };
        let config_row = SettingField::render(move |_, _, _| {
            let path = config_path.clone();
            h_flex().gap_2().child(div().text_sm().child(home_relative(&config_path))).child(
                Button::new("config-file").outline().small().label("Show in Finder").on_click(move |_, _, cx| {
                    // The folder always exists (the watcher makes it); the file only once written.
                    let target = if path.exists() { path.clone() } else { path.parent().unwrap_or(&path).into() };
                    cx.reveal_path(&target);
                }),
            )
        });

        SettingPage::new("General").default_open(true).group(
            SettingGroup::new()
                .title("Dictation")
                .item(SettingItem::new("Model", model_field).description(engine))
                .item(
                    SettingItem::new(
                        "Keep model loaded",
                        SettingField::switch(
                            |cx: &App| cx.global::<AppSettings>().keep_model_loaded,
                            |on: bool, cx: &mut App| {
                                controller::with(cx, |c, cx| c.set_keep_model_loaded(on, cx));
                            },
                        ),
                    )
                    .description(keep_loaded),
                )
                .item(
                    SettingItem::new("Push-to-talk shortcut", SettingField::render(move |_, _, _| recorder.clone()))
                        .description("Hold it and speak; let go to paste. Click, then press a new combination."),
                )
                .item(
                    SettingItem::new(
                        "Hold right ⌥ to talk",
                        SettingField::switch(
                            |cx: &App| cx.global::<AppSettings>().hold_right_option,
                            |on: bool, cx: &mut App| {
                                controller::with(cx, |c, cx| c.set_hold_right_option(on, cx));
                            },
                        ),
                    )
                    .description(right_option),
                )
                .item(SettingItem::new("Settings file", config_row).description(config_note)),
        )
    }

    fn permissions_page(&self, cx: &mut Context<Self>) -> SettingPage {
        let this = cx.entity().downgrade();
        let requesting = self.mic_request.is_some();
        let mic = easyvox_core::mic_permission_status();
        let accessibility = easyvox_core::accessibility_trusted();
        let input_monitoring = crate::platform::input_monitoring_granted();

        SettingPage::new("Permissions").group(
            SettingGroup::new()
                .title("macOS privacy")
                .item(
                    SettingItem::new(
                        "Microphone",
                        SettingField::render(move |_, _, _| {
                            let this = this.clone();
                            let (label, button) = match (mic, requesting) {
                                (_, true) => ("Waiting for your answer…", None),
                                (MicPermission::Authorized, _) => ("Allowed", None),
                                (MicPermission::NotDetermined, _) => ("Not asked yet", Some("Allow…")),
                                (MicPermission::Denied, _) => ("Denied", Some("Open System Settings…")),
                                (MicPermission::Restricted, _) => ("Restricted by this Mac's policy", None),
                                (MicPermission::Unknown, _) => ("Unknown", Some("Open System Settings…")),
                            };
                            status_row(
                                label,
                                button.map(|text| {
                                    Button::new("microphone").outline().small().label(text).on_click(move |_, _, cx| {
                                        if mic == MicPermission::NotDetermined {
                                            let _ = this.update(cx, |view, cx| view.request_mic(cx));
                                        } else {
                                            cx.open_url(PRIVACY_MICROPHONE);
                                        }
                                    })
                                }),
                            )
                        }),
                    )
                    .description("Needed for dictation. EasyVox listens only while you hold the shortcut."),
                )
                .item(
                    SettingItem::new(
                        "Accessibility",
                        SettingField::render(move |_, _, _| system_settings_row(accessibility, PRIVACY_ACCESSIBILITY)),
                    )
                    .description("Pastes the text at the cursor. Without it, the text is on the clipboard for ⌘V."),
                )
                .item(
                    SettingItem::new(
                        "Input Monitoring",
                        SettingField::render(move |_, _, _| {
                            system_settings_row(input_monitoring, PRIVACY_INPUT_MONITORING)
                        }),
                    )
                    .description("Only for holding right ⌥ to talk. Relaunch EasyVox after allowing it."),
                ),
        )
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pages = vec![self.general_page(cx), self.permissions_page(cx)];
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(Settings::new("easyvox-settings").pages(pages))
    }
}

/// A download's state in a few words: "Downloading… 120 MB of 539 MB".
fn download_words(status: &DownloadStatus) -> String {
    match status {
        DownloadStatus::Running(Progress::Verifying) => "Checking the copy already downloaded…".into(),
        DownloadStatus::Running(Progress::Downloading { done, total }) => {
            format!("Downloading… {} of {}", format_size(*done), format_size(*total))
        }
        DownloadStatus::Running(Progress::Reconnecting { attempt, max, done, total }) => format!(
            "Connection lost at {} of {}. Trying again ({attempt} of {max})…",
            format_size(*done),
            format_size(*total)
        ),
        DownloadStatus::Failed { error } => format!("Download failed: {error}"),
    }
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

fn status_row(label: &'static str, button: Option<Button>) -> impl IntoElement {
    h_flex().gap_2().child(div().text_sm().child(label)).when_some(button, |this, b| this.child(b))
}

fn system_settings_row(granted: bool, url: &'static str) -> impl IntoElement {
    let button = (!granted).then(|| {
        Button::new(url).outline().small().label("Open System Settings…").on_click(move |_, _, cx| cx.open_url(url))
    });
    status_row(if granted { "Allowed" } else { "Not allowed" }, button)
}

/// Click, then press a combination: it becomes the push-to-talk shortcut. The OS hot key is
/// suspended while recording, or Carbon would swallow the combination.
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
        let current = Keystroke::parse(&cx.global::<AppSettings>().shortcut).ok();
        let error = self.error.clone().or_else(|| cx.global::<AppStatus>().shortcut_error.clone().map(Into::into));
        let border = if self.recording { cx.theme().ring } else { cx.theme().border };
        v_flex()
            .items_end()
            .gap_1()
            .child(
                h_flex()
                    .id("shortcut-recorder")
                    .track_focus(&self.focus)
                    .min_w(px(140.))
                    .h(px(30.))
                    .px_2()
                    .justify_center()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, window, cx| this.start(window, cx)))
                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                        if this.recording {
                            cx.stop_propagation();
                            this.handle_keystroke(&ev.keystroke, cx);
                        }
                    }))
                    .map(|this| match (self.recording, current) {
                        (true, _) => {
                            this.child(div().text_sm().text_color(cx.theme().muted_foreground).child("Press shortcut…"))
                        }
                        (false, Some(ks)) => this.child(Kbd::new(ks)),
                        (false, None) => this.child(div().text_sm().child("Not set")),
                    }),
            )
            .when_some(error, |this, e| this.child(div().text_xs().text_color(cx.theme().danger).child(e)))
    }
}
