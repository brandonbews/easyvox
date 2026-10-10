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
    /// Keep the speech model in memory while unused. Off, it's freed after `unload_after_secs`
    /// without a dictation and reloaded on the next key press, while recording.
    pub keep_model_loaded: bool,
    /// How long without a dictation before the model is freed (unless `keep_model_loaded`).
    pub unload_after_secs: u64,
    /// Advanced: the speech pipeline's numbers (preview, chunks, pauses). Applied when the
    /// engine loads.
    pub tuning: easyvox_core::Tuning,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            model: easyvox_core::RECOMMENDED_MODEL.into(),
            shortcut: crate::hotkey::DEFAULT_SHORTCUT.into(),
            hold_right_option: false,
            keep_model_loaded: false,
            unload_after_secs: 10 * 60,
            tuning: easyvox_core::Tuning::default(),
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
    /// Freed after a stretch of no use; the next dictation reloads it.
    Unloaded,
    Failed {
        error: String,
    },
}
