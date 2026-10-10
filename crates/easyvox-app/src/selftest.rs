//! `--self-test` (feature `self-test`): runs the real app through scripted dictations and
//! checks it from the inside, for a Mac nobody is looking at (screen locked, no key presses):
//! one with the model loaded, then one right after the idle timeout freed it.
//! It writes a JSON report and exits 0 through the tray's Quit item when every check passes,
//! or 1 otherwise.
//!
//! The microphone is never opened (a test clip is replayed), ⌘V is never sent (the text is only
//! copied), and history goes to a temporary database. The settings file is a temporary one too,
//! unless `EASYVOX_CONFIG` names one; the self-test edits it to check live reloading, then puts
//! it back as it was.

use crate::controller::Controller;
use crate::overlay::Mode;
use crate::platform::{self, PanelState};
use crate::state::{AppSettings, AppStatus, EngineStatus};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The clip replayed as if spoken (about 11 s), and its transcript in `testdata/manifest.tsv`.
const CLIP: &str = "punct_samantha";
/// Release the shortcut this long before the clip ends, so the release (not the end of the
/// file) ends the session, like a person letting go.
const RELEASE_BEFORE_END: f64 = 0.3;
/// Word error rate the final text may have against the transcript.
const MAX_WER: f64 = 0.15;

#[derive(Clone)]
pub struct Options {
    out: PathBuf,
    /// Temporary folder for the history database; removed at the end.
    dir: PathBuf,
}

impl Options {
    /// `Some` when the command line has `--self-test` (and maybe `--out <file>`).
    pub fn from_args() -> Option<Self> {
        let mut args = std::env::args().skip(1);
        let (mut enabled, mut out) = (false, None);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--self-test" => enabled = true,
                "--out" => out = args.next().map(PathBuf::from),
                _ => {} // e.g. -psn_… from LaunchServices
            }
        }
        let tmp = std::env::temp_dir();
        enabled.then(|| Self {
            out: out.unwrap_or_else(|| tmp.join("easyvox-self-test.json")),
            dir: tmp.join(format!("easyvox-self-test-{}", std::process::id())),
        })
    }

    pub fn history_db(&self) -> PathBuf {
        self.dir.join("history.sqlite3")
    }

    /// `EASYVOX_CONFIG` if set, else a file in the temporary folder: never your own settings.
    pub fn config_file(&self) -> PathBuf {
        std::env::var_os("EASYVOX_CONFIG")
            .filter(|v| !v.is_empty())
            .map_or_else(|| self.dir.join("config.toml"), PathBuf::from)
    }
}

pub fn start(controller: Entity<Controller>, options: Options, cx: &mut App) {
    log::info!("self-test: starting; report goes to {}", options.out.display());
    controller.update(cx, |c, _| {
        c.source_override = Some(crate::testdata_dir().join(format!("{CLIP}.wav")));
        c.auto_paste = false;
    });
    cx.spawn(async move |cx| run(controller, options, cx).await).detach();
}

#[derive(Default)]
struct Report {
    checks: Vec<Value>,
    failed: usize,
}

impl Report {
    fn check(&mut self, name: &str, ok: bool, detail: Value) {
        if ok {
            log::info!("self-test: pass: {name}");
        } else {
            log::error!("self-test: FAIL: {name}: {detail}");
            self.failed += 1;
        }
        self.checks.push(json!({"name": name, "ok": ok, "detail": detail}));
    }
}

