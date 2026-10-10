//! App-wide state the windows render from. These are GPUI globals, not fields on the
//! `Controller`, because a view can render while the controller is mid-update, and reading an
//! entity that is being updated panics.

use gpui_kit::Global;

/// The settings in force. They come from the settings file (`config.rs`): the defaults, with
/// whatever the file sets on top. The Settings window shows the first four and writes them back
/// to the file; the rest are `[advanced]`, in the file only.
#[derive(Clone, Debug, PartialEq)]
pub struct AppSettings {
    /// Catalog id of the speech model (see `easyvox_core::catalog`).
    pub model: String,
    /// Push-to-talk shortcut in GPUI keystroke syntax, e.g. `alt-space`.
    pub shortcut: String,
    /// Also dictate while the right ⌥ key is held on its own (needs Input Monitoring).
    pub hold_right_option: bool,
    /// Keep the speech model in memory while unused. Off, it's freed after
    /// `app.unload_after_secs` without a dictation and reloaded on the next key press, while
    /// recording.
    pub keep_model_loaded: bool,
    /// Advanced: the app's own timings and sizes.
    pub app: AppTuning,
    /// Advanced: the speech pipeline's numbers (preview, chunks, pauses). Each dictation takes
    /// the values in force when it starts.
    pub tuning: easyvox_core::Tuning,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            model: easyvox_core::RECOMMENDED_MODEL.into(),
            shortcut: crate::hotkey::DEFAULT_SHORTCUT.into(),
            hold_right_option: false,
            keep_model_loaded: false,
            app: AppTuning::default(),
            tuning: easyvox_core::Tuning::default(),
        }
    }
}

easyvox_core::tunables! {
    /// The app's own numbers, in the settings file's `[advanced]` table next to the pipeline's
    /// [`Tuning`](easyvox_core::Tuning), and declared the same way: each with its default and
    /// allowed range.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct AppTuning {
        /// How long EasyVox keeps the speech model in memory after your last dictation, unless
        /// "Keep model loaded" is on. Freeing it takes the app from about 670 MB to about
        /// 90 MB, and the next dictation still records at once while the model loads again
        /// (about 0.15 s). Lower frees the memory sooner but reloads more often; higher keeps
        /// it ready longer.
        unload_after_secs: u64 = 600, 1..=86_400;
        /// How long the overlay stays up after the final text is pasted. Higher leaves more time
        /// to read it; lower gets it out of the way sooner. 0 hides it at once.
        overlay_hide_after_final_ms: u32 = 600, 0..=10_000;
        /// How long a notice (no microphone access, nothing heard, the model still loading)
        /// stays up. Higher leaves more time to read it.
        overlay_hide_after_notice_ms: u32 = 2_500, 500..=30_000;
        /// The most text the overlay shows: longer text keeps only its end, so the newest words
        /// stay in view. Higher shows more of what came before, but past about 72 the newest
        /// words can run past the edge of the overlay and be cut off.
        overlay_max_chars: u32 = 72, 10..=500;
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
    /// The settings file; why it couldn't be read or written, if it couldn't; and the keys in
    /// it that were ignored or put back to their defaults.
    pub config_path: std::path::PathBuf,
    pub config_error: Option<String>,
    pub config_warnings: Vec<String>,
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
