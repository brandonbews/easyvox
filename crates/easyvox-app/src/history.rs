//! Transcript history: the store (easyvox-core's SQLite `History`, text only) and its window, a
//! virtual list with a copy button per row.

use crate::state::AppSettings;
use easyvox_core::HistoryEntry;
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::{ActiveTheme, Sizable, Theme, VirtualListScrollHandle, h_flex, v_flex, v_virtual_list};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::path::Path;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

/// How many transcripts the window loads.
const LOAD_LIMIT: usize = 500;

/// Newest first.
pub struct HistoryStore {
    pub entries: Vec<HistoryEntry>,
    db: Option<easyvox_core::History>,
    /// Why the database couldn't be opened or written, shown in the window.
    pub error: Option<String>,
    /// Ids for entries kept in memory only, when the database is unavailable.
    next_memory_id: i64,
}

impl HistoryStore {
    pub fn open(path: &Path) -> Self {
        let mut store = Self { entries: vec![], db: None, error: None, next_memory_id: -1 };
        match easyvox_core::History::open(path).and_then(|db| Ok((db.recent(LOAD_LIMIT)?, db))) {
            Ok((entries, db)) => {
                log::info!("history: {} transcripts in {}", entries.len(), path.display());
                store.entries = entries;
                store.db = Some(db);
            }
            Err(e) => {
                log::error!("history: couldn't open {}: {e:#}", path.display());
                store.error = Some(format!("{e:#}"));
            }
        }
        store
    }

    pub fn push(&mut self, text: String, audio_secs: f32, model_id: &str, cx: &mut Context<Self>) {
        let saved = self.db.as_ref().map(|db| db.add(&text, audio_secs, model_id));
        let entry = match saved {
            Some(Ok(entry)) => entry,
            other => {
                if let Some(Err(e)) = other {
                    log::error!("history: couldn't save a transcript: {e:#}");
                    self.error = Some(format!("{e:#}"));
                }
                let id = self.next_memory_id;
                self.next_memory_id -= 1;
                HistoryEntry { id, text, created_at_ms: now_ms(), duration_secs: audio_secs, model_id: model_id.into() }
            }
        };
        self.entries.insert(0, entry);
        cx.notify();
    }
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn ago(created_at_ms: i64) -> String {
    let secs = (now_ms() - created_at_ms).max(0) / 1000;
    match secs {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=86399 => format!("{} h ago", secs / 3600),
        _ => format!("{} d ago", secs / 86400),
    }
}

fn model_name(id: &str) -> &str {
    easyvox_core::models::find(id).map_or(id, |m| m.name)
}

const ROW_HEIGHT: f32 = 56.;

pub struct HistoryView {
    store: Entity<HistoryStore>,
    scroll: VirtualListScrollHandle,
    item_sizes: Rc<Vec<Size<Pixels>>>,
    _subscriptions: [Subscription; 2],
}

impl HistoryView {
    pub fn new(store: Entity<HistoryStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let rows = store.read(cx).entries.len();
        let on_change = cx.observe(&store, |this, store, cx| {
            this.item_sizes = row_sizes(store.read(cx).entries.len());
            cx.notify();
        });
        let on_appearance =
            cx.observe_window_appearance(window, |_, window, cx| Theme::sync_system_appearance(Some(window), cx));
        Self {
            store,
            scroll: VirtualListScrollHandle::new(),
            item_sizes: row_sizes(rows),
            _subscriptions: [on_change, on_appearance],
        }
    }

    fn render_row(entry: &HistoryEntry, cx: &App) -> impl IntoElement + use<> {
        h_flex()
            .id(ElementId::Name(format!("row-{}", entry.id).into()))
            .h(px(ROW_HEIGHT))
            .w_full()
            .px_4()
            .gap_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                v_flex().flex_1().overflow_hidden().child(div().text_sm().truncate().child(entry.text.clone())).child(
                    div().text_xs().text_color(cx.theme().muted_foreground).child(format!(
                        "{} · {:.1} s · {}",
                        ago(entry.created_at_ms),
                        entry.duration_secs,
                        model_name(&entry.model_id)
                    )),
                ),
            )
            .child(
                Clipboard::new(ElementId::Name(format!("copy-{}", entry.id).into()))
                    .value(SharedString::from(entry.text.clone()))
                    .tooltip("Copy")
                    .small(),
            )
    }

    fn render_empty(cx: &App) -> impl IntoElement + use<> {
        let shortcut = Keystroke::parse(&cx.global::<AppSettings>().shortcut).ok();
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .gap_2()
            .px_8()
            .child(div().text_base().font_weight(FontWeight::MEDIUM).child("No transcripts yet"))
            .child(
                h_flex()
                    .gap_1()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Hold")
                    .when_some(shortcut, |this, ks| this.child(Kbd::new(ks)))
                    .child("and speak. Everything you dictate shows up here."),
            )
    }
}

fn row_sizes(rows: usize) -> Rc<Vec<Size<Pixels>>> {
    Rc::new(vec![size(px(100.), px(ROW_HEIGHT)); rows])
}

impl Render for HistoryView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.store.read(cx);
        let count = store.entries.len();
        let error = store.error.clone();
        let header = h_flex()
            .px_4()
            .py_3()
            .justify_between()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("History"))
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(match count {
                1 => "1 transcript".to_string(),
                n => format!("{n} transcripts"),
            }));

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(header)
            .when_some(error, |this, e| {
                this.child(
                    div().px_4().py_2().text_sm().text_color(cx.theme().danger).child(format!("History problem: {e}")),
                )
            })
            .map(|this| {
                if count == 0 {
                    return this.child(Self::render_empty(cx));
                }
                this.child(
                    div()
                        .relative()
                        .flex_1()
                        .overflow_hidden()
                        .child(
                            v_virtual_list(
                                cx.entity().clone(),
                                "history-list",
                                self.item_sizes.clone(),
                                |this, range, _window, cx| {
                                    let entries = &this.store.read(cx).entries;
                                    range
                                        .filter_map(|ix| entries.get(ix))
                                        .map(|entry| Self::render_row(entry, cx))
                                        .collect::<Vec<_>>()
                                },
                            )
                            .track_scroll(&self.scroll),
                        )
                        .child(Scrollbar::vertical(&self.scroll)),
                )
            })
    }
}