async fn run(controller: Entity<Controller>, options: Options, cx: &mut AsyncApp) {
    let started = Instant::now();
    let mut report = Report::default();

    let (tray_created, tray_visible) = cx.update(|cx| {
        let tray = controller.read(cx).tray.as_ref();
        (tray.is_some(), tray.and_then(|t| platform::status_item_visible(&t.icon)))
    });
    report.check(
        "menu bar icon created",
        tray_created && tray_visible == Some(true),
        json!({"status_item_visible": tray_visible}),
    );

    let (shortcut, shortcut_error, hotkey_id) = cx.update(|cx| {
        let status = cx.global::<AppStatus>().shortcut_error.clone();
        (cx.global::<AppSettings>().shortcut.clone(), status, controller.read(cx).hotkeys.current_id())
    });
    report.check(
        "push-to-talk shortcut registered",
        hotkey_id.is_some() && shortcut_error.is_none(),
        json!({"shortcut": shortcut, "error": shortcut_error}),
    );

    let (theme_dark, system) = cx.update(|cx| (cx.theme().is_dark(), cx.window_appearance()));
    let system_dark = matches!(system, WindowAppearance::Dark | WindowAppearance::VibrantDark);
    report.check(
        "theme follows the system appearance",
        theme_dark == system_dark,
        json!({"theme_dark": theme_dark, "system": format!("{system:?}")}),
    );

    let at_rest = panel_state(&controller, cx);
    report.check(
        "overlay is a guarded, click-through panel",
        at_rest.class == "EasyVoxOverlayPanel" && !at_rest.can_become_key && at_rest.ignores_mouse && !at_rest.visible,
        panel_json(&at_rest),
    );

    // The first load under a new bundle id compiles Metal shaders (~14 s).
    let waited = wait_for(cx, Duration::from_secs(120), |cx| {
        !matches!(cx.global::<AppStatus>().engine, EngineStatus::Loading | EngineStatus::NotLoaded)
    })
    .await;
    let (engine, model) = cx.update(|cx| {
        let model = controller.read(cx).engine.as_ref().map(|e| e.model_info().id);
        (cx.global::<AppStatus>().engine.clone(), model)
    });
    let engine_ready = matches!(engine, EngineStatus::Ready { .. });
    report.check(
        "speech model loads",
        engine_ready,
        json!({"model": model, "status": format!("{engine:?}"), "waited_ms": waited}),
    );

    match (engine_ready, hotkey_id) {
        (true, Some(id)) => {
            dictation(&controller, id, &options, &mut report, cx).await;
            dictation_after_unload(&controller, id, &mut report, cx).await;
        }
        _ => report.check("dictation", false, json!("skipped: needs the model and the shortcut")),
    }

    config_live_reload(&controller, &mut report, cx).await;
    windows(&controller, &mut report, cx).await;

    let pass = report.failed == 0;
    let json = json!({
        "pass": pass,
        "failed": report.failed,
        "elapsed_s": started.elapsed().as_secs_f64(),
        "exe": std::env::current_exe().ok(),
        "checks": report.checks,
    });
    match std::fs::write(&options.out, serde_json::to_string_pretty(&json).unwrap_or_default()) {
        Ok(()) => log::info!("self-test: report written to {}", options.out.display()),
        Err(e) => log::error!("self-test: couldn't write {}: {e}", options.out.display()),
    }
    let _ = std::fs::remove_dir_all(&options.dir);

    if !pass {
        log::error!("self-test: {} of {} checks failed", report.failed, report.checks.len());
        std::process::exit(1);
    }
    // Quit the way a person would, through the menu, with the model still loaded: this must
    // exit cleanly (no ggml-metal abort).
    log::info!("self-test: all {} checks passed; quitting through the menu", report.checks.len());
    let clicked = cx.update(|cx| {
        controller.read(cx).tray.as_ref().map(|t| platform::perform_menu_item(&t.menu, crate::tray::QUIT_TITLE))
    });
    if let Some(Err(e)) = clicked {
        log::error!("self-test: {e}");
    }
    sleep(cx, 3000).await;
    log::warn!("self-test: the Quit menu item didn't exit; calling cx.quit()");
    cx.update(|cx| cx.quit());
}

