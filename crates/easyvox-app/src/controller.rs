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

use crate::config::{ConfigFile, Reload};
use crate::history::{HistoryStore, HistoryView};
use crate::hotkey::HotkeyService;
use crate::overlay::Overlay;
use crate::settings::SettingsView;
use crate::state::{AppSettings, AppStatus, EngineStatus};
use crate::{platform, tray};
use easyvox_core::{AudioSource, Engine, MicPermission, Session, TranscriptEvent};
use gpui_kit::component::Theme;
use gpui_kit::*;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub enum UiEvent {
    Menu(String),
    HotKey { id: u32, pressed: bool },
    RightOption { pressed: bool },
    Transcript { seq: u64, event: TranscriptEvent },
    // The settings file may have changed (from its watcher, on a background thread).
    ConfigChanged,
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
    history_window: Option<AnyWindowHandle>,
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
    right_option_tap: bool,
    /// Push-to-talk replays this WAV instead of opening the microphone (debug and self-test).
    pub source_override: Option<PathBuf>,
    /// Simulate ⌘V after copying the final text. The self-test turns it off: it must never
    /// type into other apps.
    pub auto_paste: bool,
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
            history_window: None,
            hide_task: None,
            load_task: None,
            unload_task: None,
            reloading: false,
            config,
            config_watcher: None,
            config_task: None,
            config_reloads: 0,
            right_option_tap: false,
            source_override: None,
            auto_paste: true,
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
        }
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
            tray::MENU_QUIT => {
                log::info!("quitting");
                cx.quit();
            }
            _ => log::warn!("unknown menu item {id:?}"),
        }
    }

    /// Press: start a session and show the overlay. Release: stop it; `Final` follows.
    pub fn on_push_to_talk(&mut self, pressed: bool, source: &str, cx: &mut Context<Self>) {
        if pressed {
            let audio = match &self.source_override {
                Some(wav) => AudioSource::WavReplay(wav.clone()),
                None => {
                    // Never open the mic while permission is undetermined: CoreAudio hangs.
                    let permission = easyvox_core::mic_permission_status();
                    if permission != MicPermission::Authorized {
                        log::warn!("push-to-talk: Microphone access is {permission:?}");
                        let msg = "Microphone access needed: menu bar icon → Settings… → Permissions";
                        self.show_notice(msg.into(), cx);
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
        }
    }

    pub fn start_session(&mut self, audio: AudioSource, source: &str, cx: &mut Context<Self>) {
        if !self.is_idle() {
            return; // key repeat, or a press while the last session finishes
        }
        let Some(engine) = self.engine.clone() else {
            let msg = match &cx.global::<AppStatus>().engine {
                EngineStatus::Failed { .. } => "The speech model didn't load: see Settings",
                _ => "The speech model is still loading…",
            };
            self.show_notice(msg.into(), cx);
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
                self.show_notice(format!("Can't start: {e}"), cx);
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
        self.overlay.view.update(cx, |v, cx| v.reset(cx));
        self.overlay.show();
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
            }
            TranscriptEvent::Error(e) => {
                // The core still sends a Final after an error.
                log::error!("session {seq}: {e}");
                self.overlay.view.update(cx, |v, cx| {
                    v.provisional = format!("  ⚠ {e}");
                    cx.notify();
                });
            }
        }
    }

    /// The final text: clipboard (and ⌘V), history, then hide the pill.
    fn deliver(&mut self, text: String, audio_secs: f32, cx: &mut Context<Self>) {
        if text.trim().is_empty() {
            // Nothing heard: leave the clipboard and history alone.
            self.show_notice("Didn't catch anything".into(), cx);
            return;
        }
        self.overlay.view.update(cx, |v, cx| v.set_final(text.clone(), cx));
        // The overlay never takes focus, so the app the user was typing in gets the ⌘V.
        let result = if self.auto_paste {
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
            }
            Err(e) => {
                let e = e.to_string();
                log::warn!("paste: {e}");
                let msg = if e.contains("Accessibility") {
                    "Copied. Allow Accessibility in Settings to paste automatically".to_string()
                } else {
                    format!("Copied. Paste failed: {e}")
                };
                self.show_notice(msg, cx);
            }
        }
    }

    fn show_notice(&mut self, message: String, cx: &mut Context<Self>) {
        self.overlay.view.update(cx, |v, cx| v.set_notice(message, cx));
        self.overlay.show();
        let after = cx.global::<AppSettings>().app.overlay_hide_after_notice_ms;
        self.schedule_hide(Duration::from_millis(after.into()), cx);
    }

    fn schedule_hide(&mut self, after: Duration, cx: &mut Context<Self>) {
        self.hide_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(after).await;
            let _ = this.update(cx, |c, _| {
                if c.is_idle() {
                    c.overlay.hide();
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
        self.load_engine(model_id, cx);
        self.save_settings(cx);
    }

    /// Loads (or switches to) a speech model on the background executor. The engine it
    /// replaces is dropped there too, since freeing a model can block.
    pub fn load_engine(&mut self, model_id: &str, cx: &mut Context<Self>) {
        // No tuning here: each session takes the one in force when it starts.
        let mut config = easyvox_core::EngineConfig::recommended(easyvox_core::paths::models_dir());
        config.model_id = model_id.to_string();
        cx.global_mut::<AppSettings>().model = model_id.to_string();
        cx.global_mut::<AppStatus>().engine = EngineStatus::Loading;
        cx.refresh_windows();
        log::info!("engine: loading {model_id}");
        let model = model_id.to_string();
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let started = Instant::now();
            let result = cx.background_spawn(async move { easyvox_core::Engine::load(config) }).await;
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

    /// Turns "hold right ⌥ to talk" on or off (from Settings). If the event tap can't be
    /// installed, it stays off and Settings shows why.
    pub fn set_hold_right_option(&mut self, on: bool, cx: &mut Context<Self>) {
        if on && !self.ensure_right_option_tap(cx) {
            return;
        }
        cx.global_mut::<AppSettings>().hold_right_option = on;
        cx.global_mut::<AppStatus>().right_option_error = None;
        cx.refresh_windows();
        self.save_settings(cx);
    }

    /// Installs the right-⌥ event tap the first time it's needed. False, with the reason in
    /// `AppStatus`, if it couldn't be.
    pub fn ensure_right_option_tap(&mut self, cx: &mut Context<Self>) -> bool {
        if self.right_option_tap {
            return true;
        }
        let tx = self.tx.clone();
        let result = platform::modifier_tap::install(move |pressed| {
            let _ = tx.send_blocking(UiEvent::RightOption { pressed });
        });
        match result {
            Ok(()) => {
                log::info!("right ⌥ tap installed");
                self.right_option_tap = true;
                cx.global_mut::<AppStatus>().right_option_error = None;
            }
            Err(e) => {
                log::warn!("right ⌥ tap: {e:#}");
                cx.global_mut::<AppStatus>().right_option_error = Some(e.to_string());
            }
        }
        cx.refresh_windows();
        self.right_option_tap
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
    /// numbers, which it reads as it draws and hides.
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
        if new.hold_right_option && !old.hold_right_option {
            self.ensure_right_option_tap(cx);
        }
        if new.model != old.model {
            self.load_engine(&new.model, cx);
        } else if new.keep_model_loaded != old.keep_model_loaded
            || new.app.unload_after_secs != old.app.unload_after_secs
        {
            self.apply_keep_model_loaded(cx);
        }
        self.overlay.view.update(cx, |_, cx| cx.notify());
        cx.refresh_windows();
    }

    pub fn open_settings(&mut self, cx: &mut Context<Self>) {
        if Self::bring_forward(self.settings_window, cx) {
            return;
        }
        let options = window_options("EasyVox Settings", size(px(880.), px(520.)), cx);
        match gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| SettingsView::new(window, cx))) {
            Ok((handle, _)) => {
                self.settings_window = Some(handle);
                cx.activate(true); // an accessory app has to bring itself forward
            }
            Err(e) => log::error!("couldn't open Settings: {e:#}"),
        }
    }

    pub fn open_history(&mut self, cx: &mut Context<Self>) {
        if Self::bring_forward(self.history_window, cx) {
            return;
        }
        let options = window_options("EasyVox History", size(px(560.), px(640.)), cx);
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
