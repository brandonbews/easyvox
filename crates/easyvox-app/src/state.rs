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
        /// stays up. It's two lines, what happened and what to do, so it needs a moment to read.
        /// Higher leaves more time; lower gets it out of the way sooner.
        overlay_hide_after_notice_ms: u32 = 4_000, 500..=30_000;
        /// The most of the transcript the overlay lays out. The pill shows the end of it, as
        /// much as fits, and scrolls as words come so the newest stay in view; this caps the
        /// text kept out of sight past its left edge, where it's cut. Lower draws a little less
        /// each frame, but under about 120 the cut (an "…") can come into view; higher costs
        /// more to lay out for nothing you'd see.
        overlay_max_chars: u32 = 200, 10..=500;
        /// How far above the Dock the overlay sits, in points, from the top of the Dock to the
        /// bottom of the pill; above the bottom of the screen when the Dock is hidden or on a
        /// side. It goes on the screen with the window you're typing in, centred. Higher keeps it
        /// further from the Dock; lower, closer to the bottom of the screen.
        overlay_dock_gap_pt: u32 = 24, 0..=400;
        /// How long the overlay takes to pop open when you press the shortcut: it fades in as
        /// it grows from `overlay_shrink_pct` smaller to full size, just past it and back. It
        /// shows at once and never waits for the model; this is only how it appears. Core
        /// Animation runs it, so it costs no redraws. Higher is gentler; lower is snappier. 0
        /// shows it at once.
        overlay_open_ms: u32 = 200, 0..=1_000;
        /// How long the overlay takes to close once it hides: it fades out as it shrinks by
        /// `overlay_shrink_pct`. The paste has happened by then, and a press during the close
        /// brings the overlay straight back, so it never delays you. Higher is gentler; lower
        /// gets it out of the way sooner. 0 hides it at once.
        overlay_close_ms: u32 = 120, 0..=1_000;
        /// How much smaller than full size the overlay pops open from and closes to. With
        /// macOS's Reduce motion on, it doesn't change size at all and only fades. Higher is
        /// more noticeable; 0 only fades.
        overlay_shrink_pct: u32 = 15, 0..=30;
        /// How long new words take to fade in, and provisional words (drawn fainter) to firm
        /// up once they're confirmed. Each change draws the overlay every `overlay_frame_ms`
        /// for this long. Higher is softer but redraws more; lower is cheaper and more abrupt.
        /// 0 changes the text at once.
        overlay_words_ms: u32 = 200, 0..=1_000;
        /// How long the overlay takes to grow to fit new words, and to scroll long text so the
        /// newest words stay in view, instead of jumping. With Reduce motion on, both jump.
        /// Higher is smoother but redraws for longer; lower is cheaper. 0 jumps.
        overlay_glide_ms: u32 = 250, 0..=1_000;
        /// How long the level meter's bars take to ease to a new height while you talk,
        /// instead of jumping there. While only the meter moves, the overlay draws every
        /// `overlay_meter_frame_ms` for this long after each change, which is most of the time
        /// you're talking. With Reduce motion on, the bars jump. Higher is smoother but draws
        /// for longer; lower is cheaper and more abrupt. 0 jumps.
        overlay_meter_ms: u32 = 120, 0..=1_000;
        /// Time between the frames of the overlay's words fading and gliding, while one runs;
        /// nothing is drawn once it ends. The open and close don't use it. Lower is smoother
        /// (16 is 60 frames a second; 8 suits a 120 Hz screen) but draws more often; higher is
        /// lighter and choppier.
        overlay_frame_ms: u32 = 16, 8..=100;
        /// Time between the frames of the level meter's easing while nothing else in the
        /// overlay moves (the words and the glide use `overlay_frame_ms`). Higher is lighter (33
        /// is 30 frames a second, 50 is 20) and choppier; lower is smoother but draws more often.
        overlay_meter_frame_ms: u32 = 33, 8..=100;
        /// How long the menu bar item shows a check after EasyVox pastes your text, before it
        /// goes back to how it was. Higher leaves more time to glance at it; 0 skips the check.
        menubar_pasted_ms: u32 = 1_000, 0..=10_000;
    }
}

impl Global for AppSettings {}

/// Runtime status, written by the controller and shown in Settings and onboarding.
#[derive(Default)]
pub struct AppStatus {
    /// Where models are downloaded to and loaded from: `easyvox_core::paths::models_dir()`,
    /// set at launch. The self-test points it at a temporary folder for a while.
    pub models_dir: std::path::PathBuf,
    pub engine: EngineStatus,
    /// Model downloads running or failed, by catalog id. One is removed when it succeeds or is
    /// cancelled; a failed one stays until the next try or a delete.
    pub downloads: std::collections::HashMap<&'static str, DownloadStatus>,
    /// The macOS Microphone prompt is up (asked from onboarding or Settings).
    pub mic_asking: bool,
    /// The macOS Accessibility prompt was shown since launch. Accessibility has no "denied"
    /// answer to read back, so this is how onboarding knows to point at System Settings next.
    pub accessibility_prompted: bool,
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

#[derive(Clone, Debug, Default, PartialEq)]
pub enum EngineStatus {
    #[default]
    NotLoaded,
    /// The chosen model isn't downloaded (or is only partly there), so there's no engine.
    Missing,
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

/// One model download, as Settings and onboarding show it.
#[derive(Clone, Debug)]
pub enum DownloadStatus {
    Running(easyvox_core::download::Progress),
    /// Why it failed: its `Display` is one sentence for the UI, and onboarding reads the kind
    /// (offline, wrong file, …) to say what to do. Nothing was kept.
    Failed(easyvox_core::download::DownloadError),
}