/// Synthetic ⌥Space press → replayed clip → live text → release → final text → clipboard →
/// history → overlay hides.
async fn dictation(
    controller: &Entity<Controller>,
    hotkey_id: u32,
    options: &Options,
    report: &mut Report,
    cx: &mut AsyncApp,
) {
    let clip = crate::testdata_dir().join(format!("{CLIP}.wav"));
    let clip_secs = match easyvox_core::audio::load_wav_16k_mono(&clip) {
        Ok(pcm) => pcm.len() as f64 / easyvox_core::SAMPLE_RATE as f64,
        Err(e) => return report.check("dictation", false, json!(format!("can't read {}: {e:#}", clip.display()))),
    };
    let history_before = cx.update(|cx| controller.read(cx).history.read(cx).entries.len());
    let clipboard_before = platform::clipboard_change_count();

    let status = platform::post_hotkey_event(hotkey_id, true);
    let pressed = Instant::now();
    let started = wait_for(cx, Duration::from_secs(1), |cx| !controller.read(cx).is_idle()).await;
    report.check(
        "shortcut press starts a session",
        status == 0 && started.is_some(),
        json!({"carbon_status": status, "ms": started}),
    );
    if started.is_none() {
        return;
    }
    let shown = panel_state(controller, cx);
    report.check(
        "overlay shows without taking focus",
        shown.visible && !shown.key && !shown.can_become_key && shown.ignores_mouse,
        panel_json(&shown),
    );

    let mut partials: Vec<(u128, String)> = vec![];
    let release_at = Duration::from_secs_f64((clip_secs - RELEASE_BEFORE_END).max(0.5));
    while pressed.elapsed() < release_at {
        sleep(cx, 100).await;
        let (mode, text) = cx.update(|cx| {
            let view = controller.read(cx).overlay.view.read(cx);
            (view.mode, view.text())
        });
        if mode == Mode::Live && partials.last().is_none_or(|(_, last)| *last != text) {
            partials.push((pressed.elapsed().as_millis(), text));
        }
    }
    report.check(
        "live text streams into the overlay",
        partials.len() >= 3,
        json!({"updates_seen": partials.len(), "first_ms": partials.first().map(|p| p.0),
               "last": partials.last().map(|p| &p.1)}),
    );

    let status = platform::post_hotkey_event(hotkey_id, false);
    let finished = wait_for(cx, Duration::from_secs(10), |cx| controller.read(cx).is_idle()).await;
    let final_text = cx.update(|cx| {
        let entries = &controller.read(cx).history.read(cx).entries;
        (entries.len() > history_before).then(|| entries[0].text.clone())
    });
    let expected = ground_truth(CLIP).unwrap_or_default();
    let wer = final_text.as_deref().map(|t| word_error_rate(&expected, t));
    report.check(
        "release delivers the final text",
        status == 0 && finished.is_some() && wer.is_some_and(|w| w <= MAX_WER),
        json!({"release_to_idle_ms": finished, "text": final_text, "expected": expected, "wer": wer}),
    );

    let clipboard_after = platform::clipboard_change_count();
    report.check(
        "final text copied to the clipboard (⌘V skipped)",
        final_text.is_some() && clipboard_after != clipboard_before,
        json!({"change_count_before": clipboard_before, "change_count_after": clipboard_after}),
    );

    let saved = easyvox_core::History::open(&options.history_db()).and_then(|h| h.recent(1));
    let saved_text = saved.as_ref().ok().and_then(|rows| rows.first()).map(|row| row.text.clone());
    report.check(
        "transcript saved to history",
        final_text.is_some() && saved_text == final_text,
        json!({"db": options.history_db(), "error": saved.err().map(|e| format!("{e:#}"))}),
    );

    let hidden = wait_for(cx, Duration::from_secs(3), |cx| !controller.read(cx).overlay.panel.state().visible).await;
    report.check("overlay hides after the final text", hidden.is_some(), json!({"ms": hidden}));
}

