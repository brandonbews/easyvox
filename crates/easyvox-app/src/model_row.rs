//! One offered model, the way Settings and onboarding list it: its name, size and "pick this
//! if", what you can do with it now (Download with progress and Cancel; Use; Delete), and
//! its credits: license, creator, the modification note and the model card, with links that open
//! in the browser.
//!
//! [`ModelRow`] is a plain element. It reads the app's state from the globals (never from the
//! controller, which may be mid-update while a window draws) and acts through
//! [`controller::with`], so any window can show it. Its only state of its own, the "Delete?"
//! confirmation and a delete error, lives in the window, keyed by the model's id. It's drawn in
//! the windows' look (`brand.rs`): pill buttons, vermilion only for Download, and problems in
//! words with a thin glyph.

use crate::brand;
use crate::controller;
use crate::state::{AppSettings, AppStatus, DownloadStatus, EngineStatus};
use easyvox_core::download::{DownloadError, Progress, format_size};
use easyvox_core::models::ModelInfo;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::link::Link;
use gpui_kit::component::progress::Progress as ProgressBar;
use gpui_kit::component::{Disableable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::collections::HashMap;
use std::path::Path;

/// Where a model stands. Worked out from the app's state apart from drawing, so it can be tested.
#[derive(Clone, Debug, PartialEq)]
pub enum ModelState {
    NotDownloaded,
    Downloading(Progress),
    /// The last download failed, for this reason. Nothing was kept.
    Failed(DownloadError),
    /// On this Mac, taking this many bytes (the model and its license notice).
    Downloaded {
        bytes: u64,
    },
}

impl ModelState {
    pub fn of(model: &ModelInfo, downloads: &HashMap<&'static str, DownloadStatus>, models_dir: &Path) -> Self {
        match downloads.get(model.id) {
            Some(DownloadStatus::Running(progress)) => Self::Downloading(*progress),
            Some(DownloadStatus::Failed(error)) => Self::Failed(error.clone()),
            None if model.is_available(models_dir) => Self::Downloaded { bytes: disk_usage(model, models_dir) },
            None => Self::NotDownloaded,
        }
    }
}

/// The bytes a downloaded model takes: its file and its license notice.
fn disk_usage(model: &ModelInfo, models_dir: &Path) -> u64 {
    [model.resolve(models_dir), model.license_notice_path(models_dir)]
        .iter()
        .filter_map(|path| std::fs::metadata(path).ok())
        .map(|m| m.len())
        .sum()
}

/// A download's state in a few words: "Downloading… 120 MB of 539 MB".
pub fn download_words(status: &DownloadStatus) -> String {
    match status {
        DownloadStatus::Running(progress) => progress_words(progress),
        DownloadStatus::Failed(error) => format!("Download failed: {error}"),
    }
}

fn progress_words(progress: &Progress) -> String {
    match progress {
        Progress::Verifying => "Checking the copy already downloaded…".into(),
        Progress::Downloading { done, total } => {
            format!("Downloading… {} of {}", format_size(*done), format_size(*total))
        }
        Progress::Reconnecting { attempt, max, done, total } => format!(
            "Connection lost at {} of {}. Trying again ({attempt} of {max})…",
            format_size(*done),
            format_size(*total)
        ),
    }
}

/// How far a download is, in percent, for the progress bar.
fn percent(progress: &Progress) -> f32 {
    match progress {
        Progress::Verifying => 0.,
        Progress::Downloading { done, total } | Progress::Reconnecting { done, total, .. } => {
            (*done as f64 * 100. / (*total).max(1) as f64) as f32
        }
    }
}

/// How many rows were drawn: the self-test checks that the Models page really draws them.
#[cfg(feature = "self-test")]
pub static DRAWN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The row's own state: whether "Delete" is waiting for its confirmation, and why the last
/// delete failed.
#[derive(Default)]
struct RowUi {
    confirm_delete: bool,
    error: Option<String>,
}

/// One model, with its state, its actions and its credits.
#[derive(IntoElement)]
pub struct ModelRow {
    model: &'static ModelInfo,
    /// Download is the vermilion button.
    emphasised: bool,
}

impl ModelRow {
    pub fn new(model: &'static ModelInfo) -> Self {
        Self { model, emphasised: true }
    }

    /// Whether Download is the vermilion button (it is unless this says otherwise). Onboarding
    /// keeps vermilion for the step to do next.
    pub fn emphasised(mut self, emphasised: bool) -> Self {
        self.emphasised = emphasised;
        self
    }
}

impl RenderOnce for ModelRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        #[cfg(feature = "self-test")]
        DRAWN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let model = self.model;
        let id = model.id;
        let key = |part: &str| ElementId::Name(format!("model-{id}-{part}").into());
        let status = cx.global::<AppStatus>();
        let state = ModelState::of(model, &status.downloads, &status.models_dir);
        let active = cx.global::<AppSettings>().model == id;
        let engine = cx.global::<AppStatus>().engine.clone();
        let ui = window.use_keyed_state(key("ui"), cx, |_, _| RowUi::default());
        let (confirm_delete, delete_error) = {
            let ui = ui.read(cx);
            (ui.confirm_delete, ui.error.clone())
        };
        let c = brand::current(cx);
        let muted: Hsla = rgb(c.ink2).into();

        let size = match &state {
            ModelState::Downloaded { bytes } => format!("{} on this Mac", format_size(*bytes)),
            _ => format!("{} download", format_size(model.download_bytes)),
        };
        let engine_note = match (&engine, active, &state) {
            (EngineStatus::Loading, true, ModelState::Downloaded { .. }) => Some("Loading…"),
            _ => None,
        };
        let error = match (&state, &engine, active) {
            (ModelState::Failed(e), _, _) => Some(format!("Download failed: {e}")),
            (ModelState::Downloaded { .. }, EngineStatus::Failed { error }, true) => {
                Some(format!("Couldn't load: {error}"))
            }
            _ => delete_error,
        };

        let actions = h_flex().flex_shrink_0().gap_2().map(|this| match &state {
            ModelState::Downloading(_) => {
                this.child(brand::button(key("cancel"), "Cancel").on_click(move |_, _, cx| {
                    controller::with(cx, |c, _| c.cancel_download(id));
                }))
            }
            ModelState::NotDownloaded | ModelState::Failed(_) => {
                let label = if matches!(state, ModelState::Failed(_)) { "Try again" } else { "Download" };
                let ui = ui.clone();
                let button = if self.emphasised {
                    brand::primary(key("download"), label)
                } else {
                    brand::button(key("download"), label)
                };
                this.child(button.on_click(move |_, _, cx| {
                    ui.update(cx, |ui, cx| {
                        *ui = RowUi::default();
                        cx.notify();
                    });
                    controller::with(cx, |c, cx| c.download_model(id, cx));
                }))
            }
            ModelState::Downloaded { .. } if active => this.child(
                brand::button(key("delete"), "Delete")
                    .disabled(true)
                    .tooltip("This is the model in use. Use another one first."),
            ),
            ModelState::Downloaded { bytes } if confirm_delete => {
                let (keep, delete) = (ui.clone(), ui.clone());
                this.child(brand::button(key("keep"), "Keep").on_click(move |_, _, cx| {
                    keep.update(cx, |ui, cx| {
                        ui.confirm_delete = false;
                        cx.notify();
                    });
                }))
                .child(
                    Button::new(key("confirm-delete"))
                        .danger()
                        .small()
                        .rounded(px(12.))
                        .px_3()
                        .label(format!("Delete {}", format_size(*bytes)))
                        .on_click(move |_, _, cx| {
                            let result = controller::with(cx, |c, cx| c.delete_model(id, cx))
                                .unwrap_or(Err("EasyVox isn't ready.".into()));
                            delete.update(cx, |ui, cx| {
                                ui.confirm_delete = false;
                                ui.error = result.err();
                                cx.notify();
                            });
                        }),
                )
            }
            ModelState::Downloaded { .. } => {
                let (use_it, ask) = (ui.clone(), ui.clone());
                this.child(brand::button(key("use"), "Use").on_click(move |_, _, cx| {
                    use_it.update(cx, |ui, cx| {
                        *ui = RowUi::default();
                        cx.notify();
                    });
                    controller::with(cx, |c, cx| c.select_model(id, cx));
                }))
                .child(brand::button(key("delete"), "Delete").on_click(move |_, _, cx| {
                    ask.update(cx, |ui, cx| {
                        ui.confirm_delete = true;
                        ui.error = None;
                        cx.notify();
                    });
                }))
            }
        });

        let heading = v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(2.))
            .child(
                h_flex()
                    .gap_2()
                    .child(div().text_size(brand::TEXT).font_weight(FontWeight::MEDIUM).child(model.name))
                    .when(active && matches!(state, ModelState::Downloaded { .. }), |this| {
                        // A quiet label, not a colour: vermilion is for actions.
                        this.child(
                            h_flex()
                                .h(px(18.))
                                .px(px(7.))
                                .rounded(px(9.))
                                .bg(rgb(c.keycap))
                                .text_size(brand::TEXT_SMALL)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(c.ink))
                                .child("In use"),
                        )
                    })
                    .when_some(engine_note, |this, note| {
                        this.child(div().text_size(brand::TEXT_SMALL).text_color(muted).child(note))
                    }),
            )
            .child(
                div()
                    .text_size(brand::TEXT_SMALL)
                    .line_height(px(15.))
                    .text_color(muted)
                    .child(format!("{size} · Pick this if {}", model.pick_if)),
            );

        v_flex()
            .w_full()
            .gap_2()
            .child(h_flex().w_full().items_start().justify_between().gap_4().child(heading).child(actions))
            .when_some(
                match &state {
                    ModelState::Downloading(progress) => Some(*progress),
                    _ => None,
                },
                |this, progress| {
                    this.child(
                        h_flex()
                            .gap_3()
                            .child(
                                ProgressBar::new(key("progress"))
                                    .small()
                                    .w(px(220.))
                                    .value(percent(&progress))
                                    .loading(progress == Progress::Verifying),
                            )
                            .child(
                                div().text_size(brand::TEXT_SMALL).text_color(muted).child(progress_words(&progress)),
                            ),
                    )
                },
            )
            .when_some(error, |this, e| this.child(brand::problem(c, e, brand::TEXT_SMALL)))
            .child(credits(model, muted))
    }
}

