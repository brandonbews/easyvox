//! First-launch onboarding as plain logic: four steps (Microphone, Accessibility, the speech
//! model, its first load), where each one stands, what it says and what it offers, worked out
//! from [`Facts`] read elsewhere. No GPUI and no system calls here, so the tests need neither
//! TCC nor the network. The window is `onboarding_window.rs`.
//!
//! There's no stored "onboarding done" flag. Onboarding opens at launch whenever a required step
//! is missing ([`LaunchFacts::needed`]): the Microphone was never asked, Accessibility isn't
//! allowed, or the chosen model isn't downloaded. So a fresh Mac, a reset grant or a deleted
//! model all bring it back, and a relaunch with everything in place skips it. Settings →
//! Permissions opens it any time.

use crate::model_row::ModelState;
use crate::state::EngineStatus;
use easyvox_core::MicPermission;
use easyvox_core::download::{DownloadError, Progress};

/// What decides, at launch, whether onboarding opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaunchFacts {
    pub mic: MicPermission,
    pub accessibility: bool,
    /// The chosen model (the default, unless the settings file picks another) is downloaded.
    pub model_downloaded: bool,
}

impl LaunchFacts {
    /// A required step is missing. A Microphone the user said no to doesn't count: it was asked,
    /// and opening onboarding at every launch wouldn't change the answer (Settings and the
    /// push-to-talk notice say where to turn it on).
    pub fn needed(&self) -> bool {
        self.mic == MicPermission::NotDetermined || !self.accessibility || !self.model_downloaded
    }
}