/// The idle timeout (shortened to 1 s) frees the model; the next press records at once while
/// the session reloads it, and the final text still has the first words.
async fn dictation_after_unload(
    controller: &Entity<Controller>,
    hotkey_id: u32,
    report: &mut Report,
    cx: &mut AsyncApp,
) {
    let footprint_mb = || easyvox_core::metrics::proc_stats().footprint_bytes as f64 / 1e6;
    let loaded_mb = footprint_mb();
    cx.update(|cx| {
        cx.global_mut::<AppSettings>().app.unload_after_secs = 1;
        controller.update(cx, |c, cx| c.schedule_unload(cx));
    });
    let freed =
        wait_for(cx, Duration::from_secs(10), |cx| controller.read(cx).engine.as_ref().is_some_and(|e| !e.is_loaded()))
            .await;
    cx.update(|cx| cx.global_mut::<AppSettings>().app.unload_after_secs = AppSettings::default().app.unload_after_secs);
    sleep(cx, 500).await; // let the allocator hand the memory back
    let unloaded_mb = footprint_mb();
    let status = cx.update(|cx| cx.global::<AppStatus>().engine.clone());
    report.check(
        "model unloads after the idle timeout",
        freed.is_some() && matches!(status, EngineStatus::Unloaded) && unloaded_mb < loaded_mb / 2.,
        json!({"ms_after_1s_timeout": freed, "status": format!("{status:?}"),
               "footprint_mb_loaded": loaded_mb.round(), "footprint_mb_unloaded": unloaded_mb.round()}),
    );
    if freed.is_none() {
        return;
    }

    let clip = crate::testdata_dir().join(format!("{CLIP}.wav"));
    let clip_secs = easyvox_core::audio::load_wav_16k_mono(&clip)
        .map_or(0., |pcm| pcm.len() as f64 / easyvox_core::SAMPLE_RATE as f64);
    let history_before = cx.update(|cx| controller.read(cx).history.read(cx).entries.len());
    let pressed_status = platform::post_hotkey_event(hotkey_id, true);
    let pressed = Instant::now();
    let started = wait_for(cx, Duration::from_secs(1), |cx| !controller.read(cx).is_idle()).await;
    // `start_session` marks the model as loading when it had to reload it.
    let status_at_start = cx.update(|cx| cx.global::<AppStatus>().engine.clone());
    let mut first_words_ms = None;
    let release_at = Duration::from_secs_f64((clip_secs - RELEASE_BEFORE_END).max(0.5));
    while started.is_some() && pressed.elapsed() < release_at {
        sleep(cx, 20).await;
        let live = cx.update(|cx| controller.read(cx).overlay.view.read(cx).mode == Mode::Live);
        if live && first_words_ms.is_none() {
            first_words_ms = Some(pressed.elapsed().as_millis());
        }
    }
    let released_status = platform::post_hotkey_event(hotkey_id, false);
    let finished = wait_for(cx, Duration::from_secs(10), |cx| controller.read(cx).is_idle()).await;
    let (final_text, status_after) = cx.update(|cx| {
        let entries = &controller.read(cx).history.read(cx).entries;
        let text = (entries.len() > history_before).then(|| entries[0].text.clone());
        (text, cx.global::<AppStatus>().engine.clone())
    });
    let wer = final_text.as_deref().map(|t| word_error_rate(&ground_truth(CLIP).unwrap_or_default(), t));
    report.check(
        "a press after an unload records at once, reloads the model and keeps every word",
        pressed_status == 0
            && released_status == 0
            && started.is_some()
            && matches!(status_at_start, EngineStatus::Loading)
            && matches!(status_after, EngineStatus::Ready { .. })
            && finished.is_some()
            && wer.is_some_and(|w| w <= MAX_WER),
        json!({"start_ms": started, "status_at_start": format!("{status_at_start:?}"),
               "first_words_ms": first_words_ms, "status_after": format!("{status_after:?}"),
               "release_to_idle_ms": finished, "text": final_text, "wer": wer}),
    );
}

/// An outside edit to the settings file applies without a restart; the app's own write (a
/// Settings switch) is saved without being reloaded, so the two can't loop. Then the file is
/// put back as it was.
async fn config_live_reload(controller: &Entity<Controller>, report: &mut Report, cx: &mut AsyncApp) {
    let path = cx.update(|cx| cx.global::<AppStatus>().config_path.clone());
    let original = std::fs::read_to_string(&path).ok();
    let (watching, reloads_before) = cx.update(|cx| {
        let c = controller.read(cx);
        (c.config_watcher.is_some(), c.config_reloads)
    });
    // The overlay's text length: nothing else in the self-test depends on it.
    let mut doc: toml_edit::DocumentMut = original.as_deref().unwrap_or("").parse().unwrap_or_default();
    doc["advanced"]["overlay_max_chars"] = toml_edit::value(41);
    let written = std::fs::write(&path, doc.to_string());
    let applied = wait_for(cx, Duration::from_secs(5), |cx| {
        cx.global::<AppSettings>().app.overlay_max_chars == 41 && controller.read(cx).config_reloads > reloads_before
    })
    .await;
    report.check(
        "an outside edit to the settings file applies live",
        watching && written.is_ok() && applied.is_some(),
        json!({"file": path, "watching": watching, "write_error": written.err().map(|e| e.to_string()),
               "applied_ms": applied}),
    );

    let (keep_before, reloads_before) =
        cx.update(|cx| (cx.global::<AppSettings>().keep_model_loaded, controller.read(cx).config_reloads));
    cx.update(|cx| controller.update(cx, |c, cx| c.set_keep_model_loaded(!keep_before, cx)));
    sleep(cx, 1500).await; // the watcher reports the write; the reload must find nothing new
    let saved = std::fs::read_to_string(&path).unwrap_or_default();
    let reloads_after = cx.update(|cx| controller.read(cx).config_reloads);
    report.check(
        "a change in Settings is saved to the file without reloading it",
        saved.contains(&format!("keep_model_loaded = {}", !keep_before)) && reloads_after == reloads_before,
        json!({"reloads_before": reloads_before, "reloads_after": reloads_after,
               "saved_keep_model_loaded": saved.contains(&format!("keep_model_loaded = {}", !keep_before))}),
    );
    cx.update(|cx| controller.update(cx, |c, cx| c.set_keep_model_loaded(keep_before, cx)));
    let _ = match original {
        Some(text) => std::fs::write(&path, text),
        None => std::fs::remove_file(&path),
    };
}