/// The model's credits in two small lines, with links: who made it and under which license;
/// how the file was changed and by whom, and the model card.
fn credits(model: &'static ModelInfo, color: Hsla) -> impl IntoElement {
    let id = model.id;
    v_flex()
        .text_size(brand::TEXT_SMALL)
        .line_height(px(15.))
        .text_color(color)
        .child(linked_text(format!("{id}-credit"), &format!("{}.", credit_with_link(model)), px(3.)))
        .child(linked_text(
            format!("{id}-changes"),
            &format!("{}. [Model card]({})", modification_with_link(model), model.card_url),
            px(3.),
        ))
}

/// The attribution with the license's name linked to its text: "… licensed under
/// [CC BY 4.0](https://…)".
pub fn credit_with_link(model: &ModelInfo) -> String {
    let name = model.license.name;
    match model.attribution.split_once(name) {
        Some((before, after)) => format!("{before}[{name}]({}){after}", model.license.url),
        None => format!("{} ([{name}]({}))", model.attribution, model.license.url),
    }
}

/// The modification note with who made it linked to their files: "… by
/// [handy-computer](https://huggingface.co/handy-computer/…)".
pub fn modification_with_link(model: &ModelInfo) -> String {
    let who = model.modified_by();
    match model.modification.strip_suffix(who).filter(|_| !who.is_empty()) {
        Some(before) => format!("{before}[{who}]({})", model.source_url()),
        None => model.modification.to_string(),
    }
}

