//! App-wide state the windows render from. These are GPUI globals, not fields on the
//! `Controller`, because a view can render while the controller is mid-update, and reading an
//! entity that is being updated panics.

use gpui_kit::Global;

/// The user's settings. In memory for now; they'll be saved to `~/.config/easyvox/config.toml`
/// later, so keep them plain values that map one-to-one onto keys in that file.
pub struct AppSettings {
    /// Catalog id of the speech model (see `easyvox_core::catalog`).
    pub model: String,
    /// Push-to-talk shortcut in GPUI keystroke syntax, e.g. `alt-space`.
    pub shortcut: String,
    /// Also dictate while the right ⌥ key is held on its own (needs Input Monitoring).
    pub hold_right_option: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            model: easyvox_core::RECOMMENDED_MODEL.into(),
            shortcut: crate::hotkey::DEFAULT_SHORTCUT.into(),
            hold_right_option: false,
        }
    }
}

impl Global for AppSettings {}

/// Runtime status, written by the controller and shown in Settings.
#[derive(Default)]
pub struct AppStatus {
    pub engine: EngineStatus,
    /// Why the push-to-talk shortcut couldn't be registered, if it couldn't.
    pub shortcut_error: Option<String>,
    /// Why the right-⌥ tap couldn't be installed, if it couldn't.
    pub right_option_error: Option<String>,
}

impl Global for AppStatus {}

#[derive(Clone, Debug, Default)]
pub enum EngineStatus {
    #[default]
    NotLoaded,
    Loading,
    Ready {
        load_ms: u32,
    },
    Failed {
        error: String,
    },
}
