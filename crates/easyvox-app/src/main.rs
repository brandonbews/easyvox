//! EasyVox: hold a shortcut, speak, and the text is pasted where you're typing. A menu bar app:
//! a thin GPUI Kit shell around `easyvox-core`.
//!
//! ```text
//! easyvox                              run normally (⌥Space push-to-talk)
//! easyvox --self-test [--out <file>]   feature `self-test`: scripted check, JSON report, exit code
//! ```
//!
//! Logs go to stderr; `EASYVOX_LOG=debug` shows more.

mod controller;
mod history;
mod hotkey;
mod logger;
mod overlay;
mod platform;
#[cfg(feature = "self-test")]
mod selftest;
mod settings;
mod state;
mod tray;

use controller::{Controller, ControllerGlobal, UiEvent};
use gpui_kit::component::Theme;
use gpui_kit::*;
use state::{AppSettings, AppStatus};

/// The workspace's test clips, for the debug "Test dictation" item and the self-test. The path
/// is baked in at build time; `EASYVOX_TESTDATA` overrides it.
#[cfg(any(debug_assertions, feature = "self-test"))]
pub fn testdata_dir() -> std::path::PathBuf {
    std::env::var_os("EASYVOX_TESTDATA")
        .map(Into::into)
        .unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata").into())
}

/// Startup failures that leave nothing useful to run: log and exit (a panic here would abort
/// inside an AppKit callback and leave only a crash report).
fn fatal(what: &str, error: anyhow::Error) -> ! {
    log::error!("{what}: {error:#}");
    std::process::exit(1)
}

fn main() {
    logger::init();
    #[cfg(feature = "self-test")]
    let self_test = selftest::Options::from_args();
    #[cfg(feature = "self-test")]
    let history_db = self_test.as_ref().map_or_else(easyvox_core::paths::history_db, |o| o.history_db());
    #[cfg(not(feature = "self-test"))]
    let history_db = easyvox_core::paths::history_db();

    application().with_assets(assets::Assets).run(move |cx: &mut App| {
        gpui_kit::init(cx);
        // gpui-kit's init picks the light theme; follow the system instead. Each window keeps
        // it in sync from then on.
        Theme::sync_system_appearance(None, cx);
        // A menu bar app: no Dock icon and no app menu (Info.plist sets LSUIElement too).
        cx.set_activation_policy(ActivationPolicy::Accessory);
        cx.set_global(AppSettings::default());
        cx.set_global(AppStatus::default());

        // One channel carries every OS callback and transcript event to the UI thread.
        let (tx, rx) = async_channel::unbounded::<UiEvent>();

        let menu_tx = tx.clone();
        let tray = match tray::create(move |id| {
            let _ = menu_tx.send_blocking(UiEvent::Menu(id));
        }) {
            Ok(tray) => {
                log::info!("tray created");
                Some(tray)
            }
            Err(e) => {
                log::error!("couldn't create the menu bar icon: {e:#}");
                None
            }
        };

        let hotkey_tx = tx.clone();
        let mut hotkeys = hotkey::HotkeyService::new(move |id, pressed| {
            let _ = hotkey_tx.send_blocking(UiEvent::HotKey { id, pressed });
        })
        .unwrap_or_else(|e| fatal("couldn't set up global shortcuts", e));
        let shortcut = cx.global::<AppSettings>().shortcut.clone();
        match hotkeys.bind(&shortcut) {
            Ok(()) => log::info!("hotkey registered: {shortcut}"),
            Err(e) => {
                log::error!("hotkey: {e:#}");
                cx.global_mut::<AppStatus>().shortcut_error = Some(e.to_string());
            }
        }

        let overlay = overlay::create(cx).unwrap_or_else(|e| fatal("couldn't create the overlay", e));
        let history = cx.new(|_| history::HistoryStore::open(&history_db));
        let controller = cx.new(|_| Controller::new(hotkeys, tray, overlay, history, tx));
        cx.set_global(ControllerGlobal(controller.clone()));

        let target = controller.clone();
        cx.spawn(async move |cx| {
            while let Ok(event) = rx.recv().await {
                target.update(cx, |c, cx| c.handle(event, cx));
            }
        })
        .detach();

        // Loads on the background executor: the menu bar icon and shortcut work meanwhile.
        let model = cx.global::<AppSettings>().model.clone();
        controller.update(cx, |c, cx| c.load_engine(&model, cx));

        #[cfg(feature = "self-test")]
        if let Some(options) = self_test {
            selftest::start(controller, options, cx);
        }
    });
}