/// A line of text with links in it, written `[text](url)`. It wraps between words, like a
/// paragraph; `space` is the gap a space leaves at the parent's text size. Links are GPUI Kit
/// `Link`s, which open in the browser.
pub fn linked_text(id: impl Into<SharedString>, source: &str, space: Pixels) -> Div {
    let id: SharedString = id.into();
    // Words are what wraps, so punctuation stays with the link before it ("4.0." stays whole).
    let mut words: Vec<Vec<(String, Option<String>)>> = vec![vec![]];
    for (text, url) in segments(source) {
        for (i, piece) in text.split(' ').enumerate() {
            if i > 0 {
                words.push(vec![]);
            }
            if !piece.is_empty()
                && let Some(word) = words.last_mut()
            {
                word.push((piece.to_string(), url.clone()));
            }
        }
    }
    h_flex().flex_wrap().gap_x(space).children(words.into_iter().filter(|w| !w.is_empty()).enumerate().map(
        |(i, word)| {
            h_flex().children(word.into_iter().enumerate().map(|(j, (text, url))| match url {
                Some(url) => {
                    Link::new(ElementId::Name(format!("{id}-{i}-{j}").into())).href(url).child(text).into_any_element()
                }
                None => div().child(text).into_any_element(),
            }))
        },
    ))
}

