//! EasyVox: hold a shortcut, speak, and the text is pasted where you're typing. A menu bar app:
//! a thin GPUI Kit shell around `easyvox-core`.
//!
//! ```text
//! easyvox                              run normally (⌥Space push-to-talk)
//! easyvox --print-default-config       print every settings-file key at its default, and quit
//! easyvox --self-test [--out <file>]   feature `self-test`: scripted check, JSON report, exit code
//! ```
//!
//! Settings come from `~/.config/easyvox/config.toml` (see `config.rs`). Logs go to stderr;
//! `EASYVOX_LOG=debug` shows more.

mod about;
mod brand;
mod config;
mod controller;
mod curve;
mod history;
mod hotkey;
mod logger;
mod model_row;
mod onboarding;
mod onboarding_window;
mod overlay;
mod permissions;
mod platform;
#[cfg(feature = "self-test")]
mod selftest;
mod settings;
mod state;
mod tray;

use controller::{Controller, ControllerGlobal, UiEvent};
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
    if std::env::args().skip(1).any(|a| a == "--print-default-config") {
        print!("{}", config::default_config_text());
        return;
    }
    // `GGML_METAL_NO_RESIDENCY=1`: stops ggml-metal waking the app ~150 times a second for as
    // long as it runs, loaded or not (see `easyvox_core::process_env`). Set here rather than in
    // Info.plist's LSEnvironment so it also holds for `cargo run` and the bare binary.
    for (key, value) in easyvox_core::process_env() {
        // SAFETY: the first thing `main` does, before GPUI, the logger or anything else starts
        // a thread, so nothing can read the environment while it changes.
        unsafe { std::env::set_var(key, value) };
    }
    logger::init();
    #[cfg(feature = "self-test")]
    let self_test = selftest::Options::from_args();
    #[cfg(feature = "self-test")]
    let history_db = self_test.as_ref().map_or_else(easyvox_core::paths::history_db, |o| o.history_db());
    #[cfg(not(feature = "self-test"))]
    let history_db = easyvox_core::paths::history_db();
    // The self-test never reads or writes your settings: it uses its own file unless
    // EASYVOX_CONFIG names one.
    #[cfg(feature = "self-test")]
    let config_path = self_test.as_ref().map_or_else(easyvox_core::paths::config_file, |o| o.config_file());
    #[cfg(not(feature = "self-test"))]
    let config_path = easyvox_core::paths::config_file();
    let mut config = config::ConfigFile::new(config_path);
    let (loaded, config_error) = config.load();
    log::info!("settings file: {}", config.path().display());
    for warning in &loaded.warnings {
        log::warn!("settings file: {warning}");
    }
    if let Some(e) = &config_error {
        log::error!("settings file: {e}; using the defaults");
    }

    application().with_assets(brand::Assets).run(move |cx: &mut App| {
        gpui_kit::init(cx);
        // gpui-kit's init picks its own light theme. Switch to the branded pair, at the system's
        // appearance; each window keeps it in sync from then on (`Theme::sync_system_appearance`).
        brand::install_theme(cx);
        // A menu bar app: no Dock icon and no app menu (Info.plist sets LSUIElement too).
        cx.set_activation_policy(ActivationPolicy::Accessory);
        cx.set_global(loaded.settings);
        cx.set_global(AppStatus {
            models_dir: easyvox_core::paths::models_dir(),
            config_path: config.path().to_path_buf(),
            config_error,
            config_warnings: loaded.warnings,
            ..Default::default()
        });

        // One channel carries every OS callback and transcript event to the UI thread.
        let (tx, rx) = async_channel::unbounded::<UiEvent>();

        let (menu_tx, open_tx) = (tx.clone(), tx.clone());
        let tray = match tray::create(
            move |id| {
                let _ = menu_tx.send_blocking(UiEvent::Menu(id));
            },
            move || {
                let _ = open_tx.send_blocking(UiEvent::TrayOpened);
            },
        ) {
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
        let watch_path = config.path().to_path_buf();
        let controller = cx.new(|_| Controller::new(hotkeys, tray, overlay, history, config, tx.clone()));
        cx.set_global(ControllerGlobal(controller.clone()));

        // Outside edits to the settings file apply live. FSEvents: nothing runs until it changes.
        let config_tx = tx.clone();
        match config::watch(&watch_path, move || {
            let _ = config_tx.try_send(UiEvent::ConfigChanged);
        }) {
            Ok(watcher) => controller.update(cx, |c, _| c.config_watcher = Some(watcher)),
            Err(e) => log::warn!(
                "settings file: can't watch {} ({e:#}); edits apply at the next launch",
                watch_path.display()
            ),
        }
        // Hold right ⌥: its event tap, if Input Monitoring is allowed (otherwise Settings says
        // it's waiting, and the tap comes once it is).
        controller.update(cx, |c, cx| c.apply_right_option(cx));

        let target = controller.clone();
        cx.spawn(async move |cx| {
            while let Ok(event) = rx.recv().await {
                target.update(cx, |c, cx| c.handle(event, cx));
            }
        })
        .detach();

        // Part files left by downloads that were killed (a crash, a force quit). Any that a
        // running download holds stay.
        let models_dir = cx.global::<AppStatus>().models_dir.clone();
        let _ = std::thread::Builder::new()
            .name("remove-partials".into())
            .spawn(move || easyvox_core::download::remove_partials(&models_dir));

        // Loads on the background executor: the menu bar icon and shortcut work meanwhile. A
        // model that isn't downloaded yet shows as missing instead.
        let model = cx.global::<AppSettings>().model.clone();
        controller.update(cx, |c, cx| c.load_engine(&model, cx));

        // First launch, or a step went missing since (a reset permission, a deleted model):
        // onboarding. There's no "done" flag to store; the system and the models folder say.
        let launch = onboarding_window::launch_facts(cx);
        controller.update(cx, |c, cx| c.onboard_if_needed(&launch, cx));

        // The menu bar item follows the app's state: whenever the status or the settings change
        // (the model loads, a download starts, the shortcut changes), it shows what's new, and
        // nothing if nothing visible changed.
        let tray_status = controller.clone();
        cx.observe_global::<AppStatus>(move |cx| tray_status.update(cx, |c, cx| c.refresh_tray(cx))).detach();
        let tray_settings = controller.clone();
        cx.observe_global::<AppSettings>(move |cx| tray_settings.update(cx, |c, cx| c.refresh_tray(cx))).detach();
        controller.update(cx, |c, cx| c.refresh_tray(cx));

        #[cfg(feature = "self-test")]
        if let Some(options) = self_test {
            selftest::start(controller, options, cx);
        }
    });
}