/// Opens Settings and History through their menu items and draws each once. GPUI doesn't
/// draw on its own while the screen is locked, so this forced draw is what runs their render
/// code (and would hit any render-time panic).
async fn windows(controller: &Entity<Controller>, report: &mut Report, cx: &mut AsyncApp) {
    let clicked = cx.update(|cx| {
        let tray = controller.read(cx).tray.as_ref()?;
        Some(
            [crate::tray::SETTINGS_TITLE, crate::tray::HISTORY_TITLE]
                .map(|t| platform::perform_menu_item(&tray.menu, t)),
        )
    });
    let opened =
        wait_for(cx, Duration::from_secs(3), |cx| controller.read(cx).windows().iter().all(Option::is_some)).await;
    let drawn = cx.update(|cx| {
        controller
            .read(cx)
            .windows()
            .into_iter()
            .flatten()
            .filter(|handle| handle.update(cx, |_, window, cx| window.draw(cx).clear(cx)).is_ok())
            .count()
    });
    report.check(
        "Settings and History open from the menu and render",
        opened.is_some() && drawn == 2,
        json!({"menu_actions": format!("{clicked:?}"), "opened_ms": opened, "windows_drawn": drawn}),
    );
}

fn panel_state(controller: &Entity<Controller>, cx: &mut AsyncApp) -> PanelState {
    cx.update(|cx| controller.read(cx).overlay.panel.state())
}

fn panel_json(s: &PanelState) -> Value {
    json!({"class": s.class, "visible": s.visible, "key": s.key, "can_become_key": s.can_become_key,
           "ignores_mouse": s.ignores_mouse})
}

async fn sleep(cx: &mut AsyncApp, ms: u64) {
    cx.background_executor().timer(Duration::from_millis(ms)).await;
}

/// Polls `done` every 20 ms; returns the ms it took, or `None` on timeout.
async fn wait_for(cx: &mut AsyncApp, timeout: Duration, mut done: impl FnMut(&mut App) -> bool) -> Option<u128> {
    let start = Instant::now();
    loop {
        if cx.update(&mut done) {
            return Some(start.elapsed().as_millis());
        }
        if start.elapsed() > timeout {
            return None;
        }
        sleep(cx, 20).await;
    }
}

fn ground_truth(id: &str) -> Option<String> {
    let manifest = std::fs::read_to_string(crate::testdata_dir().join("manifest.tsv")).ok()?;
    manifest.lines().find(|l| l.split('\t').next() == Some(id))?.split('\t').nth(3).map(String::from)
}

/// Word-level Levenshtein distance over lowercase words without punctuation, divided by the
/// number of reference words.
fn word_error_rate(reference: &str, hypothesis: &str) -> f64 {
    let words = |s: &str| -> Vec<String> {
        s.to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '\'' { c } else { ' ' })
            .collect::<String>()
            .split_whitespace()
            .map(String::from)
            .collect()
    };
    let (r, h) = (words(reference), words(hypothesis));
    let mut row: Vec<usize> = (0..=h.len()).collect();
    for (i, rw) in r.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, hw) in h.iter().enumerate() {
            let substitution = diagonal + usize::from(rw != hw);
            diagonal = row[j + 1];
            row[j + 1] = substitution.min(row[j] + 1).min(row[j + 1] + 1);
        }
    }
    row[h.len()] as f64 / r.len().max(1) as f64
}

#[cfg(test)]
mod tests {
    use super::word_error_rate;

    #[test]
    fn word_error_rate_ignores_case_and_punctuation() {
        assert_eq!(word_error_rate("Well, here's the thing.", "well here's the thing"), 0.0);
        assert_eq!(word_error_rate("a b c d", "a x c"), 0.5);
        assert_eq!(word_error_rate("a b", ""), 1.0);
    }
}