/// `source` cut into plain text and `[text](url)` links, in order.
fn segments(source: &str) -> Vec<(String, Option<String>)> {
    let mut out = vec![];
    let mut rest = source;
    while let Some(open) = rest.find('[') {
        let link = rest[open + 1..].split_once("](").and_then(|(text, after)| {
            let (url, tail) = after.split_once(')')?;
            (!text.contains('[')).then_some((text, url, tail))
        });
        let Some((text, url, tail)) = link else { break };
        if open > 0 {
            out.push((rest[..open].to_string(), None));
        }
        out.push((text.to_string(), Some(url.to_string())));
        rest = tail;
    }
    if !rest.is_empty() {
        out.push((rest.to_string(), None));
    }
    out
}

#[cfg(test)]
mod tests {
    // Not `super::*`: that brings in GPUI's own `test` attribute through `gpui_kit::*`.
    use super::{
        DownloadError, DownloadStatus, HashMap, ModelState, Progress, credit_with_link, download_words,
        modification_with_link, percent, progress_words, segments,
    };
    use easyvox_core::models;

    #[test]
    fn links_are_cut_out_of_the_text() {
        let seg = |text: &str, url: Option<&str>| (text.to_string(), url.map(str::to_string));
        assert_eq!(
            segments("by NVIDIA, licensed under [CC BY 4.0](https://cc/). [Model card](https://card)"),
            vec![
                seg("by NVIDIA, licensed under ", None),
                seg("CC BY 4.0", Some("https://cc/")),
                seg(". ", None),
                seg("Model card", Some("https://card")),
            ]
        );
        assert_eq!(segments("no links [here"), vec![seg("no links [here", None)]);
        assert!(segments("").is_empty());
    }

    #[test]
    fn the_credits_link_the_license_and_the_converter() {
        let v2 = models::find(easyvox_core::RECOMMENDED_MODEL).unwrap();
        assert_eq!(
            credit_with_link(v2),
            "Parakeet TDT 0.6B v2 by NVIDIA, licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)"
        );
        assert_eq!(
            modification_with_link(v2),
            "Converted to GGUF and quantized to Q5_K_M by \
             [handy-computer](https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v2-gguf)"
        );
        for m in models::offered() {
            let links = segments(&credit_with_link(m)).into_iter().filter(|(_, url)| url.is_some()).count()
                + segments(&modification_with_link(m)).into_iter().filter(|(_, url)| url.is_some()).count();
            assert_eq!(links, 2, "{}", m.id);
        }
    }

    #[test]
    fn a_models_state_comes_from_its_download_and_its_file() {
        let dir = std::env::temp_dir().join(format!("easyvox-model-row-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let m = models::find("whisper-base.en-q8_0-gguf").unwrap();
        let mut downloads = HashMap::new();
        assert_eq!(ModelState::of(m, &downloads, &dir), ModelState::NotDownloaded);

        let progress = Progress::Downloading { done: 10, total: m.download_bytes };
        downloads.insert(m.id, DownloadStatus::Running(progress));
        assert_eq!(ModelState::of(m, &downloads, &dir), ModelState::Downloading(progress));
        let timed_out = DownloadError::Network("timed out".into());
        downloads.insert(m.id, DownloadStatus::Failed(timed_out.clone()));
        assert_eq!(ModelState::of(m, &downloads, &dir), ModelState::Failed(timed_out));

        downloads.clear();
        std::fs::create_dir_all(dir.join("gguf")).unwrap();
        std::fs::File::create(m.resolve(&dir)).unwrap().set_len(m.download_bytes).unwrap(); // sparse
        std::fs::write(m.license_notice_path(&dir), "notice").unwrap();
        assert_eq!(ModelState::of(m, &downloads, &dir), ModelState::Downloaded { bytes: m.download_bytes + 6 });
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn progress_reads_in_words_and_percent() {
        let p = Progress::Downloading { done: 120_000_000, total: 539_012_608 };
        assert_eq!(progress_words(&p), "Downloading… 120 MB of 539 MB");
        assert!((percent(&p) - 22.26).abs() < 0.01);
        assert_eq!(percent(&Progress::Verifying), 0.);
        let failed = DownloadStatus::Failed(DownloadError::Http(404));
        assert_eq!(download_words(&failed), "Download failed: The server doesn't have this model (HTTP 404).");
    }
}
