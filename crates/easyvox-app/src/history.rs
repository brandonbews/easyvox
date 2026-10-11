//! Transcript history: the store (easyvox-core's SQLite `History`, text only) and its window.
//!
//! The window is the design's (`design/`, boards 16–17) without what the core doesn't keep yet:
//! dictations newest first, grouped by day ("Today", "Yesterday", "Wednesday 7 October"), each
//! with its time, its word count and a copy button; a footer with how many there are and where
//! they're kept; and when there are none, the knot and how to make one. The day, the time and the
//! word count come from what the core stores (`created_at_ms` and the text). The rows are a
//! virtual list, so only the visible ones are drawn: each row is one or two lines of text, picked
//! by measuring the text once.

use crate::brand::{self, Colors};
use crate::state::AppSettings;
use easyvox_core::HistoryEntry;
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::{ActiveTheme, Sizable, Theme, VirtualListScrollHandle, h_flex, v_flex, v_virtual_list};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::collections::HashMap;
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
    /// The database had more than were loaded.
    more_in_db: bool,
    /// Ids for entries kept in memory only, when the database is unavailable.
    next_memory_id: i64,
}

impl HistoryStore {
    pub fn open(path: &Path) -> Self {
        let mut store = Self { entries: vec![], db: None, error: None, more_in_db: false, next_memory_id: -1 };
        match easyvox_core::History::open(path).and_then(|db| Ok((db.recent(LOAD_LIMIT)?, db))) {
            Ok((entries, db)) => {
                log::info!("history: {} transcripts in {}", entries.len(), path.display());
                store.more_in_db = entries.len() == LOAD_LIMIT;
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

// ───────────────────────── The list, as data ─────────────────────────

/// What the list shows, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Item {
    /// A day's title: the local day (days since 1 January 1970), and its newest dictation's time.
    Day { day: i64, ms: i64 },
    /// A dictation, by its index in the store, and where it sits in its day's group.
    Row { ix: usize, first: bool, last: bool, two_lines: bool },
    /// Room under the last group, above the footer.
    End,
}

/// `entries` (newest first) as the list shows them: each day's title, then that day's
/// dictations. `day_of` gives a time's local day.
fn layout(entries: &[HistoryEntry], day_of: impl Fn(i64) -> i64) -> Vec<Item> {
    let mut items = vec![];
    let mut start = 0;
    while start < entries.len() {
        let day = day_of(entries[start].created_at_ms);
        let end =
            (start + 1..entries.len()).find(|&i| day_of(entries[i].created_at_ms) != day).unwrap_or(entries.len());
        items.push(Item::Day { day, ms: entries[start].created_at_ms });
        items.extend((start..end).map(|ix| Item::Row {
            ix,
            first: ix == start,
            last: ix + 1 == end,
            two_lines: false,
        }));
        start = end;
    }
    items
}

/// The local day of a time, given the local time zone's offset from UTC then.
fn local_day(ms: i64, offset_secs: i64) -> i64 {
    (ms.div_euclid(1000) + offset_secs).div_euclid(86_400)
}

/// "Today", "Yesterday", or the date, which `date` writes.
fn day_title(day: i64, today: i64, date: impl FnOnce() -> String) -> String {
    match today - day {
        0 => "Today".into(),
        1 => "Yesterday".into(),
        _ => date(),
    }
}

/// "1 word", "20 words".
fn word_count(text: &str) -> String {
    match text.split_whitespace().count() {
        1 => "1 word".into(),
        n => format!("{n} words"),
    }
}

/// The footer: how many dictations, and where they're kept.
fn footer_words(count: usize, more_in_db: bool) -> String {
    const WHERE: &str = "Stored only on this Mac";
    match (count, more_in_db) {
        (0, _) => "History is stored only on this Mac".into(),
        (n, true) => format!("The latest {n} dictations · {WHERE}"),
        (1, false) => format!("1 dictation · {WHERE}"),
        (n, false) => format!("{n} dictations · {WHERE}"),
    }
}

// Heights in the list, in points (board 16): a day's title, and a row of one or two lines of
// text above its word count. The first row of a day adds the group's top edge, the last its
// bottom, and every other row the hairline above it.
const DAY_TITLE: f32 = 40.;
const ROW_ONE_LINE: f32 = 10. + 18. + 4. + 14. + 10.;
const ROW_TWO_LINES: f32 = ROW_ONE_LINE + 18.;
const GROUP_EDGE: f32 = 4. + 1.;
const SEPARATOR: f32 = 1.;
/// The time's column: room for "10:41 PM" where the region writes 12-hour times.
const TIME: f32 = 56.;
/// What a row's text doesn't get of the window's width: the window's margins (20 each side), the
/// group's padding (12 each side) and edges, the time and the copy button (24), with the gaps
/// between them (12 each).
const ROW_CHROME: f32 = 20. * 2. + 12. * 2. + 2. + TIME + 24. + 12. * 2.;

fn height(item: &Item) -> f32 {
    match *item {
        Item::Day { .. } => DAY_TITLE,
        Item::End => 16.,
        Item::Row { first, last, two_lines, .. } => {
            let text = if two_lines { ROW_TWO_LINES } else { ROW_ONE_LINE };
            text + if first { GROUP_EDGE } else { SEPARATOR } + if last { GROUP_EDGE } else { 0. }
        }
    }
}

// ───────────────────────── Local time ─────────────────────────

/// Times and dates the way this Mac's language and region write them ("9:41" or "9:41 AM";
/// "Wednesday 7 October" or "Wednesday, October 7"), through Foundation's formatters. The core
/// stores UTC milliseconds; the local time zone is asked at each use, so a trip abroad shows.
#[cfg(target_os = "macos")]
mod clock {
    use objc2::rc::Retained;
    use objc2_foundation::{NSDate, NSDateFormatter, NSString, NSTimeZone};

    pub struct Clock {
        time: Retained<NSDateFormatter>,
        date: Retained<NSDateFormatter>,
        date_with_year: Retained<NSDateFormatter>,
        year: Retained<NSDateFormatter>,
    }

    impl Clock {
        pub fn new() -> Self {
            let formatter = |template: &str| {
                let formatter = NSDateFormatter::new();
                formatter.setLocalizedDateFormatFromTemplate(&NSString::from_str(template));
                formatter
            };
            Self {
                // `j`: the hour as the region writes it, 12- or 24-hour.
                time: formatter("jmm"),
                date: formatter("EEEEdMMMM"),
                date_with_year: formatter("EEEEdMMMMy"),
                year: formatter("y"),
            }
        }

        /// The local time zone's offset from UTC at `ms`, in seconds.
        pub fn offset_secs(&self, ms: i64) -> i64 {
            NSTimeZone::localTimeZone().secondsFromGMTForDate(&date(ms)) as i64
        }

        pub fn time(&self, ms: i64) -> String {
            self.time.stringFromDate(&date(ms)).to_string()
        }

        /// The date, with the year when it isn't the year of `now_ms`.
        pub fn date(&self, ms: i64, now_ms: i64) -> String {
            let (then, now) = (date(ms), date(now_ms));
            let same_year = self.year.stringFromDate(&then) == self.year.stringFromDate(&now);
            let formatter = if same_year { &self.date } else { &self.date_with_year };
            formatter.stringFromDate(&then).to_string()
        }
    }

    fn date(ms: i64) -> Retained<NSDate> {
        NSDate::dateWithTimeIntervalSince1970(ms as f64 / 1000.)
    }
}

#[cfg(target_os = "macos")]
use clock::Clock;

// ───────────────────────── The window ─────────────────────────

pub struct HistoryView {
    store: Entity<HistoryStore>,
    scroll: VirtualListScrollHandle,
    clock: Clock,
    /// What the list shows and each item's size. Built again when it may have changed: a new
    /// dictation, a new day, another window width.
    items: Rc<Vec<Item>>,
    item_sizes: Rc<Vec<Size<Pixels>>>,
    built_for: Option<(usize, Option<i64>, i64, i32)>,
    /// Each dictation's width on one line, by id, measured once: whether it needs two.
    widths: HashMap<i64, Pixels>,
    _subscriptions: [Subscription; 2],
}

impl HistoryView {
    pub fn new(store: Entity<HistoryStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let on_change = cx.observe(&store, |_, _, cx| cx.notify());
        let on_appearance =
            cx.observe_window_appearance(window, |_, window, cx| Theme::sync_system_appearance(Some(window), cx));
        Self {
            store,
            scroll: VirtualListScrollHandle::new(),
            clock: Clock::new(),
            items: Rc::default(),
            item_sizes: Rc::default(),
            built_for: None,
            widths: HashMap::new(),
            _subscriptions: [on_change, on_appearance],
        }
    }

    /// Lays the list out again if the dictations, the day or the width changed since.
    fn lay_out(&mut self, window: &Window, cx: &App) {
        let entries = &self.store.read(cx).entries;
        let now = now_ms();
        let today = local_day(now, self.clock.offset_secs(now));
        let text_width = window.viewport_size().width - px(ROW_CHROME);
        let key = (entries.len(), entries.first().map(|e| e.id), today, f32::from(text_width) as i32);
        if self.built_for == Some(key) {
            return;
        }
        let clock = &self.clock;
        let mut items = layout(entries, |ms| local_day(ms, clock.offset_secs(ms)));
        items.push(Item::End);
        let font = font(cx.theme().font_family.clone());
        for item in &mut items {
            if let Item::Row { ix, two_lines, .. } = item {
                let entry = &entries[*ix];
                let width = *self.widths.entry(entry.id).or_insert_with(|| one_line_width(&entry.text, &font, window));
                *two_lines = entry.text.contains('\n') || width > text_width;
            }
        }
        self.item_sizes = Rc::new(items.iter().map(|item| size(px(100.), px(height(item)))).collect());
        self.items = Rc::new(items);
        self.built_for = Some(key);
    }

    fn render_item(&self, item: Item, c: &'static Colors, cx: &App) -> AnyElement {
        match item {
            Item::End => div().w_full().h(px(height(&item))).into_any_element(),
            Item::Day { day, ms } => {
                let now = now_ms();
                let today = local_day(now, self.clock.offset_secs(now));
                let title = day_title(day, today, || self.clock.date(ms, now));
                div()
                    .w_full()
                    .h(px(DAY_TITLE))
                    .flex()
                    .items_end()
                    .px(px(32.))
                    .pb(px(8.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title)
                    .into_any_element()
            }
            Item::Row { ix, first, last, two_lines } => {
                let Some(entry) = self.store.read(cx).entries.get(ix) else { return div().into_any_element() };
                let text = div()
                    .text_size(brand::TEXT)
                    .line_height(px(18.))
                    .line_clamp(if two_lines { 2 } else { 1 })
                    .text_ellipsis()
                    .child(entry.text.clone());
                let row = h_flex()
                    .items_start()
                    .gap_3()
                    .py(px(10.))
                    .child(
                        v_flex().flex_1().min_w_0().child(text).child(
                            div()
                                .mt(px(4.))
                                .text_size(brand::TEXT_SMALL)
                                .line_height(px(14.))
                                .text_color(rgb(c.ink2))
                                .child(word_count(&entry.text)),
                        ),
                    )
                    .child(
                        div()
                            .w(px(TIME))
                            .flex_shrink_0()
                            .text_right()
                            .text_size(brand::TEXT_KEY)
                            .line_height(px(18.))
                            .text_color(rgb(c.ink2))
                            .child(self.clock.time(entry.created_at_ms)),
                    )
                    .child(
                        Clipboard::new(ElementId::Name(format!("copy-{}", entry.id).into()))
                            .value(SharedString::from(entry.text.clone()))
                            .tooltip("Copy")
                            .small(),
                    );
                // The day's group is drawn a row at a time: its sides on every row, its top
                // edge and corners on the first, its bottom on the last.
                let group = v_flex()
                    .size_full()
                    .px_3()
                    .bg(rgb(c.group))
                    .border_color(rgb(c.group_border))
                    .border_l_1()
                    .border_r_1()
                    .map(|this| {
                        if first {
                            this.border_t_1().rounded_t(px(10.)).pt(px(4.))
                        } else {
                            this.child(div().h(px(SEPARATOR)).w_full().bg(rgb(c.hairline)))
                        }
                    })
                    .when(last, |this| this.border_b_1().rounded_b(px(10.)).pb(px(4.)))
                    .child(row);
                div()
                    .id(ElementId::Name(format!("row-{}", entry.id).into()))
                    .w_full()
                    .h(px(height(&item)))
                    .px(px(20.))
                    .child(group)
                    .into_any_element()
            }
        }
    }

    fn render_empty(c: &'static Colors, cx: &App) -> impl IntoElement + use<> {
        let shortcut = Keystroke::parse(&cx.global::<AppSettings>().shortcut).ok();
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .px(px(40.))
            .pb(px(24.))
            .child(brand::glyph(brand::KNOT, px(30.), c.ink2))
            .child(
                div()
                    .mt(px(16.))
                    .text_size(brand::TEXT_TITLE)
                    .line_height(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("No dictations yet"),
            )
            .child(
                h_flex()
                    .mt(px(6.))
                    .gap(px(5.))
                    .text_color(rgb(c.ink2))
                    .child("Hold")
                    .when_some(shortcut, |this, ks| this.child(brand::keycap(c, Kbd::format(&ks))))
                    .child("in any app and talk. What you say shows up here."),
            )
    }
}

/// `text`'s width on one line at the list's size.
fn one_line_width(text: &str, font: &Font, window: &Window) -> Pixels {
    let line: SharedString = text.replace('\n', " ").into();
    let run = TextRun {
        len: line.len(),
        font: font.clone(),
        color: black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window.text_system().shape_line(line, brand::TEXT, &[run], None).width()
}

impl Render for HistoryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = brand::current(cx);
        self.lay_out(window, cx);
        let store = self.store.read(cx);
        let count = store.entries.len();
        let footer = footer_words(count, store.more_in_db);
        let error = store.error.clone();

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_size(brand::TEXT)
            .when_some(error, |this, e| {
                this.child(div().px(px(32.)).pt(px(12.)).child(brand::problem(
                    c,
                    format!("Can't use the history file, so dictations are kept only until EasyVox quits. ({e})"),
                    brand::TEXT_SMALL,
                )))
            })
            .map(|this| {
                if count == 0 {
                    return this.child(Self::render_empty(c, cx));
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
                                move |this, range, _window, cx| {
                                    let items = this.items.clone();
                                    range
                                        .filter_map(|ix| items.get(ix).copied())
                                        .map(|item| this.render_item(item, c, cx))
                                        .collect::<Vec<_>>()
                                },
                            )
                            .track_scroll(&self.scroll),
                        )
                        .child(Scrollbar::vertical(&self.scroll)),
                )
            })
            .child(
                h_flex()
                    .flex_shrink_0()
                    .h(px(30.))
                    .justify_center()
                    .border_t_1()
                    .border_color(rgb(c.hairline))
                    .text_size(brand::TEXT_SMALL)
                    .text_color(rgb(c.ink2))
                    .child(footer),
            )
    }
}

#[cfg(test)]
mod tests {
    // Not `super::*`: that brings in GPUI's own `test` attribute through `gpui_kit::*`.
    use super::{
        GROUP_EDGE, HistoryEntry, Item, ROW_ONE_LINE, ROW_TWO_LINES, SEPARATOR, day_title, footer_words, height,
        layout, local_day, word_count,
    };

    const HOUR: i64 = 3_600_000;
    const DAY: i64 = 24 * HOUR;

    /// An invented dictation at `ms`.
    fn entry(id: i64, ms: i64) -> HistoryEntry {
        HistoryEntry {
            id,
            text: "Pick up the blue folder on the way in.".into(),
            created_at_ms: ms,
            duration_secs: 2.,
            model_id: "test".into(),
        }
    }

    fn row(ix: usize, first: bool, last: bool) -> Item {
        Item::Row { ix, first, last, two_lines: false }
    }

    #[test]
    fn dictations_are_grouped_by_local_day_newest_first() {
        // 10 October 2026, 23:30 UTC is already the 11th two hours east of UTC.
        let late = 1_791_675_000_000;
        let offset = 2 * 3600;
        let entries = [entry(4, late), entry(3, late - HOUR), entry(2, late - 23 * HOUR), entry(1, late - 3 * DAY)];
        let day = |ms| local_day(ms, offset);
        let items = layout(&entries, day);
        assert_eq!(
            items,
            [
                Item::Day { day: day(late), ms: late },
                row(0, true, false),
                row(1, false, true),
                Item::Day { day: day(late - 23 * HOUR), ms: late - 23 * HOUR },
                row(2, true, true),
                Item::Day { day: day(late - 3 * DAY), ms: late - 3 * DAY },
                row(3, true, true),
            ]
        );
        assert_eq!(day(late) - day(late - 23 * HOUR), 1);
        // The same two dictations in UTC: the same day.
        assert_eq!(local_day(late, 0), local_day(late - HOUR, 0));
        assert!(layout(&[], day).is_empty());
    }

    #[test]
    fn days_are_local_days() {
        assert_eq!(local_day(0, 0), 0);
        assert_eq!(local_day(-1, 0), -1, "before 1970 rounds down");
        assert_eq!(local_day(DAY - 1, 0), 0);
        assert_eq!(local_day(DAY - HOUR, 3600), 1, "an hour east, it's already tomorrow");
        assert_eq!(local_day(HOUR / 2, -3600), -1, "an hour west, it's still yesterday");
    }

    #[test]
    fn day_titles_name_today_and_yesterday_and_date_the_rest() {
        let date = || "Wednesday 7 October".to_string();
        assert_eq!(day_title(100, 100, date), "Today");
        assert_eq!(day_title(99, 100, date), "Yesterday");
        assert_eq!(day_title(96, 100, date), "Wednesday 7 October");
        assert_eq!(day_title(101, 100, date), "Wednesday 7 October", "a clock set back");
    }

    #[test]
    fn counts_read_in_words() {
        assert_eq!(word_count("Hello"), "1 word");
        assert_eq!(word_count("  Pick up the\nblue folder. "), "5 words");
        assert_eq!(footer_words(0, false), "History is stored only on this Mac");
        assert_eq!(footer_words(1, false), "1 dictation · Stored only on this Mac");
        assert_eq!(footer_words(214, false), "214 dictations · Stored only on this Mac");
        assert_eq!(footer_words(500, true), "The latest 500 dictations · Stored only on this Mac");
    }

    #[test]
    fn rows_are_as_tall_as_their_lines_and_place_in_the_group() {
        let two = Item::Row { ix: 0, first: false, last: false, two_lines: true };
        assert_eq!(height(&row(0, false, false)), ROW_ONE_LINE + SEPARATOR);
        assert_eq!(height(&two), ROW_TWO_LINES + SEPARATOR);
        assert_eq!(height(&row(0, true, true)), ROW_ONE_LINE + 2. * GROUP_EDGE);
        assert_eq!(height(&row(0, true, false)), ROW_ONE_LINE + GROUP_EDGE);
        assert_eq!(height(&row(0, false, true)), ROW_ONE_LINE + SEPARATOR + GROUP_EDGE);
    }
}
