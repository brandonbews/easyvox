//! The app controller: owns the engine, shortcut, tray, overlay and windows, and runs the
//! push-to-talk state machine. Every OS callback (tray menu, Carbon hot key, event tap) and
//! every transcript event arrives on one `async_channel` that a GPUI task drains on the UI
//! thread (see `main.rs`), so all state changes happen here, on one thread.
//!
//! The speech model is freed after a stretch of no use (`unload_after_secs`, unless "Keep
//! model loaded" is on). The next key press starts recording at once, and the session reloads
//! the model meanwhile (`Engine::start_with`).
//!
//! Settings come from the settings file (`config.rs`). Changes made in the Settings window are
//! applied here and written to the file; outside edits to the file reach here through its
//! watcher and are applied the same way, without writing anything back, so the two can't feed
//! each other in a loop. Each dictation takes the pipeline's tuning in force when it starts.
//!
//! Models are downloaded here too (`download_model`), for onboarding and Settings alike. Each
//! download runs `easyvox_core::download` on its own thread and reports on the same channel;
//! its state is in `AppStatus::downloads`. Switching models goes through `use_model`, which
//! waits for a dictation to end, then frees the old model before loading the new one, so only
//! one is ever in memory (see `MODEL_SWAP`).
//!
//! The permission prompts start here too (`request_mic`, `prompt_accessibility`,
//! `request_input_monitoring`), so onboarding and Settings show the same state. Onboarding opens
//! at launch when a required step is missing (`onboard_if_needed`; see `onboarding.rs`).
//!
//! Hold right ⌥ to talk needs Input Monitoring. Turning it on asks for the permission (macOS's
//! prompt, which also puts EasyVox in System Settings' list), and the event tap is installed
//! once the permission is allowed: at once if it is, otherwise when a read of the permissions
//! finds it (`recheck_permissions`: the setup and Settings windows read them every second while
//! one is missing, and a click on the menu bar item or a press of the shortcut reads them too).
//! The tap is only ever made after the grant, so no relaunch is needed.

use crate::config::{ConfigFile, Reload};
use crate::history::{HistoryStore, HistoryView};
use crate::hotkey::HotkeyService;
use crate::onboarding::LaunchFacts;
use crate::onboarding_window::OnboardingView;
use crate::overlay::{Notice, Overlay};
use crate::permissions::Permissions;
use crate::platform::InputMonitoring;
use crate::settings::{Page, SettingsView};
use crate::state::{AppSettings, AppStatus, DownloadStatus, EngineStatus, RightOptionProblem};
use crate::{platform, tray};
use easyvox_core::download::{CancelToken, DownloadError, Progress};
use easyvox_core::models::ModelInfo;
use easyvox_core::{AudioSource, Engine, MicPermission, Session, TranscriptEvent};
use gpui_kit::component::Theme;
use gpui_kit::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

pub enum UiEvent {
    Menu(String),
    HotKey { id: u32, pressed: bool },
    RightOption { pressed: bool },
    // The menu bar item was clicked: its menu opens.
    TrayOpened,
    Transcript { seq: u64, event: TranscriptEvent },
    // The settings file may have changed (from its watcher, on a background thread).
    ConfigChanged,
    // From a model download's thread.
    Download { model: &'static str, update: DownloadUpdate },
}

pub enum DownloadUpdate {
    Progress(Progress),
    Done(Result<PathBuf, DownloadError>),
}

/// A running model download.
struct DownloadJob {
    cancel: CancelToken,
    /// Keeps App Nap from throttling the download. The Mac can still idle-sleep: the download
    /// picks up where it stopped after the wake.
    _no_app_nap: ActivityGuard,
}

enum Phase {
    Idle,
    Recording {
        seq: u64,
        session: Session,
        started: Instant,
        _no_app_nap: ActivityGuard,
    },
    /// Released; waiting for `Final`. The session is kept until then, because dropping it
    /// joins its worker thread.
    Finalizing {
        seq: u64,
        session: Session,
        released: Instant,
        _no_app_nap: ActivityGuard,
    },
}

/// How long to wait after the settings file changes before reading it, so the several events
/// one save produces (or an editor writing in steps) make one reload.
const CONFIG_SETTLE: Duration = Duration::from_millis(150);

/// How long to wait for an answer to the Microphone prompt.
const MIC_PROMPT_TIMEOUT: Duration = Duration::from_secs(120);

/// Held while a model is freed or loaded on the background executor, so two models are never
/// in memory together: a switch frees the old model before the new one loads, and a load that
/// a later switch overtook frees its model before the next load starts.
static MODEL_SWAP: Mutex<()> = Mutex::new(());

/// Frees an engine's model now, even if a clone of the engine is still around. Blocking.
fn free(engine: Engine) {
    let id = engine.model_info().id;
    if engine.unload() {
        log::info!("engine: freed {id}");
    }
}

pub struct ControllerGlobal(pub Entity<Controller>);
impl Global for ControllerGlobal {}

/// Updates the controller from anywhere that has `&mut App` (e.g. a Settings callback). Don't
/// call it while the controller is being updated: GPUI panics on re-entrant updates.
pub fn with<R>(cx: &mut App, f: impl FnOnce(&mut Controller, &mut Context<Controller>) -> R) -> Option<R> {
    let entity = cx.try_global::<ControllerGlobal>()?.0.clone();
    Some(entity.update(cx, f))
}

/// Drops a session on its own thread: `Session`'s `Drop` stops and joins its worker.
fn drop_off_main_thread(session: Session) {
    let _ = std::thread::Builder::new().name("session-drop".into()).spawn(move || drop(session));
}

pub struct Controller {
    pub engine: Option<Engine>,
    pub hotkeys: HotkeyService,
    /// Owns the menu bar icon: dropping it removes the icon. Read only by the self-test.
    #[cfg_attr(not(feature = "self-test"), allow(dead_code))]
    pub tray: Option<tray::Tray>,
    pub overlay: Overlay,
    pub history: Entity<HistoryStore>,
    tx: async_channel::Sender<UiEvent>,
    phase: Phase,
    seq: u64,
    settings_window: Option<AnyWindowHandle>,
    settings_view: Option<WeakEntity<SettingsView>>,
    history_window: Option<AnyWindowHandle>,
    onboarding_window: Option<AnyWindowHandle>,
    onboarding_view: Option<WeakEntity<OnboardingView>>,
    hide_task: Option<Task<()>>,
    load_task: Option<Task<()>>,
    /// Frees the model when it fires; replaced (cancelled) by every dictation.
    unload_task: Option<Task<()>>,
    /// The current session started with the model unloaded, so it's reloading it.
    reloading: bool,
    /// The settings file, and its watcher (`None` if it couldn't be watched).
    config: ConfigFile,
    pub config_watcher: Option<crate::config::Watcher>,
    /// Reads the file once its changes settle; replaced by every new change event.
    config_task: Option<Task<()>>,
    /// Outside edits to the settings file that were applied (read by the self-test).
    #[cfg_attr(not(feature = "self-test"), allow(dead_code))]
    pub config_reloads: u32,
    /// The right-⌥ event tap is installed (it stays for the rest of the process).
    right_option_tap: bool,
    /// The Input Monitoring prompt was asked for and hasn't been answered.
    asking_input_monitoring: bool,
    /// Microphone and Accessibility as last read, for the menu bar item. Read again on events (a
    /// press, a click on the item, a prompt answered, a change a window's watch saw), never on a
    /// timer of the controller's own.
    permissions: Permissions,
    /// The text was pasted: the menu bar item shows a check until then (`menubar_pasted_ms`),
    /// and the task puts it back.
    pasted_until: Option<Instant>,
    pasted_task: Option<Task<()>>,
    /// Model downloads running now, by catalog id.
    downloads: HashMap<&'static str, DownloadJob>,
    /// A model picked while a dictation was running: switched to once it ends.
    pending_model: Option<String>,
    /// Counts model loads, so a load can tell that a later one overtook it.
    load_generation: Arc<AtomicU64>,
    /// Push-to-talk replays this WAV instead of opening the microphone (debug and self-test).
    pub source_override: Option<PathBuf>,
    /// Simulate ⌘V after copying the final text. The self-test turns it off: it must never
    /// type into other apps.
    pub auto_paste: bool,
    /// Put the final text on the clipboard at all. The self-test's `--replay` turns it off: it
    /// may replay a private recording, whose words must not land on the clipboard.
    pub copy_final: bool,
}

impl Controller {
    pub fn new(
        hotkeys: HotkeyService,
        tray: Option<tray::Tray>,
        overlay: Overlay,
        history: Entity<HistoryStore>,
        config: ConfigFile,
        tx: async_channel::Sender<UiEvent>,
    ) -> Self {
        Self {
            engine: None,
            hotkeys,
            tray,
            overlay,
            history,
            tx,
            phase: Phase::Idle,
            seq: 0,
            settings_window: None,
            settings_view: None,
            history_window: None,
            onboarding_window: None,
            onboarding_view: None,
            hide_task: None,
            load_task: None,
            unload_task: None,
            reloading: false,
            config,
            config_watcher: None,
            config_task: None,
            config_reloads: 0,
            right_option_tap: false,
            asking_input_monitoring: false,
            permissions: Permissions::read(),
            pasted_until: None,
            pasted_task: None,
            downloads: HashMap::new(),
            pending_model: None,
            load_generation: Arc::default(),
            source_override: None,
            auto_paste: true,
            copy_final: true,
        }
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.phase, Phase::Idle)
    }