/// Everything the steps are worked out from.
#[derive(Clone, Debug, PartialEq)]
pub struct Facts {
    pub mic: MicPermission,
    /// The macOS Microphone prompt is up.
    pub mic_asking: bool,
    pub accessibility: bool,
    /// The macOS Accessibility prompt was shown since launch.
    pub accessibility_asked: bool,
    /// The chosen model: downloaded, downloading, failed or not there.
    pub model: ModelState,
    /// The speech engine, which loads the chosen model.
    pub engine: EngineStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepId {
    Microphone,
    Accessibility,
    Model,
    /// The model's first load, which compiles ggml's Metal shaders (~14 s, once per app
    /// identity), so the first dictation doesn't wait for it.
    WarmUp,
}

impl StepId {
    pub fn title(self) -> &'static str {
        match self {
            Self::Microphone => "Microphone",
            Self::Accessibility => "Accessibility",
            Self::Model => "Speech model",
            Self::WarmUp => "Get ready",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// Not started: the step's action starts it.
    ToDo,
    /// Waits for an earlier step: the first load waits for the download.
    Waiting,
    Busy(Busy),
    Done,
    /// It went wrong, or the answer was no. The step's actions fix it or try again.
    Problem(Problem),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Busy {
    /// The macOS Microphone prompt is up.
    MicPrompt,
    /// Prompted; waiting for the switch in System Settings.
    AccessibilityPending,
    Downloading,
    /// Checking the sha256 of a copy already on disk.
    Verifying,
    /// The connection dropped or stalled; the download picks up where it stopped.
    Resuming,
    /// The first load (the shader compile).
    WarmingUp,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Problem {
    MicDenied,
    /// Screen Time or a management profile doesn't allow it.
    MicRestricted,
    MicUnknown,
    /// No connection to the server: offline, DNS, refused.
    NoNetwork,
    /// The file that arrived isn't the catalog's (sha256 or size). Nothing was kept.
    WrongFile,
    NoSpace,
    /// Any other download failure, as its sentence.
    DownloadFailed(String),
    LoadFailed(String),
}

/// A System Settings privacy pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    Microphone,
    Accessibility,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Show the macOS Microphone prompt.
    AskMicrophone,
    /// Show the macOS Accessibility prompt.
    AskAccessibility,
    OpenSettings(Pane),
    // The model row draws these three itself (Download, Cancel, Try again).
    Download,
    CancelDownload,
    RetryDownload,
    /// Load the model (again).
    WarmUp,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub id: StepId,
    pub state: State,
    /// What the step is for, or where it stands and what to do next: a sentence or two.
    pub text: String,
    /// What can be done now, the main action first.
    pub actions: Vec<Action>,
}

impl Step {
    fn new(id: StepId, state: State, text: impl Into<String>, actions: &[Action]) -> Self {
        Self { id, state, text: text.into(), actions: actions.to_vec() }
    }

    pub fn is_done(&self) -> bool {
        self.state == State::Done
    }
}

/// The four steps, in order.
#[derive(Clone, Debug, PartialEq)]
pub struct Onboarding {
    pub steps: [Step; 4],
}

impl Onboarding {
    /// Where each step stands.
    pub fn new(facts: &Facts) -> Self {
        let model_step = model_step(&facts.model);
        let warm_up = warm_up_step(model_step.is_done(), &facts.engine);
        Self {
            steps: [
                mic_step(facts.mic, facts.mic_asking),
                accessibility_step(facts.accessibility, facts.accessibility_asked),
                model_step,
                warm_up,
            ],
        }
    }

    #[cfg_attr(not(any(test, feature = "self-test")), allow(dead_code))]
    pub fn step(&self, id: StepId) -> &Step {
        // Always found: `new` builds one step of each kind.
        self.steps.iter().find(|s| s.id == id).unwrap_or(&self.steps[0])
    }

    /// The first step that isn't done: the one to look at next.
    pub fn current(&self) -> Option<StepId> {
        self.steps.iter().find(|s| !s.is_done()).map(|s| s.id)
    }

    /// Every step is done: the first dictation will work, and start without the shader wait.
    pub fn finished(&self) -> bool {
        self.current().is_none()
    }

    /// How many steps are done.
    pub fn done(&self) -> usize {
        self.steps.iter().filter(|s| s.is_done()).count()
    }
}

fn mic_step(mic: MicPermission, asking: bool) -> Step {
    use Action::{AskMicrophone, OpenSettings};
    let id = StepId::Microphone;
    let settings = [OpenSettings(Pane::Microphone)];
    match mic {
        MicPermission::Authorized => Step::new(id, State::Done, "Allowed.", &[]),
        MicPermission::NotDetermined if asking => {
            Step::new(id, State::Busy(Busy::MicPrompt), "Waiting for your answer in the macOS prompt…", &[])
        }
        MicPermission::NotDetermined => Step::new(
            id,
            State::ToDo,
            "EasyVox listens only while you hold the shortcut. macOS asks you once.",
            &[AskMicrophone],
        ),
        MicPermission::Denied => Step::new(
            id,
            State::Problem(Problem::MicDenied),
            "Microphone access is off, so EasyVox can't hear you. Turn EasyVox on in System Settings → \
             Privacy & Security → Microphone.",
            &settings,
        ),
        MicPermission::Restricted => Step::new(
            id,
            State::Problem(Problem::MicRestricted),
            "This Mac's settings don't allow microphone access (Screen Time or a management profile).",
            &settings,
        ),
        MicPermission::Unknown => Step::new(
            id,
            State::Problem(Problem::MicUnknown),
            "macOS didn't say whether EasyVox may use the microphone. Check System Settings → Privacy & \
             Security → Microphone.",
            &settings,
        ),
    }
}

fn accessibility_step(trusted: bool, asked: bool) -> Step {
    use Action::{AskAccessibility, OpenSettings};
    let id = StepId::Accessibility;
    let settings = OpenSettings(Pane::Accessibility);
    match (trusted, asked) {
        (true, _) => Step::new(id, State::Done, "Allowed.", &[]),
        // macOS has no "denied" to read back: after the prompt, all there is to do is point at
        // the switch, and notice when it's on.
        (false, true) => Step::new(
            id,
            State::Busy(Busy::AccessibilityPending),
            "Turn EasyVox on in System Settings → Privacy & Security → Accessibility. This step ticks itself \
             off when you do.",
            &[settings],
        ),
        (false, false) => Step::new(
            id,
            State::ToDo,
            "Lets EasyVox paste the text where you're typing, with ⌘V. Without it, the text waits on the \
             clipboard for you to paste.",
            &[AskAccessibility, settings],
        ),
    }
}

/// The window shows the model's own row under this step, with its name, size, progress, error
/// and buttons, so the words here only frame it.
fn model_step(state: &ModelState) -> Step {
    use Action::{CancelDownload, Download, RetryDownload};
    let id = StepId::Model;
    match state {
        ModelState::Downloaded { .. } => Step::new(id, State::Done, "Downloaded and checked.", &[]),
        ModelState::NotDownloaded => Step::new(
            id,
            State::ToDo,
            "It turns your speech into text, on this Mac. It's downloaded once, from Hugging Face; no audio or \
             text ever leaves the Mac.",
            &[Download],
        ),
        ModelState::Downloading(progress) => {
            let (busy, text) = match progress {
                Progress::Verifying => (Busy::Verifying, "Checking the copy that's already here…"),
                Progress::Downloading { .. } => {
                    (Busy::Downloading, "Downloading. You can carry on with the other steps meanwhile.")
                }
                Progress::Reconnecting { .. } => {
                    (Busy::Resuming, "The connection dropped. The download picks up where it stopped, by itself.")
                }
            };
            Step::new(id, State::Busy(busy), text, &[CancelDownload])
        }
        // The model row shows what went wrong; this says what to do about it.
        ModelState::Failed(error) => {
            let (problem, text) = match error {
                DownloadError::Offline(_) => {
                    (Problem::NoNetwork, "This Mac seems to be offline. Connect to the internet, then click Try again.")
                }
                DownloadError::ChecksumMismatch { .. } | DownloadError::SizeMismatch { .. } => (
                    Problem::WrongFile,
                    "What arrived wasn't the expected file, so EasyVox didn't keep it. Click Try again to \
                     download it afresh.",
                ),
                DownloadError::NotEnoughSpace { .. } => {
                    (Problem::NoSpace, "Free up some disk space, then click Try again.")
                }
                other => (Problem::DownloadFailed(other.to_string()), "Click Try again to start the download over."),
            };
            Step::new(id, State::Problem(problem), text, &[RetryDownload])
        }
    }
}

const WARM_UP_INTRO: &str = "Once the model is here, EasyVox loads it once to set up this Mac's graphics chip: \
                             about 15 seconds, the first time only. Then your first dictation starts at once.";

fn warm_up_step(model_downloaded: bool, engine: &EngineStatus) -> Step {
    let id = StepId::WarmUp;
    if !model_downloaded {
        return Step::new(id, State::Waiting, WARM_UP_INTRO, &[]);
    }
    match engine {
        EngineStatus::Loading => Step::new(
            id,
            State::Busy(Busy::WarmingUp),
            "Getting ready… The first time takes about 15 seconds; after that, the model loads in a fraction \
             of a second.",
            &[],
        ),
        // Unloaded: it loaded once and was freed after a while unused; the shaders are compiled.
        EngineStatus::Ready { .. } | EngineStatus::Unloaded => {
            Step::new(id, State::Done, "Ready. Your first dictation starts at once.", &[])
        }
        EngineStatus::Failed { error } => Step::new(
            id,
            State::Problem(Problem::LoadFailed(error.clone())),
            format!("Couldn't load the model: {error}"),
            &[Action::WarmUp],
        ),
        EngineStatus::NotLoaded | EngineStatus::Missing => Step::new(id, State::ToDo, WARM_UP_INTRO, &[Action::WarmUp]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Mac with nothing done yet: a fresh install.
    fn fresh() -> Facts {
        Facts {
            mic: MicPermission::NotDetermined,
            mic_asking: false,
            accessibility: false,
            accessibility_asked: false,
            model: ModelState::NotDownloaded,
            engine: EngineStatus::Missing,
        }
    }

    /// Everything in place, the model loaded.
    fn ready() -> Facts {
        Facts {
            mic: MicPermission::Authorized,
            mic_asking: false,
            accessibility: true,
            accessibility_asked: false,
            model: ModelState::Downloaded { bytes: 539_013_000 },
            engine: EngineStatus::Ready { load_ms: 180 },
        }
    }

    fn step(facts: &Facts, id: StepId) -> Step {
        Onboarding::new(facts).step(id).clone()
    }

    fn states(facts: &Facts) -> Vec<State> {
        Onboarding::new(facts).steps.iter().map(|s| s.state.clone()).collect()
    }

    #[test]
    fn a_fresh_mac_starts_at_the_microphone_with_everything_to_do() {
        let o = Onboarding::new(&fresh());
        assert_eq!(states(&fresh()), [State::ToDo, State::ToDo, State::ToDo, State::Waiting]);
        assert_eq!(o.current(), Some(StepId::Microphone));
        assert!(!o.finished());
        assert_eq!(o.done(), 0);
        assert_eq!(o.step(StepId::Microphone).actions, [Action::AskMicrophone]);
        assert_eq!(
            o.step(StepId::Accessibility).actions,
            [Action::AskAccessibility, Action::OpenSettings(Pane::Accessibility)]
        );
        assert_eq!(o.step(StepId::Model).actions, [Action::Download]);
        assert!(o.step(StepId::WarmUp).actions.is_empty(), "nothing to load before the download");
    }

    #[test]
    fn everything_in_place_is_finished() {
        let o = Onboarding::new(&ready());
        assert!(o.finished());
        assert_eq!(o.current(), None);
        assert_eq!(o.done(), 4);
        assert!(o.steps.iter().all(|s| s.actions.is_empty()));
    }

    #[test]
    fn the_microphone_step_follows_the_permission_and_the_prompt() {
        let with = |mic, asking| step(&Facts { mic, mic_asking: asking, ..fresh() }, StepId::Microphone);
        assert_eq!(with(MicPermission::NotDetermined, true).state, State::Busy(Busy::MicPrompt));
        assert!(with(MicPermission::NotDetermined, true).actions.is_empty(), "no second prompt while one is up");
        assert_eq!(with(MicPermission::Authorized, false).state, State::Done);
        let settings = [Action::OpenSettings(Pane::Microphone)];
        for (mic, problem) in [
            (MicPermission::Denied, Problem::MicDenied),
            (MicPermission::Restricted, Problem::MicRestricted),
            (MicPermission::Unknown, Problem::MicUnknown),
        ] {
            let s = with(mic, false);
            assert_eq!(s.state, State::Problem(problem), "{mic:?}");
            assert_eq!(s.actions, settings, "{mic:?}: a denied permission is fixed in System Settings");
        }
        // The answer came back while the prompt flag was still set: the answer wins.
        assert_eq!(with(MicPermission::Denied, true).state, State::Problem(Problem::MicDenied));
    }

    #[test]
    fn accessibility_asks_first_then_points_at_system_settings() {
        let with = |accessibility, asked| {
            step(&Facts { accessibility, accessibility_asked: asked, ..fresh() }, StepId::Accessibility)
        };
        assert_eq!(with(false, false).state, State::ToDo);
        let pending = with(false, true);
        assert_eq!(pending.state, State::Busy(Busy::AccessibilityPending));
        assert_eq!(pending.actions, [Action::OpenSettings(Pane::Accessibility)]);
        assert!(pending.text.contains("ticks itself off"));
        assert_eq!(with(true, false).state, State::Done);
        assert_eq!(with(true, true).state, State::Done, "allowed after the prompt");
    }

    #[test]
    fn the_model_step_follows_the_download() {
        let with = |model| step(&Facts { model, ..fresh() }, StepId::Model);
        let total = 539_012_608;
        let cancel = [Action::CancelDownload];
        for (progress, busy) in [
            (Progress::Downloading { done: 1, total }, Busy::Downloading),
            (Progress::Verifying, Busy::Verifying),
            (Progress::Reconnecting { attempt: 1, max: 5, done: 100, total }, Busy::Resuming),
        ] {
            let s = with(ModelState::Downloading(progress));
            assert_eq!(s.state, State::Busy(busy), "{progress:?}");
            assert_eq!(s.actions, cancel, "{progress:?}");
        }
        assert_eq!(with(ModelState::Downloaded { bytes: total }).state, State::Done);
    }

    #[test]
    fn each_download_failure_says_what_to_do_and_offers_a_retry() {
        let failed = |e| step(&Facts { model: ModelState::Failed(e), ..fresh() }, StepId::Model);
        let cases = [
            (DownloadError::Offline("The Internet connection appears to be offline.".into()), Problem::NoNetwork),
            (DownloadError::ChecksumMismatch { expected: "a".into(), actual: "b".into() }, Problem::WrongFile),
            (DownloadError::SizeMismatch { expected: 2, actual: 3 }, Problem::WrongFile),
            (DownloadError::NotEnoughSpace { needed: 2, available: 1 }, Problem::NoSpace),
            (
                DownloadError::Network("timed out".into()),
                Problem::DownloadFailed("The download failed. Timed out.".into()),
            ),
            (
                DownloadError::Http(404),
                Problem::DownloadFailed("The server doesn't have this model (HTTP 404).".into()),
            ),
        ];
        for (error, problem) in cases {
            let s = failed(error.clone());
            assert_eq!(s.state, State::Problem(problem), "{error:?}");
            assert_eq!(s.actions, [Action::RetryDownload], "{error:?}");
            assert!(s.text.contains("Try again"), "{error:?}: {}", s.text);
        }
        assert!(failed(DownloadError::Offline(String::new())).text.contains("offline"));
    }

    #[test]
    fn the_warm_up_waits_for_the_download_then_follows_the_engine() {
        let downloaded = ModelState::Downloaded { bytes: 1 };
        let with =
            |model: &ModelState, engine| step(&Facts { model: model.clone(), engine, ..ready() }, StepId::WarmUp);

        // Before the model is here there's nothing to load, whatever the engine says.
        for model in [
            ModelState::NotDownloaded,
            ModelState::Downloading(Progress::Verifying),
            ModelState::Failed(DownloadError::Http(500)),
        ] {
            let s = with(&model, EngineStatus::Missing);
            assert_eq!(s.state, State::Waiting, "{model:?}");
            assert!(s.actions.is_empty());
        }
        assert_eq!(with(&downloaded, EngineStatus::Loading).state, State::Busy(Busy::WarmingUp));
        assert!(with(&downloaded, EngineStatus::Loading).text.starts_with("Getting ready"));
        assert_eq!(with(&downloaded, EngineStatus::Ready { load_ms: 14_000 }).state, State::Done);
        assert_eq!(with(&downloaded, EngineStatus::Unloaded).state, State::Done, "loaded once, then freed");
        for engine in [EngineStatus::NotLoaded, EngineStatus::Missing] {
            let s = with(&downloaded, engine.clone());
            assert_eq!((s.state, s.actions), (State::ToDo, vec![Action::WarmUp]), "{engine:?}");
        }
        let failed = with(&downloaded, EngineStatus::Failed { error: "bad GGUF".into() });
        assert_eq!(failed.state, State::Problem(Problem::LoadFailed("bad GGUF".into())));
        assert_eq!(failed.actions, [Action::WarmUp]);
        assert!(failed.text.contains("bad GGUF"));
    }

    #[test]
    fn the_current_step_is_the_first_one_not_done() {
        // The download can run while the permissions wait: the model step isn't current yet.
        let downloading =
            Facts { model: ModelState::Downloading(Progress::Downloading { done: 5, total: 10 }), ..fresh() };
        assert_eq!(Onboarding::new(&downloading).current(), Some(StepId::Microphone));

        let permissions_done = Facts { mic: MicPermission::Authorized, accessibility: true, ..downloading };
        assert_eq!(Onboarding::new(&permissions_done).current(), Some(StepId::Model));

        // A denied microphone stays current: the rest can be done, but onboarding isn't finished.
        let denied = Facts { mic: MicPermission::Denied, ..ready() };
        let o = Onboarding::new(&denied);
        assert_eq!((o.current(), o.done()), (Some(StepId::Microphone), 3));

        let warming = Facts { engine: EngineStatus::Loading, ..ready() };
        let o = Onboarding::new(&warming);
        assert_eq!((o.current(), o.finished()), (Some(StepId::WarmUp), false));
    }

    #[test]
    fn onboarding_opens_at_launch_only_when_a_required_step_is_missing() {
        let all = LaunchFacts { mic: MicPermission::Authorized, accessibility: true, model_downloaded: true };
        assert!(!all.needed(), "a relaunch with everything in place skips it");
        assert!(LaunchFacts { mic: MicPermission::NotDetermined, ..all }.needed(), "the mic was never asked");
        assert!(LaunchFacts { accessibility: false, ..all }.needed());
        assert!(LaunchFacts { model_downloaded: false, ..all }.needed(), "a fresh data folder, or a deleted model");
        let fresh = LaunchFacts { mic: MicPermission::NotDetermined, accessibility: false, model_downloaded: false };
        assert!(fresh.needed());
        // Asked and refused: not reopened at every launch.
        for mic in [MicPermission::Denied, MicPermission::Restricted, MicPermission::Unknown] {
            assert!(!LaunchFacts { mic, ..all }.needed(), "{mic:?}");
        }
    }

    #[test]
    fn every_step_has_words() {
        for facts in [fresh(), ready()] {
            for s in Onboarding::new(&facts).steps {
                assert!(!s.text.is_empty() && !s.id.title().is_empty(), "{s:?}");
                assert!(s.text.ends_with(['.', '…']), "{:?}: {}", s.id, s.text);
            }
        }
    }
}