    pub fn handle(&mut self, event: UiEvent, cx: &mut Context<Self>) {
        match event {
            UiEvent::Menu(id) => self.on_menu(&id, cx),
            UiEvent::HotKey { id, pressed } => {
                if Some(id) == self.hotkeys.current_id() {
                    self.on_push_to_talk(pressed, "shortcut", cx);
                }
            }
            UiEvent::RightOption { pressed } => {
                // Releases always pass, so switching the setting off mid-hold can't leave a
                // session recording.
                if !pressed || cx.global::<AppSettings>().hold_right_option {
                    self.on_push_to_talk(pressed, "right ⌥", cx);
                }
            }
            UiEvent::Transcript { seq, event } => self.on_transcript(seq, event, cx),
            UiEvent::ConfigChanged => {
                self.config_task = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(CONFIG_SETTLE).await;
                    let _ = this.update(cx, |c, cx| c.reload_settings(cx));
                }));
            }
            UiEvent::Download { model, update } => self.on_download(model, update, cx),
            UiEvent::TrayOpened => self.recheck_permissions(cx),
        }
    }

    /// What the menu bar item shows, from the dictation, the app's state and the permissions as
    /// last read (`DESIGN.md`, "Menu bar").
    fn tray_status(&self, cx: &App) -> tray::Status {
        let (settings, status) = (cx.global::<AppSettings>(), cx.global::<AppStatus>());
        tray::Status::of(&tray::Facts {
            activity: match self.phase {
                Phase::Idle => tray::Activity::Idle,
                Phase::Recording { .. } => tray::Activity::Recording,
                Phase::Finalizing { .. } => tray::Activity::Finishing,
            },
            just_pasted: self.pasted_until.is_some_and(|until| Instant::now() < until),
            mic: self.permissions.mic,
            accessibility: self.permissions.accessibility,
            engine: &status.engine,
            downloading: self.downloads.contains_key(settings.model.as_str()),
            shortcut: crate::hotkey::label(&settings.shortcut),
            // Only once it works: until Input Monitoring is allowed, right ⌥ does nothing.
            hold_right_option: settings.hold_right_option && self.right_option_tap,
            shortcut_failed: status.shortcut_error.is_some(),
        })
    }

    /// Shows the menu bar item's status as it is now. Changes nothing that's already shown, so
    /// it's cheap to call on any change (`main.rs` calls it whenever `AppStatus` or `AppSettings`
    /// changes).
    pub fn refresh_tray(&mut self, cx: &mut Context<Self>) {
        let status = self.tray_status(cx);
        if let Some(tray) = &mut self.tray {
            tray.show(status);
        }
    }

    /// Reads the permissions again, then [`refresh_tray`](Self::refresh_tray), and installs the
    /// right-⌥ tap if it was waiting for Input Monitoring and that's allowed now. A revoke or a
    /// grant shows at the next event that calls this: a click on the item, a prompt answered, a
    /// change the setup or Settings window saw (they watch while open and one is missing).
    pub fn recheck_permissions(&mut self, cx: &mut Context<Self>) {
        self.permissions = Permissions::read();
        self.refresh_tray(cx);
        self.apply_right_option(cx);
    }

    fn on_menu(&mut self, id: &str, cx: &mut Context<Self>) {
        match id {
            #[cfg(debug_assertions)]
            tray::MENU_TEST_DICTATION => {
                // The session ends by itself at the end of the clip.
                let clip = crate::testdata_dir().join("punct_samantha.wav");
                self.start_session(AudioSource::WavReplay(clip), "test dictation", cx);
            }
            tray::MENU_SETTINGS => self.open_settings(cx),
            tray::MENU_HISTORY => self.open_history(cx),
            tray::MENU_ABOUT => self.open_settings_at(Page::About, cx),
            tray::MENU_QUIT => {
                log::info!("quitting");
                // Best effort: a part file the exit leaves behind goes at the next launch.
                for job in self.downloads.values() {
                    job.cancel.cancel();
                }
                cx.quit();
            }
            fix if fix.starts_with(tray::MENU_FIX) => match self.tray.as_ref().and_then(|t| t.fix(fix)) {
                Some(tray::Fix::SystemSettings(url)) => cx.open_url(url),
                Some(tray::Fix::Setup) => self.open_onboarding(cx),
                Some(tray::Fix::Settings(page)) => self.open_settings_at(page, cx),
                None => log::warn!("menu bar: no problem for {fix:?}"),
            },
            _ => log::warn!("unknown menu item {id:?}"),
        }
    }

    /// Press: start a session and show the overlay. Release: stop it; `Final` follows.
    /// Every press reads the permissions again, for the menu bar item, which shows each press and
    /// release; and, once the session has started, Input Monitoring if right ⌥ waits for it.
    pub fn on_push_to_talk(&mut self, pressed: bool, source: &str, cx: &mut Context<Self>) {
        if pressed {
            self.permissions = Permissions::read();
        }
        self.push_to_talk(pressed, source, cx);
        self.refresh_tray(cx);
        if pressed {
            self.apply_right_option(cx);
        }
    }

    fn push_to_talk(&mut self, pressed: bool, source: &str, cx: &mut Context<Self>) {
        if pressed {
            let audio = match &self.source_override {
                Some(wav) => AudioSource::WavReplay(wav.clone()),
                None => {
                    // Never open the mic while permission is undetermined: CoreAudio hangs.
                    let permission = easyvox_core::mic_permission_status();
                    if permission != MicPermission::Authorized {
                        log::warn!("push-to-talk: Microphone access is {permission:?}");
                        let notice = match permission {
                            MicPermission::NotDetermined => Notice::microphone_not_asked(),
                            _ => Notice::microphone_off(),
                        };
                        self.show_notice(notice, cx);
                        return;
                    }
                    AudioSource::Microphone
                }
            };
            self.start_session(audio, source, cx);
        } else if matches!(self.phase, Phase::Recording { .. }) {
            let Phase::Recording { seq, session, started, _no_app_nap } =
                std::mem::replace(&mut self.phase, Phase::Idle)
            else {
                unreachable!()
            };
            session.stop(); // returns at once; Final follows on the channel
            log::info!("session {seq}: released after {:.2} s", started.elapsed().as_secs_f32());
            self.phase = Phase::Finalizing { seq, session, released: Instant::now(), _no_app_nap };
            self.overlay.view.update(cx, |v, cx| v.set_finishing(cx));
        }
    }

    pub fn start_session(&mut self, audio: AudioSource, source: &str, cx: &mut Context<Self>) {
        if !self.is_idle() {
            return; // key repeat, or a press while the last session finishes
        }
        let Some(engine) = self.engine.clone() else {
            let downloading = self.downloads.contains_key(cx.global::<AppSettings>().model.as_str());
            let notice = match &cx.global::<AppStatus>().engine {
                EngineStatus::Failed { .. } => Notice::model_failed(),
                EngineStatus::Missing if downloading => Notice::downloading(),
                EngineStatus::Missing => Notice::no_model(),
                _ => Notice::loading(),
            };
            self.show_notice(notice, cx);
            return;
        };
        // Recording starts now either way; an unloaded model reloads while it records.
        let reloading = !engine.is_loaded();
        // The session keeps the tuning in force now; a change applies from the next one.
        let tuning = cx.global::<AppSettings>().tuning;
        let (session, events) = match engine.start_with(audio, tuning) {
            Ok(v) => v,
            Err(e) => {
                log::error!("couldn't start a session: {e:#}");
                // The core's words when the Mac has no input device (`audio::open_mic`).
                let notice = if format!("{e:#}").contains("no default input device") {
                    Notice::no_microphone()
                } else {
                    Notice::cant_start(&e.to_string())
                };
                self.show_notice(notice, cx);
                return;
            }
        };
        self.seq += 1;
        let seq = self.seq;
        // The core sends events on a std channel; a small thread forwards them into GPUI's.
        let tx = self.tx.clone();
        let forwarder = std::thread::Builder::new().name(format!("transcript-{seq}")).spawn(move || {
            for event in events {
                if tx.send_blocking(UiEvent::Transcript { seq, event }).is_err() {
                    break;
                }
            }
        });
        if let Err(e) = forwarder {
            log::error!("couldn't start the transcript thread: {e}");
            drop_off_main_thread(session);
            return;
        }
        // Keep timers precise while dictating.
        let no_app_nap = cx.background_executor().prevent_app_nap("EasyVox dictation");
        self.phase = Phase::Recording { seq, session, started: Instant::now(), _no_app_nap: no_app_nap };
        self.hide_task = None;
        self.unload_task = None;
        self.reloading = reloading;
        if reloading {
            cx.global_mut::<AppStatus>().engine = EngineStatus::Loading;
            cx.refresh_windows();
        }
        // At once, without waiting for the model; a press during the last close turns it back.
        self.overlay.view.update(cx, |v, cx| v.reset(cx));
        self.overlay.show(cx);
        (self.pasted_until, self.pasted_task) = (None, None);
        self.refresh_tray(cx);
        log::info!("session {seq}: started ({source}){}", if reloading { ", reloading the model" } else { "" });
    }

    fn on_transcript(&mut self, seq: u64, event: TranscriptEvent, cx: &mut Context<Self>) {
        let current = match &self.phase {
            Phase::Recording { seq, .. } | Phase::Finalizing { seq, .. } => *seq,
            Phase::Idle => return,
        };
        if seq != current {
            return; // a stale session
        }
        match event {
            TranscriptEvent::Partial { confirmed, provisional } => {
                self.sync_engine_status(cx); // a preview means the model is back
                self.overlay.view.update(cx, |v, cx| v.set_partial(confirmed, provisional, cx));
            }
            TranscriptEvent::Final { text, audio_secs, finalize_ms } => {
                // A WAV replay can end (and send Final) while the key is still held.
                let since_release = match &self.phase {
                    Phase::Finalizing { released, .. } => format!("{} ms", released.elapsed().as_millis()),
                    _ => "n/a".into(),
                };
                if let Phase::Recording { session, .. } | Phase::Finalizing { session, .. } =
                    std::mem::replace(&mut self.phase, Phase::Idle)
                {
                    drop_off_main_thread(session);
                }
                log::info!(
                    "session {seq}: final, {} chars from {audio_secs:.1} s of audio (final pass {finalize_ms} ms, \
                     release → final {since_release})",
                    text.chars().count()
                );
                self.sync_engine_status(cx);
                self.schedule_unload(cx);
                self.deliver(text, audio_secs, cx);
                self.refresh_tray(cx);
                if let Some(model) = self.pending_model.take() {
                    self.use_model(&model, cx);
                }
            }
            TranscriptEvent::Error(e) => {
                // The core still sends a Final after an error.
                log::error!("session {seq}: {e}");
                self.overlay.view.update(cx, |v, cx| v.set_note(format!("  ⚠ {e}"), cx));
            }
            // About 8 a second while you talk, none in silence: the overlay redraws only if a
            // bar of its meter moves.
            TranscriptEvent::Level(level) => self.overlay.view.update(cx, |v, cx| v.set_level(level, cx)),
            TranscriptEvent::NoInput(cant_hear) => {
                if cant_hear {
                    log::warn!("session {seq}: can't hear the input (no audio, or only digital silence)");
                } else {
                    log::info!("session {seq}: hearing the input again");
                }
                self.overlay.view.update(cx, |v, cx| v.set_cant_hear(cant_hear, cx));
            }
        }
    }

    /// The final text: clipboard (and ⌘V), history, then hide the pill.
    fn deliver(&mut self, text: String, audio_secs: f32, cx: &mut Context<Self>) {
        if text.trim().is_empty() {
            // Nothing heard: leave the clipboard and history alone.
            let shortcut = crate::hotkey::label(&cx.global::<AppSettings>().shortcut);
            self.show_notice(Notice::nothing_heard(&shortcut), cx);
            return;
        }
        self.overlay.view.update(cx, |v, cx| v.set_final(text.clone(), cx));
        // The overlay never takes focus, so the app the user was typing in gets the ⌘V.
        let result = if !self.copy_final {
            Ok(())
        } else if self.auto_paste {
            easyvox_core::paste_text(&text)
        } else {
            easyvox_core::copy_to_clipboard(&text).map(|_| ())
        };
        let model = self.engine.as_ref().map(|e| e.model_info().id).unwrap_or_default();
        self.history.update(cx, |h, cx| h.push(text, audio_secs, model, cx));
        match result {
            Ok(()) => {
                let after = cx.global::<AppSettings>().app.overlay_hide_after_final_ms;
                self.schedule_hide(Duration::from_millis(after.into()), cx);
                self.show_pasted(cx);
            }
            Err(e) => {
                let e = e.to_string();
                log::warn!("paste: {e}");
                let notice = if e.contains("Accessibility") {
                    // Read again, so the menu bar item names the problem too.
                    self.permissions = Permissions::read();
                    Notice::accessibility_off()
                } else {
                    Notice::couldnt_paste()
                };
                self.show_notice(notice, cx);
            }
        }
    }

    /// The menu bar item's check, for `menubar_pasted_ms`; then it goes back to how it was.
    fn show_pasted(&mut self, cx: &mut Context<Self>) {
        let after = Duration::from_millis(cx.global::<AppSettings>().app.menubar_pasted_ms.into());
        if after.is_zero() {
            return;
        }
        self.pasted_until = Some(Instant::now() + after);
        self.pasted_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(after).await;
            let _ = this.update(cx, |c, cx| {
                c.pasted_until = None;
                c.refresh_tray(cx);
            });
        }));
    }

    fn show_notice(&mut self, notice: Notice, cx: &mut Context<Self>) {
        log::info!("notice: {}. {}", notice.title, notice.detail);
        self.overlay.view.update(cx, |v, cx| v.set_notice(notice, cx));
        self.overlay.show(cx);
        let after = cx.global::<AppSettings>().app.overlay_hide_after_notice_ms;
        self.schedule_hide(Duration::from_millis(after.into()), cx);
    }

    /// Hides the pill after `after`, unless a dictation started meanwhile. The close's fade
    /// starts then and runs on its own: the paste is done, and the next press doesn't wait.
    fn schedule_hide(&mut self, after: Duration, cx: &mut Context<Self>) {
        self.hide_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(after).await;
            let _ = this.update(cx, |c, cx| {
                if c.is_idle() {
                    c.overlay.hide(cx);
                }
            });
        }));
    }

    /// After a session that reloaded the model (or raced an unload), shows whether it's in
    /// memory now.
    fn sync_engine_status(&mut self, cx: &mut Context<Self>) {
        let unloaded = matches!(cx.global::<AppStatus>().engine, EngineStatus::Unloaded);
        let Some(engine) = &self.engine else { return };
        if !self.reloading && !unloaded {
            return;
        }
        self.reloading = false;
        cx.global_mut::<AppStatus>().engine =
            if engine.is_loaded() { EngineStatus::Ready { load_ms: engine.load_ms() } } else { EngineStatus::Unloaded };
        cx.refresh_windows();
    }

    /// Frees the model after `unload_after_secs` without a dictation, unless "Keep model
    /// loaded" is on. Replaces any earlier timer; a dictation cancels it and starts it again
    /// when it ends.
    pub fn schedule_unload(&mut self, cx: &mut Context<Self>) {
        let settings = cx.global::<AppSettings>();
        if settings.keep_model_loaded || self.engine.is_none() || !self.is_idle() {
            self.unload_task = None;
            return;
        }
        let after = Duration::from_secs(settings.app.unload_after_secs);
        self.unload_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(after).await;
            let _ = this.update(cx, |c, cx| c.unload_engine(after, cx));
        }));
    }

    /// Frees the model, on the background executor: freeing ~600 MB blocks for a moment (25–35 ms).
    fn unload_engine(&mut self, unused_for: Duration, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else { return };
        if !self.is_idle() || !engine.is_loaded() {
            return; // a dictation starts the timer again when it ends
        }
        log::info!("engine: unloading {} after {} s without use", engine.model_info().id, unused_for.as_secs());
        cx.global_mut::<AppStatus>().engine = EngineStatus::Unloaded;
        cx.refresh_windows();
        cx.background_spawn(async move {
            let started = Instant::now();
            engine.unload();
            log::info!("engine: unloaded in {} ms", started.elapsed().as_millis());
        })
        .detach();
    }

    /// "Keep model loaded" (from Settings): on, stops the unload timer and loads the model
    /// back if it was freed; off, starts the timer.
    pub fn set_keep_model_loaded(&mut self, on: bool, cx: &mut Context<Self>) {
        cx.global_mut::<AppSettings>().keep_model_loaded = on;
        self.apply_keep_model_loaded(cx);
        self.save_settings(cx);
    }

    /// Acts on "Keep model loaded" and `unload_after_secs` as they are now.
    fn apply_keep_model_loaded(&mut self, cx: &mut Context<Self>) {
        let unloaded = self.engine.as_ref().is_some_and(|e| !e.is_loaded());
        if cx.global::<AppSettings>().keep_model_loaded && unloaded && self.is_idle() {
            let model = cx.global::<AppSettings>().model.clone();
            self.load_engine(&model, cx);
        } else {
            self.schedule_unload(cx);
        }
        cx.refresh_windows();
    }

    /// Picks the speech model (from Settings).
    pub fn select_model(&mut self, model_id: &str, cx: &mut Context<Self>) {
        self.use_model(model_id, cx);
        self.save_settings(cx);
    }

    /// Makes `model_id` the speech model: the one way to switch. During a dictation the switch
    /// waits for it to end, so it applies from the next one. The old model is freed before the
    /// new one loads, so only one is ever in memory. A model that isn't downloaded leaves no
    /// engine, with [`EngineStatus::Missing`]. Doesn't write the settings file.
    pub fn use_model(&mut self, model_id: &str, cx: &mut Context<Self>) {
        cx.global_mut::<AppSettings>().model = model_id.to_string();
        if !self.is_idle() {
            log::info!("engine: switching to {model_id} once this dictation ends");
            self.pending_model = Some(model_id.to_string());
            return;
        }
        self.pending_model = None;
        if self.engine.as_ref().is_some_and(|e| e.model_info().id == model_id) {
            return; // already in use
        }
        self.unload_task = None;
        let old = self.engine.take();
        self.load_engine_replacing(model_id, old, cx);
    }

    /// Loads (or reloads) a speech model on the background executor. If the model isn't
    /// downloaded, there's no engine and the status says so.
    pub fn load_engine(&mut self, model_id: &str, cx: &mut Context<Self>) {
        self.load_engine_replacing(model_id, None, cx);
    }

    /// [`load_engine`](Self::load_engine), freeing `old` first, under [`MODEL_SWAP`]. An engine
    /// still in `self.engine` is replaced once the new one is ready, and dropped off this
    /// thread, since freeing a model can block.
    fn load_engine_replacing(&mut self, model_id: &str, old: Option<Engine>, cx: &mut Context<Self>) {
        cx.global_mut::<AppSettings>().model = model_id.to_string();
        let generation = self.load_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let latest = self.load_generation.clone();
        let models_dir = cx.global::<AppStatus>().models_dir.clone();
        if let Ok(info) = easyvox_core::models::find(model_id)
            && !info.is_available(&models_dir)
        {
            log::warn!("engine: {model_id} isn't downloaded ({})", info.resolve(&models_dir).display());
            self.load_task = None;
            self.unload_task = None;
            let replaced = self.engine.take();
            cx.background_spawn(async move {
                let _swap = MODEL_SWAP.lock().unwrap_or_else(PoisonError::into_inner);
                old.into_iter().chain(replaced).for_each(free);
            })
            .detach();
            cx.global_mut::<AppStatus>().engine = EngineStatus::Missing;
            cx.refresh_windows();
            return;
        }
        // No tuning here: each session takes the one in force when it starts.
        let mut config = easyvox_core::EngineConfig::recommended(models_dir);
        config.model_id = model_id.to_string();
        cx.global_mut::<AppStatus>().engine = EngineStatus::Loading;
        cx.refresh_windows();
        log::info!("engine: loading {model_id}");
        let model = model_id.to_string();
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let started = Instant::now();
            let result = cx
                .background_spawn(async move {
                    let _swap = MODEL_SWAP.lock().unwrap_or_else(PoisonError::into_inner);
                    old.into_iter().for_each(free);
                    let overtaken = || latest.load(Ordering::SeqCst) != generation;
                    if overtaken() {
                        return None;
                    }
                    let result = easyvox_core::Engine::load(config);
                    match result {
                        Ok(engine) if overtaken() => {
                            free(engine); // a later switch wants another model
                            None
                        }
                        result => Some(result),
                    }
                })
                .await;
            let Some(result) = result else {
                log::info!("engine: {model} was replaced before it finished loading");
                return;
            };
            let wall_ms = started.elapsed().as_millis();
            let _ = this.update(cx, |c, cx| {
                let status = match result {
                    Ok(engine) => {
                        log::info!("engine: {model} loaded in {wall_ms} ms (core load {} ms)", engine.load_ms());
                        let load_ms = engine.load_ms();
                        if let Some(old) = c.engine.replace(engine) {
                            cx.background_spawn(async move { drop(old) }).detach();
                        }
                        c.schedule_unload(cx);
                        EngineStatus::Ready { load_ms }
                    }
                    Err(e) => {
                        log::error!("engine: couldn't load {model}: {e:#}");
                        EngineStatus::Failed { error: format!("{e:#}") }
                    }
                };
                cx.global_mut::<AppStatus>().engine = status;
                cx.refresh_windows();
            });
        }));
    }

    /// Downloads a catalog model into the models folder, on its own thread; progress and the
    /// result arrive as [`UiEvent::Download`]. Does nothing if it's already downloading.
    pub fn download_model(&mut self, model_id: &str, cx: &mut Context<Self>) {
        match easyvox_core::models::find(model_id) {
            Ok(info) => {
                let models_dir = cx.global::<AppStatus>().models_dir.clone();
                self.start_download(info, models_dir, cx);
            }
            Err(e) => log::error!("download: {e:#}"),
        }
    }

    /// [`download_model`](Self::download_model) for any entry, into any folder (the self-test
    /// uses its own).
    pub fn start_download(&mut self, info: &'static ModelInfo, models_dir: PathBuf, cx: &mut Context<Self>) {
        let id = info.id;
        if self.downloads.contains_key(id) {
            return;
        }
        let cancel = CancelToken::new();
        let tx = self.tx.clone();
        let token = cancel.clone();
        let thread = std::thread::Builder::new().name(format!("download-{id}")).spawn(move || {
            let send = |update| {
                let _ = tx.send_blocking(UiEvent::Download { model: id, update });
            };
            let result = easyvox_core::download::download(info, &models_dir, &token, |p| {
                send(DownloadUpdate::Progress(p));
            });
            send(DownloadUpdate::Done(result));
        });
        let status = match thread {
            Ok(_) => {
                log::info!("download: {id} started");
                let no_app_nap = cx.background_executor().prevent_app_nap("EasyVox model download");
                self.downloads.insert(id, DownloadJob { cancel, _no_app_nap: no_app_nap });
                DownloadStatus::Running(Progress::Downloading { done: 0, total: info.download_bytes })
            }
            Err(e) => {
                log::error!("download: couldn't start a thread for {id}: {e}");
                DownloadStatus::Failed(DownloadError::Network(format!("couldn't start the download: {e}")))
            }
        };
        cx.global_mut::<AppStatus>().downloads.insert(id, status);
        cx.refresh_windows();
    }

    /// Stops a running download. Its `Done(Err(Cancelled))` follows and clears its status;
    /// nothing is left on disk.
    pub fn cancel_download(&mut self, model_id: &str) {
        if let Some(job) = self.downloads.get(model_id) {
            log::info!("download: cancelling {model_id}");
            job.cancel.cancel();
        }
    }

    /// Whether `model_id` is downloading now.
    #[cfg_attr(not(feature = "self-test"), allow(dead_code))]
    pub fn is_downloading(&self, model_id: &str) -> bool {
        self.downloads.contains_key(model_id)
    }

    /// Deletes a downloaded model and its license notice. Refuses the model in use (or picked)
    /// and one that's downloading.
    pub fn delete_model(&mut self, model_id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let info = easyvox_core::models::find(model_id).map_err(|e| e.to_string())?;
        let models_dir = cx.global::<AppStatus>().models_dir.clone();
        self.delete_model_at(info, &models_dir, cx)
    }

    /// [`delete_model`](Self::delete_model) for any entry, in any folder (the self-test uses
    /// its own, never the real models folder).
    pub fn delete_model_at(
        &mut self,
        info: &'static ModelInfo,
        models_dir: &Path,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let in_use = self.engine.as_ref().is_some_and(|e| e.model_info().id == info.id);
        if in_use || cx.global::<AppSettings>().model == info.id || self.pending_model.as_deref() == Some(info.id) {
            return Err("This is the model in use. Use another one first.".into());
        }
        if self.downloads.contains_key(info.id) {
            return Err("This model is downloading. Cancel the download first.".into());
        }
        easyvox_core::download::delete(info, models_dir).map_err(|e| format!("Couldn't delete the model: {e}"))?;
        log::info!("download: deleted {} from {}", info.id, models_dir.display());
        cx.global_mut::<AppStatus>().downloads.remove(info.id);
        cx.refresh_windows();
        Ok(())
    }

    fn on_download(&mut self, model: &'static str, update: DownloadUpdate, cx: &mut Context<Self>) {
        match update {
            DownloadUpdate::Progress(p) => {
                cx.global_mut::<AppStatus>().downloads.insert(model, DownloadStatus::Running(p));
            }
            DownloadUpdate::Done(result) => {
                self.downloads.remove(model); // and with it the App Nap guard
                let downloads = &mut cx.global_mut::<AppStatus>().downloads;
                match result {
                    Ok(path) => {
                        log::info!("download: {model} ready at {}", path.display());
                        downloads.remove(model);
                        // The chosen model just arrived (first launch, or it was missing): load it.
                        if cx.global::<AppSettings>().model == model && self.engine.is_none() {
                            self.use_model(model, cx);
                        }
                    }
                    Err(DownloadError::Cancelled) => {
                        log::info!("download: {model} cancelled");
                        downloads.remove(model);
                    }
                    Err(e) => {
                        log::error!("download: {model}: {e} ({e:?})");
                        downloads.insert(model, DownloadStatus::Failed(e));
                    }
                }
            }
        }
        cx.refresh_windows();
    }

    /// Changes the push-to-talk shortcut (from the Settings recorder).
    pub fn rebind(&mut self, keystroke: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let result = self.bind_shortcut(keystroke, cx);
        if result.is_ok() {
            self.save_settings(cx);
        }
        result
    }

    /// Registers `keystroke` as the push-to-talk shortcut. If it can't be, the old one stays.
    fn bind_shortcut(&mut self, keystroke: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let result = self.hotkeys.bind(keystroke).map_err(|e| e.to_string());
        match &result {
            Ok(()) => {
                log::info!("hotkey registered: {keystroke}");
                cx.global_mut::<AppSettings>().shortcut = keystroke.to_string();
                cx.global_mut::<AppStatus>().shortcut_error = None;
            }
            Err(e) => log::warn!("hotkey: {e}"),
        }
        cx.refresh_windows();
        result
    }

    /// Turns "hold right ⌥ to talk" on or off (from Settings). On, without Input Monitoring:
    /// asks for it if it was never asked (macOS's prompt, which puts EasyVox in the list), and
    /// the key works once it's allowed; meanwhile Settings says it's waiting. If the event tap
    /// can't be installed for another reason, it stays off and Settings shows why.
    pub fn set_hold_right_option(&mut self, on: bool, cx: &mut Context<Self>) {
        cx.global_mut::<AppSettings>().hold_right_option = on;
        if on && !self.right_option_tap && platform::input_monitoring() == InputMonitoring::NotAsked {
            self.request_input_monitoring(cx);
        }
        if !self.apply_right_option(cx) {
            cx.global_mut::<AppSettings>().hold_right_option = false;
            cx.refresh_windows();
            return;
        }
        self.save_settings(cx);
    }

    /// Puts "hold right ⌥ to talk" in force as the settings say: installs the event tap the
    /// first time it's on with Input Monitoring allowed, and otherwise records why it can't work
    /// yet in `AppStatus` (nothing, while it's off). Reads Input Monitoring only while the tap
    /// waits for it. False if the tap couldn't be installed for another reason.
    pub fn apply_right_option(&mut self, cx: &mut Context<Self>) -> bool {
        let had_tap = self.right_option_tap;
        let problem = if !cx.global::<AppSettings>().hold_right_option || self.right_option_tap {
            None
        } else if platform::input_monitoring() != InputMonitoring::Allowed {
            Some(RightOptionProblem::NeedsInputMonitoring)
        } else {
            let tx = self.tx.clone();
            let result = platform::modifier_tap::install(move |pressed| {
                let _ = tx.send_blocking(UiEvent::RightOption { pressed });
            });
            match result {
                Ok(()) => {
                    log::info!("right ⌥ tap installed");
                    self.right_option_tap = true;
                    None
                }
                Err(e) => {
                    log::warn!("right ⌥ tap: {e:#}");
                    Some(RightOptionProblem::Failed(e.to_string()))
                }
            }
        };
        let failed = matches!(problem, Some(RightOptionProblem::Failed(_)));
        if cx.global::<AppStatus>().right_option_problem != problem {
            if problem == Some(RightOptionProblem::NeedsInputMonitoring) {
                log::info!("right ⌥: waiting for Input Monitoring");
            }
            cx.global_mut::<AppStatus>().right_option_problem = problem;
            cx.refresh_windows();
        }
        if self.right_option_tap != had_tap {
            self.refresh_tray(cx); // the menu's "Hold … or right ⌥ to talk" waits for the tap
        }
        !failed
    }

    /// Writes what the Settings window changed to the settings file (see
    /// `ConfigFile::save`). If the file has an error, the change still applies until EasyVox
    /// quits, the file is left alone, and Settings says why.
    fn save_settings(&mut self, cx: &mut Context<Self>) {
        let settings = cx.global::<AppSettings>().clone();
        match self.config.save(&settings) {
            Ok(true) => {
                log::info!("settings file: saved {}", self.config.path().display());
                cx.global_mut::<AppStatus>().config_error = None;
            }
            Ok(false) => {}
            Err(e) => {
                log::error!("settings file: {e}");
                cx.global_mut::<AppStatus>().config_error = Some(e);
            }
        }
        cx.refresh_windows();
    }

    /// Reads the settings file after its watcher saw a change, and applies what changed. A
    /// file that isn't valid TOML leaves the settings in force as they are. Never writes the
    /// file, so the app's own saves (which the watcher reports too) can't cause a loop: their
    /// text is what `ConfigFile` last wrote, so it reports them unchanged.
    fn reload_settings(&mut self, cx: &mut Context<Self>) {
        match self.config.reload() {
            Reload::Unchanged => log::debug!("settings file: unchanged"),
            Reload::Invalid(e) => {
                log::error!("settings file: {e}; keeping the settings in force");
                cx.global_mut::<AppStatus>().config_error = Some(e);
            }
            Reload::Changed(loaded) => {
                for warning in &loaded.warnings {
                    log::warn!("settings file: {warning}");
                }
                log::info!("settings file: applying changes from {}", self.config.path().display());
                let status = cx.global_mut::<AppStatus>();
                status.config_error = None;
                status.config_warnings = loaded.warnings;
                self.config_reloads += 1;
                self.apply_settings(loaded.settings, cx);
            }
        }
        cx.refresh_windows();
    }

    /// Puts `new` in force, acting on whatever differs from the settings now. The pipeline's
    /// tuning needs nothing more: the next dictation takes it. Neither do the overlay's
    /// numbers, which it reads as it draws, opens and hides.
    fn apply_settings(&mut self, new: AppSettings, cx: &mut Context<Self>) {
        let old = cx.global::<AppSettings>().clone();
        {
            let s = cx.global_mut::<AppSettings>();
            s.keep_model_loaded = new.keep_model_loaded;
            s.app = new.app;
            s.tuning = new.tuning;
            s.hold_right_option = new.hold_right_option;
        }
        if new.shortcut != old.shortcut
            && let Err(e) = self.bind_shortcut(&new.shortcut, cx)
        {
            // The old shortcut still works. Keep the file's choice on show, with the reason it
            // isn't in force, so a later save from the window doesn't write the old one back.
            cx.global_mut::<AppSettings>().shortcut = new.shortcut.clone();
            cx.global_mut::<AppStatus>().shortcut_error = Some(e);
        }
        if new.hold_right_option != old.hold_right_option {
            self.apply_right_option(cx);
        }
        if new.model != old.model {
            self.use_model(&new.model, cx);
        } else if new.keep_model_loaded != old.keep_model_loaded
            || new.app.unload_after_secs != old.app.unload_after_secs
        {
            self.apply_keep_model_loaded(cx);
        }
        self.overlay.view.update(cx, |_, cx| cx.notify());
        cx.refresh_windows();
    }

    /// Shows the macOS Microphone prompt, from onboarding or Settings. It blocks until answered,
    /// so it runs on the background executor; `AppStatus::mic_asking` is set meanwhile. Never
    /// opens the microphone.
    pub fn request_mic(&mut self, cx: &mut Context<Self>) {
        if cx.global::<AppStatus>().mic_asking {
            return;
        }
        log::info!("asking for Microphone access");
        cx.global_mut::<AppStatus>().mic_asking = true;
        cx.refresh_windows();
        cx.spawn(async move |this, cx| {
            let status = cx.background_spawn(async { easyvox_core::request_mic_permission(MIC_PROMPT_TIMEOUT) }).await;
            log::info!("Microphone access: {status:?}");
            let _ = this.update(cx, |c, cx| {
                cx.global_mut::<AppStatus>().mic_asking = false;
                cx.refresh_windows();
                c.recheck_permissions(cx);
            });
        })
        .detach();
    }

    /// Shows the macOS Accessibility prompt (from onboarding or Settings), which offers to open
    /// System Settings. Returns at once; the grant comes when the user turns the switch on, and
    /// the window's watch notices it.
    pub fn prompt_accessibility(&mut self, cx: &mut Context<Self>) {
        let trusted = easyvox_core::prompt_accessibility();
        log::info!("asked for Accessibility access (allowed already: {trusted})");
        cx.global_mut::<AppStatus>().accessibility_prompted = true;
        cx.refresh_windows();
        self.recheck_permissions(cx);
    }

    /// Asks for Input Monitoring (from Settings, or turning right ⌥ on): the first time, macOS
    /// shows its prompt, whose button opens System Settings at the list, and EasyVox appears in
    /// that list, switched off. The request blocks until TCC answers, so it runs on the background
    /// executor. The grant comes when the user turns the switch on; a window's watch, or the next
    /// click on the menu bar item, notices it.
    pub fn request_input_monitoring(&mut self, cx: &mut Context<Self>) {
        if self.asking_input_monitoring {
            return;
        }
        log::info!("asking for Input Monitoring");
        self.asking_input_monitoring = true;
        cx.spawn(async move |this, cx| {
            let allowed = cx.background_spawn(async { platform::request_input_monitoring() }).await;
            log::info!("Input Monitoring: allowed {allowed}, now {:?}", platform::input_monitoring());
            let _ = this.update(cx, |c, cx| {
                c.asking_input_monitoring = false;
                c.recheck_permissions(cx);
                cx.refresh_windows();
            });
        })
        .detach();
    }

    /// Loads the chosen model if it's downloaded and isn't loaded or loading: onboarding's
    /// "Get ready" step, also after a failed load. The first load under a new app identity
    /// compiles ggml's Metal shaders (~14 s), so the first dictation doesn't wait for them.
    pub fn warm_up(&mut self, cx: &mut Context<Self>) {
        let busy = matches!(cx.global::<AppStatus>().engine, EngineStatus::Loading | EngineStatus::Ready { .. });
        if !busy && self.is_idle() {
            self.reload_chosen_model(cx);
        }
    }

    /// Loads the chosen model from the models folder, freeing the one in memory first.
    fn reload_chosen_model(&mut self, cx: &mut Context<Self>) {
        let model = cx.global::<AppSettings>().model.clone();
        self.unload_task = None;
        let old = self.engine.take();
        self.load_engine_replacing(&model, old, cx);
    }

    /// Opens onboarding if a required step is missing (see `LaunchFacts::needed`). True if it
    /// did. `main` calls it at launch with what the system says; the self-test, with its own.
    pub fn onboard_if_needed(&mut self, facts: &LaunchFacts, cx: &mut Context<Self>) -> bool {
        log::info!("onboarding: {facts:?}, needed: {}", facts.needed());
        if facts.needed() {
            self.open_onboarding(cx);
        }
        facts.needed()
    }

    /// Opens the onboarding window, or brings it forward.
    pub fn open_onboarding(&mut self, cx: &mut Context<Self>) {
        if Self::bring_forward(self.onboarding_window, cx) {
            return;
        }
        let options = window_options("Set Up EasyVox", size(px(560.), px(540.)), cx);
        match gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| OnboardingView::new(window, cx))) {
            Ok((handle, view)) => {
                self.onboarding_window = Some(handle);
                self.onboarding_view = Some(view.downgrade());
                cx.activate(true);
            }
            Err(e) => log::error!("couldn't open onboarding: {e:#}"),
        }
    }

    /// The onboarding view, while its window is open.
    #[cfg(feature = "self-test")]
    pub fn onboarding_view(&self) -> Option<Entity<OnboardingView>> {
        self.onboarding_view.as_ref().and_then(WeakEntity::upgrade)
    }

    /// The onboarding window, if it was opened (it may have been closed since).
    #[cfg(feature = "self-test")]
    pub fn onboarding_window(&self) -> Option<AnyWindowHandle> {
        self.onboarding_window
    }

    /// Points the app at another models folder and loads the chosen model from there, freeing
    /// the one in memory first. The self-test uses it to run onboarding's download and warm-up
    /// in a temporary folder, then to come back.
    #[cfg(feature = "self-test")]
    pub fn set_models_dir(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        log::info!("models folder: {}", dir.display());
        cx.global_mut::<AppStatus>().models_dir = dir;
        self.reload_chosen_model(cx);
    }

    pub fn open_settings(&mut self, cx: &mut Context<Self>) {
        if Self::bring_forward(self.settings_window, cx) {
            return;
        }
        let options = window_options("EasyVox Settings", size(px(760.), px(540.)), cx);
        match gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| SettingsView::new(window, cx))) {
            Ok((handle, view)) => {
                self.settings_window = Some(handle);
                self.settings_view = Some(view.downgrade());
                cx.activate(true); // an accessory app has to bring itself forward
            }
            Err(e) => log::error!("couldn't open Settings: {e:#}"),
        }
    }

    /// Opens Settings (or brings it forward) on `page`: the tray's "About EasyVox" opens the
    /// About page.
    pub fn open_settings_at(&mut self, page: Page, cx: &mut Context<Self>) {
        self.open_settings(cx);
        if let Some(view) = self.settings_view.as_ref().and_then(WeakEntity::upgrade) {
            view.update(cx, |view, cx| view.show(page, cx));
        }
    }

    pub fn open_history(&mut self, cx: &mut Context<Self>) {
        if Self::bring_forward(self.history_window, cx) {
            return;
        }
        let options = window_options("EasyVox History", size(px(760.), px(520.)), cx);
        let store = self.history.clone();
        match gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| HistoryView::new(store, window, cx))) {
            Ok((handle, _)) => {
                self.history_window = Some(handle);
                cx.activate(true);
            }
            Err(e) => log::error!("couldn't open History: {e:#}"),
        }
    }

    /// The Settings and History windows, if they were opened.
    #[cfg(feature = "self-test")]
    pub fn windows(&self) -> [Option<AnyWindowHandle>; 2] {
        [self.settings_window, self.history_window]
    }

    /// The Settings view, while its window is open.
    #[cfg(feature = "self-test")]
    pub fn settings_view(&self) -> Option<Entity<SettingsView>> {
        self.settings_view.as_ref().and_then(WeakEntity::upgrade)
    }

    /// Activates an already open window. False if there's none (or it was closed).
    fn bring_forward(handle: Option<AnyWindowHandle>, cx: &mut Context<Self>) -> bool {
        let Some(handle) = handle else { return false };
        if handle.update(cx, |_, window, _| window.activate_window()).is_err() {
            return false;
        }
        cx.activate(true);
        true
    }
}

/// A standard window, centred. The sizes callers pass are the design hand-off's: setup 560 × 540,
/// Settings 760 × 540 and History 760 × 520 (`design/`, boards 10, 12 and 16).
fn window_options(title: &str, window_size: Size<Pixels>, cx: &mut App) -> WindowOptions {
    // The appearance may have changed while no window was open to notice.
    Theme::sync_system_appearance(None, cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(window_size, cx)),
        titlebar: Some(TitlebarOptions { title: Some(title.to_string().into()), ..Default::default() }),
        focus: true,
        show: true,
        kind: WindowKind::Normal,
        ..Default::default()
    }
}
