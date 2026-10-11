//! `--self-test` (feature `self-test`): runs the real app through scripted dictations and
//! checks it from the inside, for a Mac nobody is looking at (screen locked, no key presses):
//! one with the model loaded, one right after the idle timeout freed it, and one of silence.
//! It writes a JSON report and exits 0 through the tray's Quit item when every check passes,
//! or 1 otherwise.
//!
//! The microphone is never opened (a test clip is replayed), ⌘V is never sent (the text is only
//! copied), and history goes to a temporary database. The settings file is a temporary one too,
//! unless `EASYVOX_CONFIG` names one; the self-test edits it to check live reloading, then puts
//! it back as it was. The model download checks never touch the internet: they download from a
//! server of their own on 127.0.0.1 into the temporary folder, and delete only from there.
//!
//! Onboarding is checked without touching a permission: the self-test reads the real ones
//! (onboarding must have opened at launch exactly when one is missing), and otherwise shows the
//! window injected answers. It never clicks a button that would bring up a macOS prompt.
//!
//! The speech models come from the models folder (`EASYVOX_MODELS_DIR` moves it). Nothing here
//! writes to it or deletes from it; onboarding's download check reads the chosen model from it
//! to serve on 127.0.0.1. Switching models needs a second offered model in it, or that check is
//! skipped.
//!
//! Light and dark are checked by giving EasyVox its own appearance for a moment (as a system
//! change would reach it), never by changing the system's. Reduce motion is answered by the
//! self-test (off, unless a check says otherwise), never read from or written to the system.
//!
//! GPUI doesn't draw while the screen is locked, so during the dictation and the motion checks
//! the self-test draws the overlay at each redraw and animation frame it asks for, as the screen
//! would, and its animations run as they would on screen.
//!
//! `--self-test --replay <wav> [--draw] [--repeat <n>] [--reduce-motion]` runs no checks: one
//! real-time dictation of the file with the overlay up (or `n` in a row, each after the last
//! one's close; with Reduce motion as if on), then a JSON report of what it cost the app
//! (CPU, redraws), for measuring the overlay (`docs/model-bench.md`, "Level meter" and "Overlay
//! animation"). `--draw` draws the overlay at each redraw it asks for, as GPUI does on screen but
//! not while the screen is locked.

use crate::controller::Controller;
use crate::onboarding::{Busy, LaunchFacts, Onboarding, Problem, State, StepId};
use crate::onboarding_window::{Permissions, chosen_model};
use crate::overlay::CANT_HEAR;
use crate::overlay::Mode;
use crate::platform::{self, PanelState};
use crate::settings::Page;
use crate::state::{AppSettings, AppStatus, DownloadStatus, EngineStatus};
use easyvox_core::MicPermission;
use easyvox_core::download::{DownloadError, Progress};
use easyvox_core::models::{self, ModelInfo};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
    /// `--replay <wav>`: only replay this file, and report its cost; `--draw`: and draw the
    /// overlay at each redraw it asks for, as the screen would; `--repeat <n>`: dictate it `n`
    /// times in a row, each after the last one's close; `--reduce-motion`: as if macOS's Reduce
    /// motion were on.
    replay: Option<PathBuf>,
    draw: bool,
    repeat: u32,
    reduce_motion: bool,
}

impl Options {
    /// `Some` when the command line has `--self-test` (and maybe `--out <file>`).
    pub fn from_args() -> Option<Self> {
        let mut args = std::env::args().skip(1);
        let (mut enabled, mut out, mut replay, mut draw, mut repeat) = (false, None, None, false, 1);
        let mut reduce_motion = false;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--self-test" => enabled = true,
                "--out" => out = args.next().map(PathBuf::from),
                "--replay" => replay = args.next().map(PathBuf::from),
                "--draw" => draw = true,
                "--repeat" => repeat = args.next().and_then(|n| n.parse().ok()).unwrap_or(1).max(1),
                "--reduce-motion" => reduce_motion = true,
                _ => {} // e.g. -psn_… from LaunchServices
            }
        }
        let tmp = std::env::temp_dir();
        enabled.then(|| Self {
            out: out.unwrap_or_else(|| tmp.join("easyvox-self-test.json")),
            dir: tmp.join(format!("easyvox-self-test-{}", std::process::id())),
            replay,
            draw,
            repeat,
            reduce_motion,
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
    // The overlay's pop and glide run, whatever this Mac's Reduce motion says (unless a replay
    // asks for Reduce motion).
    platform::force_reduce_motion(Some(options.replay.is_some() && options.reduce_motion));
    if let Some(wav) = options.replay.clone() {
        return replay(&controller, &options, &wav, cx).await;
    }
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
    onboarding_at_launch(&controller, &mut report, cx);

    let (theme_dark, system) = cx.update(|cx| (cx.theme().is_dark(), cx.window_appearance()));
    let system_dark = matches!(system, WindowAppearance::Dark | WindowAppearance::VibrantDark);
    report.check(
        "theme follows the system appearance",
        theme_dark == system_dark,
        json!({"theme_dark": theme_dark, "system": format!("{system:?}")}),
    );

    let at_rest = panel_state(&controller, cx);
    report.check(
        "overlay is a guarded, click-through, borderless, non-activating panel",
        at_rest.class == "EasyVoxOverlayPanel"
            && !at_rest.can_become_key
            && at_rest.ignores_mouse
            && at_rest.borderless
            && at_rest.nonactivating
            && !at_rest.visible,
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
            silence(&controller, id, &mut report, cx).await;
        }
        _ => report.check("dictation", false, json!("skipped: needs the model and the shortcut")),
    }
    reduce_motion(&controller, &mut report, cx).await;
    reopen_during_close(&controller, &mut report, cx).await;
    platform::force_reduce_motion(None);

    config_live_reload(&controller, &mut report, cx).await;
    windows(&controller, &mut report, cx).await;
    models_page(&controller, &mut report, cx).await;
    about_page(&controller, &mut report, cx).await;
    model_download(&controller, &options, &mut report, cx).await;
    match (engine_ready, hotkey_id) {
        (true, Some(id)) => {
            model_use(&controller, id, &mut report, cx).await;
            model_switch(&controller, &mut report, cx).await;
        }
        _ => report.check("switching models", false, json!("skipped: needs the model and the shortcut")),
    }
    onboarding_decision(&controller, &mut report, cx).await;
    onboarding_states(&controller, &options, &mut report, cx).await;
    match engine_ready {
        true => onboarding_download(&controller, &options, &mut report, cx).await,
        false => report.check("onboarding's download and warm-up", false, json!("skipped: needs the model")),
    }

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
    draw_each_redraw(controller, true, cx);
    let counts_before = cx.update(|cx| overlay_counts(controller, cx));

    let status = platform::post_hotkey_event(hotkey_id, true);
    let pressed = Instant::now();
    let started = wait_for(cx, Duration::from_secs(1), |cx| !controller.read(cx).is_idle()).await;
    report.check(
        "shortcut press starts a session",
        status == 0 && started.is_some(),
        json!({"carbon_status": status, "ms": started}),
    );
    if started.is_none() {
        draw_each_redraw(controller, false, cx);
        return;
    }
    sleep(cx, 40).await; // into the open's fade and pop
    let opening = cx.update(|cx| controller.read(cx).overlay.panel.motion_state());
    let shown = panel_state(controller, cx);
    report.check(
        "overlay shows without taking focus",
        shown.visible
            && !shown.key
            && !shown.can_become_key
            && shown.ignores_mouse
            && shown.borderless
            && shown.nonactivating,
        panel_json(&shown),
    );

    let mut partials: Vec<(u128, String)> = vec![];
    let release_at = Duration::from_secs_f64((clip_secs - RELEASE_BEFORE_END).max(0.5));
    let before = cx.update(|cx| overlay_counts(controller, cx));
    let (mut max_level, mut cant_hear) = (0f32, false);
    let mut appearance = None;
    while pressed.elapsed() < release_at {
        sleep(cx, 100).await;
        let (mode, text, level, no_input) = cx.update(|cx| {
            let view = controller.read(cx).overlay.view.read(cx);
            (view.mode, view.text(), view.level, view.cant_hear)
        });
        (max_level, cant_hear) = (max_level.max(level), cant_hear || no_input);
        if mode == Mode::Live && partials.last().is_none_or(|(_, last)| *last != text) {
            partials.push((pressed.elapsed().as_millis(), text));
        }
        if appearance.is_none() && pressed.elapsed() > Duration::from_secs(3) {
            appearance = Some(light_and_dark(controller, cx).await);
        }
    }
    let after = cx.update(|cx| overlay_counts(controller, cx));
    report.check(
        "live text streams into the overlay",
        partials.len() >= 3,
        json!({"updates_seen": partials.len(), "first_ms": partials.first().map(|p| p.0),
               "last": partials.last().map(|p| &p.1)}),
    );
    // Each level is one redraw at most, partials and the switches of appearance aside: the
    // meter's rate caps them.
    let interval = cx.update(|cx| cx.global::<AppSettings>().tuning.meter_interval_ms).max(1);
    let (levels, redraws) = (after.0 - before.0, after.1 - before.1);
    let cap = (release_at.as_secs_f64() * 1e3 / interval as f64) as u32 + partials.len() as u32 + 8;
    report.check(
        "the level meter moves with the voice and redraws only when it changes",
        max_level > 0.5 && !cant_hear && levels > 0 && redraws <= cap && redraws <= levels + partials.len() as u32 + 8,
        json!({"max_level": max_level, "cant_hear_seen": cant_hear, "levels_received": levels,
               "redraws_asked": redraws, "redraws_cap": cap, "renders": after.2 - before.2,
               "meter_interval_ms": interval}),
    );
    let (ok, detail) = appearance.unwrap_or((false, json!("the dictation ended before the switch")));
    report.check("the overlay follows light and dark, live, mid-dictation", ok, detail);

    let status = platform::post_hotkey_event(hotkey_id, false);
    let finished = wait_for(cx, Duration::from_secs(10), |cx| controller.read(cx).is_idle()).await;
    // The final text fades in for `overlay_words_ms`; then the pill, still up for a moment,
    // asks for nothing more.
    sleep(cx, 300).await;
    let (settled, animating_after_final, up_after_final) = cx.update(|cx| {
        let c = controller.read(cx);
        (overlay_counts(controller, cx), c.overlay.view.read(cx).is_animating(), c.overlay.panel.is_visible())
    });
    sleep(cx, 200).await;
    let still = cx.update(|cx| overlay_counts(controller, cx));
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

    // Hidden, nothing runs: no frames, no draws, no animation left on the layer.
    let rest_before = cx.update(|cx| overlay_counts(controller, cx));
    sleep(cx, 1000).await;
    let (rest_after, animating_hidden, resting) = cx.update(|cx| {
        let c = controller.read(cx);
        (overlay_counts(controller, cx), c.overlay.view.read(cx).is_animating(), c.overlay.panel.motion_state())
    });
    draw_each_redraw(controller, false, cx);
    let motion = cx.update(|cx| controller.read(cx).overlay.view.read(cx).motion);
    let frames = settled.3 - counts_before.3;
    // Each change of the text animates for the longer of the words' fade and the glide, a frame
    // every `frame` (woken a quarter early), and each move of the meter eases for `meter`: a
    // bound, with room for a few changes the polling above didn't see.
    let tick = |every: Duration| (every * 3 / 4).as_millis().max(1);
    let per_change = motion.words.max(motion.glide).as_millis() / tick(motion.frame) + 2;
    let per_move = motion.meter.as_millis() / tick(motion.meter_frame.min(motion.frame)) + 2;
    let moves = u128::from(settled.1 - counts_before.1);
    let frames_cap = (partials.len() as u128 + 6) * per_change + moves * per_move;
    let moved = |secs: Option<f64>| secs.is_some_and(|s| s > 0.);
    report.check(
        "the overlay's animations stop when they end, and nothing runs while it's hidden",
        moved(opening.fade_secs)
            && moved(opening.scale_secs)
            && (opening.scale.0 - 1.).abs() > 1e-3
            && scaled_in_place(&opening)
            && frames > 0
            && u128::from(frames) <= frames_cap
            && up_after_final
            && !animating_after_final
            && still.3 == settled.3
            && still.2 == settled.2
            && hidden.is_some()
            && rest_after == rest_before
            && !animating_hidden
            && resting.fade_secs.is_none()
            && resting.scale_secs.is_none()
            && resting.resting_opacity == 0.,
        json!({"open": motion_json(&opening), "animation_frames": frames, "frames_cap": frames_cap,
               "text_updates_seen": partials.len(), "after_final": {"up": up_after_final,
               "animating": animating_after_final, "frames_in_200_ms": still.3 - settled.3,
               "renders_in_200_ms": still.2 - settled.2},
               "hidden_1_s": {"frames": rest_after.3 - rest_before.3, "renders": rest_after.2 - rest_before.2,
               "redraws": rest_after.1 - rest_before.1, "animating": animating_hidden,
               "layer": motion_json(&resting)}}),
    );
}

/// Reduce motion on, then off, as the self-test's answer (the system's setting is never
/// touched): on, the open and close only fade, and the pill's glide and the meter's bars jump;
/// off, they pop, it glides and the bars ease. The words fade either way, and every animation
/// ends.
///
/// The open is watched as Core Animation draws it, every 10 ms. Popping, the pill starts small,
/// overshoots full size by no more than the timing curve's peak, is at full size (and opaque)
/// once the open's time is up, and keeps its centre where it is all the way.
async fn reduce_motion(controller: &Entity<Controller>, report: &mut Report, cx: &mut AsyncApp) {
    draw_each_redraw(controller, true, cx);
    let moved = |secs: Option<f64>| secs.is_some_and(|s| s > 0.);
    let mut steps = vec![];
    for on in [true, false] {
        platform::force_reduce_motion(Some(on));
        let first = cx.update(|cx| {
            controller.update(cx, |c, cx| {
                c.overlay.view.update(cx, |v, cx| v.reset(cx));
                c.overlay.show(cx);
                c.overlay.panel.motion_state()
            })
        });
        let (motion, open_secs) = cx.update(|cx| {
            let app = &cx.global::<AppSettings>().app;
            (controller.read(cx).overlay.view.read(cx).motion, f64::from(app.overlay_open_ms) / 1e3)
        });
        // The whole open, and a little after.
        let mut open = vec![first];
        let watching = Instant::now();
        while watching.elapsed().as_secs_f64() < open_secs + 0.08 {
            sleep(cx, 10).await;
            open.push(cx.update(|cx| controller.read(cx).overlay.panel.motion_state()));
        }
        let opening = &open[0];
        let opened = open.last().unwrap_or(opening);
        let (smallest, biggest) =
            open.iter().fold((f64::INFINITY, 0f64), |(lo, hi), m| (lo.min(m.scale.0), hi.max(m.scale.0)));
        let in_place = open.iter().all(scaled_in_place);
        // Words long enough to grow the pill, and a louder voice: the words fade in, and the
        // pill glides and the meter eases unless motion is reduced. The draw this asks for runs
        // before the next update.
        cx.update(|cx| {
            let view = controller.read(cx).overlay.view.clone();
            view.update(cx, |v, cx| {
                v.set_partial("Checking how the overlay".into(), "moves for you".into(), cx);
                v.set_level(0.9, cx);
            });
        });
        let (fading, gliding, easing) = cx.update(|cx| controller.read(cx).overlay.view.read(cx).animations());
        let settled =
            wait_for(cx, Duration::from_secs(2), |cx| !controller.read(cx).overlay.view.read(cx).is_animating()).await;
        cx.update(|cx| controller.update(cx, |c, cx| c.overlay.hide(cx)));
        sleep(cx, 40).await;
        let closing = cx.update(|cx| controller.read(cx).overlay.panel.motion_state());
        let hidden = wait_for(cx, Duration::from_secs(2), |cx| !controller.read(cx).overlay.panel.is_visible()).await;
        let resting = cx.update(|cx| controller.read(cx).overlay.panel.motion_state());
        let shrink = f64::from(motion.shrink);
        let movement = if on {
            motion.shrink == 0.
                && motion.glide.is_zero()
                && open.iter().all(|m| m.scale_secs.is_none() && m.scale == (1., 1.))
                && closing.scale_secs.is_none()
                && closing.scale == (1., 1.)
                && !gliding
                && !easing
        } else {
            motion.shrink > 0.
                && !motion.glide.is_zero()
                // From small, past full size by at most the curve's peak (9.8 % of the growth),
                // and onto it exactly as the open ends.
                && opening.scale_secs.is_some_and(|s| (s - open_secs).abs() < 1e-6)
                && smallest < 1. - shrink / 2.
                && biggest > 1. + 1e-3
                && biggest < 1. + 0.098 * shrink + 1e-3
                && opened.scale == (1., 1.)
                && in_place
                && moved(closing.scale_secs)
                && closing.scale.0 < 1.
                && scaled_in_place(&closing)
                && gliding
                && easing
        };
        let ok = movement
            && moved(opening.fade_secs)
            && opened.opacity == 1.
            && moved(closing.fade_secs)
            && fading
            && settled.is_some()
            && hidden.is_some()
            && resting.fade_secs.is_none()
            && resting.scale_secs.is_none();
        let samples: Vec<Value> = open.iter().map(|m| json!([m.opacity, m.scale.0])).collect();
        steps.push(json!({"reduce_motion": on, "shrink": motion.shrink, "glide_ms": motion.glide.as_millis(),
                          "open": motion_json(opening), "open_samples_opacity_scale": samples,
                          "smallest": smallest, "biggest": biggest, "in_place": in_place,
                          "opened": motion_json(opened), "close": motion_json(&closing), "words_fading": fading,
                          "gliding": gliding, "meter_easing": easing, "settled_ms": settled, "hidden_ms": hidden,
                          "ok": ok}));
    }
    platform::force_reduce_motion(Some(false));
    draw_each_redraw(controller, false, cx);
    report.check(
        "Reduce motion swaps the open and close's pop, the pill's glide and the meter's easing for plain fades",
        steps.iter().all(|s| s["ok"] == true),
        json!(steps),
    );
}

/// The layer drawn at the same scale across and up, about its centre (to within a twentieth of
/// a point), and changing size, or not, as an animation of its scale says.
fn scaled_in_place(m: &platform::MotionState) -> bool {
    let (x, y) = m.scale;
    (x - y).abs() < 1e-9 && m.centre_shift < 0.05 && (m.scale_secs.is_some() || x == 1.)
}

/// A press during the close (here, the show it makes) brings the pill straight back, from
/// where the fade and the shrink had got to (the show's animations start from what's drawn, so
/// nothing jumps), and the close's order-out never comes.
async fn reopen_during_close(controller: &Entity<Controller>, report: &mut Report, cx: &mut AsyncApp) {
    // What's drawn just before the show, and just after it.
    let show = |cx: &mut AsyncApp| {
        cx.update(|cx| {
            controller.update(cx, |c, cx| {
                c.overlay.view.update(cx, |v, cx| v.set_notice("Checking the close".into(), cx));
                let before = c.overlay.panel.motion_state();
                c.overlay.show(cx);
                (before, c.overlay.panel.motion_state())
            })
        })
    };
    let motion = |cx: &mut AsyncApp| cx.update(|cx| controller.read(cx).overlay.panel.motion_state());
    let (open_close, shrink) = cx.update(|cx| {
        let app = &cx.global::<AppSettings>().app;
        ((u64::from(app.overlay_open_ms), u64::from(app.overlay_close_ms)), app.overlay_shrink_pct)
    });
    show(cx);
    sleep(cx, open_close.0 + 50).await;
    cx.update(|cx| controller.update(cx, |c, cx| c.overlay.hide(cx)));
    sleep(cx, open_close.1 / 2).await;
    let (fading, turned) = show(cx);
    sleep(cx, 20).await;
    let back = motion(cx);
    let closing_after = cx.update(|cx| controller.read(cx).overlay.is_closing());
    // Well past when the close would have ordered it out.
    sleep(cx, open_close.0.max(open_close.1) + 150).await;
    let (up, settled) = (cx.update(|cx| controller.read(cx).overlay.panel.is_visible()), motion(cx));
    cx.update(|cx| controller.update(cx, |c, cx| c.overlay.hide(cx)));
    let hidden = wait_for(cx, Duration::from_secs(2), |cx| !controller.read(cx).overlay.panel.is_visible()).await;
    let starts_at = |from: Option<f64>, drawn: f64| from.is_some_and(|f| (f - drawn).abs() < 0.01);
    let no_jump =
        starts_at(turned.fade_from, fading.opacity) && (shrink == 0 || starts_at(turned.scale_from, fading.scale.0));
    report.check(
        "a press during the close brings the overlay straight back",
        fading.opacity < 0.99
            && fading.fade_secs.is_some()
            && (shrink == 0 || fading.scale.0 < 1.)
            && no_jump
            && back.fade_secs.is_some()
            && back.opacity > fading.opacity
            && !closing_after
            && up
            && settled.opacity > 0.99
            && settled.scale == (1., 1.)
            && hidden.is_some(),
        json!({"mid_close": motion_json(&fading), "turned": motion_json(&turned), "no_jump": no_jump,
               "reopened": motion_json(&back), "close_cancelled": !closing_after, "up_after": up,
               "settled": motion_json(&settled), "hidden_after_ms": hidden}),
    );
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

/// A press with nothing said (the silent test clip): the core decodes nothing and sends empty
/// text, so nothing is copied or saved, and the overlay says so, then hides.
async fn silence(controller: &Entity<Controller>, hotkey_id: u32, report: &mut Report, cx: &mut AsyncApp) {
    // The clip is digital silence, as a muted microphone gives: "can't hear you", sooner than
    // the default so it shows within the press.
    let no_input_ms = cx.update(|cx| {
        controller.update(cx, |c, _| c.source_override = Some(crate::testdata_dir().join("silence_2s.wav")));
        std::mem::replace(&mut cx.global_mut::<AppSettings>().tuning.meter_no_input_ms, 600)
    });
    let history_before = cx.update(|cx| controller.read(cx).history.read(cx).entries.len());
    let clipboard_before = platform::clipboard_change_count();
    let pressed = platform::post_hotkey_event(hotkey_id, true);
    let started = wait_for(cx, Duration::from_secs(1), |cx| !controller.read(cx).is_idle()).await;
    let before = cx.update(|cx| overlay_counts(controller, cx));
    let cant_hear =
        wait_for(cx, Duration::from_millis(1400), |cx| controller.read(cx).overlay.view.read(cx).cant_hear).await;
    // Drawn as a person would see it (GPUI doesn't draw while the screen is locked).
    let drawn = cx.update(|cx| draw_overlay(controller, cx));
    let (max_level, during) = cx.update(|cx| {
        let view = controller.read(cx).overlay.view.read(cx);
        (view.level, overlay_counts(controller, cx))
    });
    sleep(cx, 1500u64.saturating_sub(started.unwrap_or(0) as u64 + cant_hear.unwrap_or(1400) as u64)).await;
    let released = platform::post_hotkey_event(hotkey_id, false);
    let finished = wait_for(cx, Duration::from_secs(5), |cx| controller.read(cx).is_idle()).await;
    let (mode, notice) = cx.update(|cx| {
        let view = controller.read(cx).overlay.view.read(cx);
        (view.mode, view.text())
    });
    let notice_ms = cx.update(|cx| cx.global::<AppSettings>().app.overlay_hide_after_notice_ms);
    let hidden = wait_for(cx, Duration::from_millis(u64::from(notice_ms) + 2000), |cx| {
        !controller.read(cx).overlay.panel.state().visible
    })
    .await;
    let history_after = cx.update(|cx| controller.read(cx).history.read(cx).entries.len());
    let clipboard_after = platform::clipboard_change_count();
    cx.update(|cx| {
        controller.update(cx, |c, _| c.source_override = Some(crate::testdata_dir().join(format!("{CLIP}.wav"))));
        cx.global_mut::<AppSettings>().tuning.meter_no_input_ms = no_input_ms;
    });
    report.check(
        "digital silence: the meter rests flat and the overlay says it can't hear you",
        cant_hear.is_some() && drawn && max_level == 0. && during.0 == before.0 && during.1 - before.1 <= 2,
        json!({"cant_hear_after_ms": cant_hear, "says": CANT_HEAR, "drawn": drawn, "level": max_level,
               "levels_received": during.0 - before.0, "redraws_asked": during.1 - before.1}),
    );
    report.check(
        "silence pastes and saves nothing, and the overlay says so",
        pressed == 0
            && released == 0
            && started.is_some()
            && finished.is_some()
            && mode == Mode::Notice
            && hidden.is_some()
            && history_after == history_before
            && clipboard_after == clipboard_before,
        json!({"release_to_idle_ms": finished, "overlay": notice, "hidden_after_ms": hidden,
               "history_entries_added": history_after - history_before,
               "clipboard_changed": clipboard_after != clipboard_before}),
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

/// Draws the open Settings window. False if it isn't open or the draw failed.
fn draw_settings(controller: &Entity<Controller>, cx: &mut App) -> bool {
    let Some(handle) = controller.read(cx).windows()[0] else { return false };
    handle.update(cx, |_, window, cx| window.draw(cx).clear(cx)).is_ok()
}

/// The Models page draws a row for every offered model, in every state a row can be in:
/// downloaded or not (whatever the models folder holds), downloading, checking, reconnecting
/// and failed (set in `AppStatus` for the draw only; nothing downloads).
async fn models_page(controller: &Entity<Controller>, report: &mut Report, cx: &mut AsyncApp) {
    use crate::model_row::DRAWN;
    use std::sync::atomic::Ordering::Relaxed;
    let offered = models::offered();
    let shown = cx.update(|cx| {
        let view = controller.read(cx).settings_view()?;
        view.update(cx, |v, cx| v.show(Page::Models, cx));
        Some(())
    });
    let states = [
        None,
        Some(DownloadStatus::Running(Progress::Downloading { done: 120_000_000, total: 539_012_608 })),
        Some(DownloadStatus::Running(Progress::Verifying)),
        Some(DownloadStatus::Running(Progress::Reconnecting { attempt: 2, max: 5, done: 1, total: 2 })),
        Some(DownloadStatus::Failed(DownloadError::ChecksumMismatch { expected: "a".into(), actual: "b".into() })),
        None,
    ];
    let mut draws = vec![];
    for state in states {
        let (drawn, rows) = cx.update(|cx| {
            let ids: Vec<&'static str> = offered.iter().map(|m| m.id).collect();
            let downloads = &mut cx.global_mut::<AppStatus>().downloads;
            for id in &ids {
                match &state {
                    Some(status) => downloads.insert(id, status.clone()),
                    None => downloads.remove(id),
                };
            }
            cx.refresh_windows();
            let before = DRAWN.load(Relaxed);
            let drawn = draw_settings(controller, cx);
            (drawn, DRAWN.load(Relaxed) - before)
        });
        draws.push(json!({"state": format!("{state:?}"), "drawn": drawn, "rows": rows}));
    }
    let ok = shown.is_some()
        && draws.iter().all(|d| d["drawn"] == true && d["rows"].as_u64().unwrap_or(0) >= offered.len() as u64);
    report.check(
        "the Models page draws every offered model in every state",
        ok,
        json!({"offered": offered.iter().map(|m| m.id).collect::<Vec<_>>(), "draws": draws}),
    );
}

/// "About EasyVox" in the menu opens Settings on its About page, which draws every offered
/// model's credits; the third-party notices are in the app bundle and cover the vendored code.
async fn about_page(controller: &Entity<Controller>, report: &mut Report, cx: &mut AsyncApp) {
    use crate::about::{DRAWN, LICENSE, NOTICES};
    use std::sync::atomic::Ordering::Relaxed;
    let before = DRAWN.load(Relaxed);
    let clicked = cx.update(|cx| {
        let tray = controller.read(cx).tray.as_ref()?;
        Some(platform::perform_menu_item(&tray.menu, crate::tray::ABOUT_TITLE))
    });
    let opened = wait_for(cx, Duration::from_secs(3), |cx| {
        controller.read(cx).settings_view().is_some_and(|v| v.read(cx).page() == Page::About)
    })
    .await;
    let drawn = cx.update(|cx| draw_settings(controller, cx));
    let credits_drawn = DRAWN.load(Relaxed) - before;
    let resources = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.parent()?.join("Resources")));
    let (notices, license) = (NOTICES.path(), LICENSE.path());
    let in_bundle =
        |path: &Option<PathBuf>| path.as_ref().zip(resources.as_ref()).is_some_and(|(p, r)| p.starts_with(r));
    let text = notices.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    let covers: Vec<&str> = ["The ggml authors", "Rich Geldreich", "Lucide Icons", "option-ext", "SQLite"]
        .into_iter()
        .filter(|needle| !text.contains(needle))
        .collect();
    report.check(
        "About opens from the menu, shows the credits, and the notices are in the app",
        matches!(clicked, Some(Ok(())))
            && opened.is_some()
            && drawn
            && credits_drawn >= models::offered().len()
            && in_bundle(&notices)
            && in_bundle(&license)
            && covers.is_empty(),
        json!({"menu_action": format!("{clicked:?}"), "opened_ms": opened, "drawn": drawn,
               "credits_drawn": credits_drawn, "notices": notices, "license": license,
               "notices_missing": covers}),
    );
    cx.update(|cx| {
        if let Some(view) = controller.read(cx).settings_view() {
            view.update(cx, |v, cx| v.show(Page::General, cx));
        }
    });
}

/// A model download through the controller, the way onboarding and Settings start one, from a
/// server on 127.0.0.1 into the temporary folder: progress reaches `AppStatus`, a cancel
/// leaves nothing behind, and a full download lands verified with its status cleared and its
/// license notice next to it. Then the delete: refused while it downloads or is in use, and it
/// takes both files.
async fn model_download(controller: &Entity<Controller>, options: &Options, report: &mut Report, cx: &mut AsyncApp) {
    let payload: Vec<u8> = (0..2 * 1024 * 1024u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 11) as u8).collect();
    let serve = Serve {
        body: Body::Bytes(payload.clone()),
        pace: Some((64 * 1024, Duration::from_millis(20))),
        cut_once_at: None,
    };
    let server = match LocalServer::start(serve) {
        Ok(server) => server,
        Err(e) => return report.check("model download from a local server", false, json!(e.to_string())),
    };
    use sha2::Digest as _;
    let sha256: String = sha2::Sha256::digest(&payload).iter().map(|b| format!("{b:02x}")).collect();
    let Ok(default) = models::find(easyvox_core::RECOMMENDED_MODEL) else { return };
    let info: &'static ModelInfo = Box::leak(Box::new(ModelInfo {
        id: "self-test-download",
        path: "gguf/self-test.gguf",
        download_url: String::leak(server.url("self-test.gguf")),
        sha256: Some(String::leak(sha256)),
        download_bytes: payload.len() as u64,
        disk_bytes: payload.len() as u64,
        ..default.clone()
    }));
    let dir = options.dir.join("models");
    let start = |cx: &mut AsyncApp| {
        let dir = dir.clone();
        cx.update(|cx| controller.update(cx, |c, cx| c.start_download(info, dir, cx)));
    };
    let status = |cx: &mut App| cx.global::<AppStatus>().downloads.get(info.id).cloned();
    let downloaded = |cx: &mut App| match status(cx) {
        Some(DownloadStatus::Running(Progress::Downloading { done, .. })) => done,
        _ => 0,
    };

    start(cx);
    let progressed = wait_for(cx, Duration::from_secs(5), |cx| downloaded(cx) > 0).await;
    let refused_downloading = cx.update(|cx| controller.update(cx, |c, cx| c.delete_model_at(info, &dir, cx)));
    report.check(
        "a model can't be deleted while it downloads",
        refused_downloading.as_ref().is_err_and(|e| e.contains("downloading")),
        json!({"result": format!("{refused_downloading:?}")}),
    );
    cx.update(|cx| controller.update(cx, |c, _| c.cancel_download(info.id)));
    let cleared =
        wait_for(cx, Duration::from_secs(3), |cx| !controller.read(cx).is_downloading(info.id) && status(cx).is_none())
            .await;
    let left = names_in(&dir.join("gguf"));
    report.check(
        "a cancelled model download leaves nothing behind",
        progressed.is_some() && cleared.is_some() && left.is_empty(),
        json!({"progress_ms": progressed, "cleared_ms": cleared, "files_left": left}),
    );

    start(cx);
    let mut seen: Vec<u64> = vec![];
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(10) && cx.update(|cx| controller.read(cx).is_downloading(info.id)) {
        let done = cx.update(downloaded);
        if done > 0 && seen.last() != Some(&done) {
            seen.push(done);
        }
        sleep(cx, 20).await;
    }
    let (busy, after) = cx.update(|cx| (controller.read(cx).is_downloading(info.id), status(cx)));
    let path = info.resolve(&dir);
    let size = std::fs::metadata(&path).map(|m| m.len()).ok();
    let files = names_in(&dir.join("gguf"));
    let notice = std::fs::read_to_string(info.license_notice_path(&dir)).unwrap_or_default();
    report.check(
        "a model download from a local server shows progress and lands verified, with its license notice",
        !busy
            && after.is_none()
            && !seen.is_empty()
            && seen.windows(2).all(|w| w[0] <= w[1])
            && size == Some(info.download_bytes)
            && info.is_available(&dir)
            && files == ["self-test.gguf", "self-test.gguf.LICENSE.txt"]
            && notice == info.license_notice(),
        json!({"ms": started.elapsed().as_millis(), "progress_seen": seen, "status_after": format!("{after:?}"),
               "size": size, "files": files, "requests": server.requests().len(),
               "notice_lines": notice.lines().count()}),
    );

    // As if it were the model in use: refused. (The setting is put back at once; nothing loads.)
    let refused_in_use = cx.update(|cx| {
        let chosen = std::mem::replace(&mut cx.global_mut::<AppSettings>().model, info.id.to_string());
        let result = controller.update(cx, |c, cx| c.delete_model_at(info, &dir, cx));
        cx.global_mut::<AppSettings>().model = chosen;
        result
    });
    let kept = names_in(&dir.join("gguf"));
    let deleted = cx.update(|cx| controller.update(cx, |c, cx| c.delete_model_at(info, &dir, cx)));
    let left = names_in(&dir.join("gguf"));
    report.check(
        "deleting a model is refused while it's in use, then takes the model and its notice",
        refused_in_use.as_ref().is_err_and(|e| e.contains("in use"))
            && kept.len() == 2
            && deleted.is_ok()
            && left.is_empty()
            && !info.is_available(&dir),
        json!({"refused_in_use": format!("{refused_in_use:?}"), "deleted": format!("{deleted:?}"),
               "files_left": left}),
    );
}

/// "Use" on another downloaded model during a dictation: that dictation finishes on the model
/// it started with, then the app switches, frees the old model and loads the new one (memory
/// drops to the new model's), and a switch back loads the first again. The settings file the
/// switch writes is put back as it was. Skipped when no second offered model is downloaded.
async fn model_use(controller: &Entity<Controller>, hotkey_id: u32, report: &mut Report, cx: &mut AsyncApp) {
    let name = "Use switches models after the dictation in progress, and frees the old one";
    let models_dir = cx.update(|cx| cx.global::<AppStatus>().models_dir.clone());
    let original = cx.update(|cx| cx.global::<AppSettings>().model.clone());
    let Some(other) = models::offered().into_iter().find(|m| m.id != original && m.is_available(&models_dir)) else {
        return report.check(name, true, json!("skipped: no second offered model is downloaded"));
    };
    let path = cx.update(|cx| cx.global::<AppStatus>().config_path.clone());
    let saved_text = std::fs::read_to_string(&path).ok();
    let footprint_mb = || easyvox_core::metrics::proc_stats().footprint_bytes as f64 / 1e6;
    let before_mb = footprint_mb();

    let history_before = cx.update(|cx| controller.read(cx).history.read(cx).entries.len());
    let pressed = platform::post_hotkey_event(hotkey_id, true);
    let started = wait_for(cx, Duration::from_secs(1), |cx| !controller.read(cx).is_idle()).await;
    sleep(cx, 2500).await;
    cx.update(|cx| controller.update(cx, |c, cx| c.select_model(other.id, cx)));
    let during = cx.update(|cx| controller.read(cx).engine.as_ref().map(|e| e.model_info().id));
    let released = platform::post_hotkey_event(hotkey_id, false);
    let finished = wait_for(cx, Duration::from_secs(10), |cx| controller.read(cx).is_idle()).await;
    let dictated_with = cx.update(|cx| {
        let entries = &controller.read(cx).history.read(cx).entries;
        (entries.len() > history_before).then(|| entries[0].model_id.clone())
    });
    let switched = wait_for(cx, Duration::from_secs(60), |cx| {
        matches!(cx.global::<AppStatus>().engine, EngineStatus::Ready { .. })
            && controller.read(cx).engine.as_ref().is_some_and(|e| e.model_info().id == other.id)
    })
    .await;
    sleep(cx, 500).await; // let the allocator hand the memory back
    let after_mb = footprint_mb();
    let saved = std::fs::read_to_string(&path).unwrap_or_default();

    cx.update(|cx| controller.update(cx, |c, cx| c.select_model(&original, cx)));
    let back = wait_for(cx, Duration::from_secs(60), |cx| {
        matches!(cx.global::<AppStatus>().engine, EngineStatus::Ready { .. })
            && controller.read(cx).engine.as_ref().is_some_and(|e| e.model_info().id == original)
    })
    .await;
    let _ = match saved_text {
        Some(text) => std::fs::write(&path, text),
        None => std::fs::remove_file(&path),
    };
    report.check(
        name,
        pressed == 0
            && released == 0
            && started.is_some()
            && during == Some(original.as_str())
            && finished.is_some()
            && dictated_with.as_deref() == Some(original.as_str())
            && switched.is_some()
            && after_mb < before_mb - 150.
            && saved.contains(&format!("model = \"{}\"", other.id))
            && back.is_some(),
        json!({"from": original, "to": other.id, "engine_during_dictation": during,
               "dictation_used": dictated_with, "switched_ms": switched,
               "footprint_mb_before": before_mb.round(), "footprint_mb_after": after_mb.round(),
               "saved_to_settings_file": saved.contains(&format!("model = \"{}\"", other.id)),
               "switched_back_ms": back}),
    );
}

/// Picking a model that isn't downloaded frees the engine and shows it as missing; picking the
/// default again loads it back. Skipped when every offered model is downloaded here.
async fn model_switch(controller: &Entity<Controller>, report: &mut Report, cx: &mut AsyncApp) {
    let models_dir = cx.update(|cx| cx.global::<AppStatus>().models_dir.clone());
    let original = cx.update(|cx| cx.global::<AppSettings>().model.clone());
    let Some(missing) = models::offered().into_iter().find(|m| !m.is_available(&models_dir)) else {
        return report.check("picking a model that isn't downloaded", true, json!("skipped: all are downloaded"));
    };
    cx.update(|cx| controller.update(cx, |c, cx| c.use_model(missing.id, cx)));
    let (status, engine) =
        cx.update(|cx| (cx.global::<AppStatus>().engine.clone(), controller.read(cx).engine.is_some()));
    cx.update(|cx| controller.update(cx, |c, cx| c.use_model(&original, cx)));
    let back = wait_for(cx, Duration::from_secs(30), |cx| {
        matches!(cx.global::<AppStatus>().engine, EngineStatus::Ready { .. })
            && controller.read(cx).engine.as_ref().is_some_and(|e| e.model_info().id == original)
    })
    .await;
    report.check(
        "picking a model that isn't downloaded shows it missing; picking the default loads it again",
        matches!(status, EngineStatus::Missing) && !engine && back.is_some(),
        json!({"missing": missing.id, "status": format!("{status:?}"), "engine_kept": engine, "reloaded_ms": back}),
    );
}

/// Onboarding at launch, on this Mac's real answers (read, never changed): the window opened
/// exactly when a required step is missing.
fn onboarding_at_launch(controller: &Entity<Controller>, report: &mut Report, cx: &mut AsyncApp) {
    let (facts, open) = cx.update(|cx| (crate::onboarding_window::launch_facts(cx), onboarding_open(controller, cx)));
    report.check(
        "onboarding opened at launch exactly when a required step is missing",
        open == facts.needed(),
        json!({"mic": format!("{:?}", facts.mic), "accessibility": facts.accessibility,
               "model_downloaded": facts.model_downloaded, "needed": facts.needed(), "opened": open}),
    );
}

/// The launch decision with injected answers, through the controller as `main` calls it: with
/// nothing missing (or only a microphone the user refused) onboarding stays shut; with any
/// required step missing it opens, and draws. It's left open for the next checks.
async fn onboarding_decision(controller: &Entity<Controller>, report: &mut Report, cx: &mut AsyncApp) {
    let all = LaunchFacts { mic: MicPermission::Authorized, accessibility: true, model_downloaded: true };
    let mut results = vec![];
    for (facts, should_open) in [
        (all, false),
        (LaunchFacts { mic: MicPermission::Denied, ..all }, false),
        (LaunchFacts { mic: MicPermission::NotDetermined, ..all }, true),
        (LaunchFacts { accessibility: false, ..all }, true),
        (LaunchFacts { model_downloaded: false, ..all }, true),
    ] {
        cx.update(|cx| close_onboarding(controller, cx));
        let closed = wait_for(cx, Duration::from_secs(3), |cx| !onboarding_open(controller, cx)).await.is_some();
        let said = cx.update(|cx| controller.update(cx, |c, cx| c.onboard_if_needed(&facts, cx)));
        let open = wait_for(cx, Duration::from_secs(1), |cx| onboarding_open(controller, cx)).await.is_some();
        let drawn = open && cx.update(|cx| draw_onboarding(controller, cx));
        let ok = closed && said == should_open && open == should_open && drawn == should_open;
        results.push(json!({"facts": format!("{facts:?}"), "should_open": should_open, "opened": open,
                            "drawn": drawn, "ok": ok}));
    }
    report.check(
        "onboarding stays shut when nothing is missing, and opens and draws when a step is",
        results.iter().all(|r| r["ok"] == true),
        json!(results),
    );
}

/// The window in every state a step can be in, from injected answers (the permissions pinned
/// in the view, the download and the engine set in `AppStatus` for the draw only): what each
/// step says matches, and the window draws. Everything is put back afterwards.
async fn onboarding_states(controller: &Entity<Controller>, options: &Options, report: &mut Report, cx: &mut AsyncApp) {
    use crate::onboarding_window::DRAWN;
    use std::sync::atomic::Ordering::Relaxed;
    let name = "onboarding draws every step state, with the failure states' actions";
    let Some(view) = cx.update(|cx| controller.read(cx).onboarding_view()) else {
        return report.check(name, false, json!("the onboarding window isn't open"));
    };
    let Some(model) = cx.update(|cx| chosen_model(cx)) else {
        return report.check(name, false, json!("no chosen model"));
    };
    let (real_dir, engine_before) = cx.update(|cx| {
        let status = cx.global::<AppStatus>();
        (status.models_dir.clone(), status.engine.clone())
    });
    let empty_dir = options.dir.join("no-models");
    let total = model.download_bytes;
    let (todo, done) = (State::ToDo, State::Done);
    let offline = DownloadError::Offline("The Internet connection appears to be offline.".into());
    let wrong = DownloadError::ChecksumMismatch { expected: "a".into(), actual: "b".into() };
    let running = |p| Some(DownloadStatus::Running(p));
    // (case, mic, mic prompt up, accessibility, prompted, download, downloaded, engine, expected)
    #[rustfmt::skip]
    let cases = [
        ("fresh", MicPermission::NotDetermined, false, false, false, None, false, EngineStatus::Missing,
         [todo.clone(), todo.clone(), todo.clone(), State::Waiting]),
        ("prompts up, downloading", MicPermission::NotDetermined, true, false, true,
         running(Progress::Downloading { done: total / 4, total }), false, EngineStatus::Missing,
         [State::Busy(Busy::MicPrompt), State::Busy(Busy::AccessibilityPending), State::Busy(Busy::Downloading), State::Waiting]),
        ("mic denied, connection cut", MicPermission::Denied, false, false, false,
         running(Progress::Reconnecting { attempt: 1, max: 5, done: total / 2, total }), false, EngineStatus::Missing,
         [State::Problem(Problem::MicDenied), todo.clone(), State::Busy(Busy::Resuming), State::Waiting]),
        ("mic restricted, offline", MicPermission::Restricted, false, true, false,
         Some(DownloadStatus::Failed(offline)), false, EngineStatus::Missing,
         [State::Problem(Problem::MicRestricted), done.clone(), State::Problem(Problem::NoNetwork), State::Waiting]),
        ("checksum mismatch", MicPermission::Authorized, false, true, false,
         Some(DownloadStatus::Failed(wrong)), false, EngineStatus::Missing,
         [done.clone(), done.clone(), State::Problem(Problem::WrongFile), State::Waiting]),
        ("getting ready", MicPermission::Authorized, false, true, false, None, true, EngineStatus::Loading,
         [done.clone(), done.clone(), done.clone(), State::Busy(Busy::WarmingUp)]),
        ("load failed", MicPermission::Authorized, false, true, false, None, true,
         EngineStatus::Failed { error: "injected".into() },
         [done.clone(), done.clone(), done.clone(), State::Problem(Problem::LoadFailed("injected".into()))]),
        ("all done", MicPermission::Authorized, false, true, false, None, true, EngineStatus::Ready { load_ms: 180 },
         [done.clone(), done.clone(), done.clone(), done.clone()]),
    ];
    let mut draws = vec![];
    for (case, mic, mic_asking, accessibility, prompted, download, downloaded, engine, expected) in cases {
        let (drawn, states, finished) = cx.update(|cx| {
            let status = cx.global_mut::<AppStatus>();
            status.mic_asking = mic_asking;
            status.accessibility_prompted = prompted;
            status.engine = engine;
            status.models_dir = if downloaded { real_dir.clone() } else { empty_dir.clone() };
            match download {
                Some(d) => status.downloads.insert(model.id, d),
                None => status.downloads.remove(model.id),
            };
            view.update(cx, |v, cx| v.pin_permissions(Some(Permissions { mic, accessibility }), cx));
            cx.refresh_windows();
            let before = DRAWN.load(Relaxed);
            let drawn = draw_onboarding(controller, cx) && DRAWN.load(Relaxed) > before;
            let onboarding = view.read(cx).onboarding(cx);
            let states: Vec<State> =
                onboarding.as_ref().map(|o| o.steps.iter().map(|s| s.state.clone()).collect()).unwrap_or_default();
            (drawn, states, onboarding.is_some_and(|o| o.finished()))
        });
        let ok = drawn && states == expected && finished == (case == "all done");
        draws.push(json!({"case": case, "drawn": drawn, "states": format!("{states:?}"), "ok": ok}));
    }
    cx.update(|cx| {
        let status = cx.global_mut::<AppStatus>();
        status.mic_asking = false;
        status.accessibility_prompted = false;
        status.engine = engine_before;
        status.models_dir = real_dir;
        status.downloads.remove(model.id);
        view.update(cx, |v, cx| v.pin_permissions(None, cx));
        cx.refresh_windows();
    });
    report.check(name, draws.iter().all(|d| d["ok"] == true), json!(draws));
}

/// Onboarding's model and warm-up steps for real, in a temporary models folder: the chosen model
/// downloads from a server on 127.0.0.1 (serving the copy in the models folder, which is only
/// read), the connection is cut once and the download picks up where it stopped, the file lands
/// verified with its license notice, and the controller loads it (the warm-up), leaving every
/// step done. Then the app goes back to the real models folder.
async fn onboarding_download(
    controller: &Entity<Controller>,
    options: &Options,
    report: &mut Report,
    cx: &mut AsyncApp,
) {
    let name = "onboarding downloads the model from a local server, resumes a cut connection, then warms it up";
    let (real_dir, model, view) = cx.update(|cx| {
        (cx.global::<AppStatus>().models_dir.clone(), chosen_model(cx), controller.read(cx).onboarding_view())
    });
    let (Some(model), Some(view)) = (model.filter(|m| m.is_available(&real_dir)), view) else {
        return report.check(name, false, json!("needs the chosen model downloaded and the onboarding window open"));
    };
    let file = model.resolve(&real_dir);
    let file_name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let serve = Serve { body: Body::File(file.clone()), pace: None, cut_once_at: Some(model.download_bytes * 2 / 5) };
    let server = match LocalServer::start(serve) {
        Ok(server) => server,
        Err(e) => return report.check(name, false, json!(e.to_string())),
    };
    // The catalog entry, downloading from here instead of Hugging Face.
    let info: &'static ModelInfo =
        Box::leak(Box::new(ModelInfo { download_url: String::leak(server.url(&file_name)), ..model.clone() }));
    let dir = options.dir.join("onboarding-models");
    let steps = |cx: &mut App| -> Option<Onboarding> { view.read(cx).onboarding(cx) };
    let model_and_warm_up =
        |o: &Onboarding| (o.step(StepId::Model).state.clone(), o.step(StepId::WarmUp).state.clone());

    // Both permissions shown as allowed (the real ones aren't touched), and an empty models folder.
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            v.pin_permissions(Some(Permissions { mic: MicPermission::Authorized, accessibility: true }), cx)
        });
        controller.update(cx, |c, cx| c.set_models_dir(dir.clone(), cx));
    });
    let emptied =
        wait_for(cx, Duration::from_secs(10), |cx| matches!(cx.global::<AppStatus>().engine, EngineStatus::Missing))
            .await;
    let before = cx.update(|cx| steps(cx).map(|o| model_and_warm_up(&o)));
    let offered_download = cx.update(|cx| steps(cx).map(|o| o.step(StepId::Model).actions.clone()));

    // What the Download button does, with the local URL.
    let started = Instant::now();
    cx.update(|cx| controller.update(cx, |c, cx| c.start_download(info, dir.clone(), cx)));
    let mut seen: Vec<(State, State)> = vec![];
    let mut draws = 0;
    let mut finished = false;
    while started.elapsed() < Duration::from_secs(240) {
        let now = cx.update(|cx| steps(cx));
        let Some(now) = now else { break };
        let pair = model_and_warm_up(&now);
        if seen.last() != Some(&pair) {
            seen.push(pair);
            draws += usize::from(cx.update(|cx| draw_onboarding(controller, cx)));
        }
        if now.finished() {
            finished = true;
            break;
        }
        sleep(cx, 20).await;
    }
    let elapsed_ms = started.elapsed().as_millis();
    let saw = |state: State| seen.iter().any(|(m, w)| *m == state || *w == state);
    let (engine, loaded) = cx.update(|cx| {
        (cx.global::<AppStatus>().engine.clone(), controller.read(cx).engine.as_ref().map(|e| e.model_info().id))
    });
    let files = names_in(&info.resolve(&dir).parent().map(Path::to_path_buf).unwrap_or_default());
    let notice_ok = std::fs::read_to_string(info.license_notice_path(&dir)).is_ok_and(|n| n == info.license_notice());
    let requests = server.requests();
    drop(server);

    // Back to the real models folder; the chosen model loads from there again.
    cx.update(|cx| controller.update(cx, |c, cx| c.set_models_dir(real_dir.clone(), cx)));
    let back = wait_for(cx, Duration::from_secs(60), |cx| {
        matches!(cx.global::<AppStatus>().engine, EngineStatus::Ready { .. })
            && controller.read(cx).engine.as_ref().is_some_and(|e| e.model_info().id == model.id)
    })
    .await;
    cx.update(|cx| {
        view.update(cx, |v, cx| v.pin_permissions(None, cx));
        close_onboarding(controller, cx);
    });
    let _ = std::fs::remove_dir_all(&dir);

    let todo_first = before == Some((State::ToDo, State::Waiting));
    report.check(
        name,
        emptied.is_some()
            && todo_first
            && offered_download == Some(vec![crate::onboarding::Action::Download])
            && saw(State::Busy(Busy::Downloading))
            && saw(State::Busy(Busy::Resuming))
            && saw(State::Busy(Busy::WarmingUp))
            && finished
            && matches!(engine, EngineStatus::Ready { .. })
            && loaded == Some(model.id)
            && files.contains(&file_name)
            && files.contains(&format!("{file_name}.LICENSE.txt"))
            && files.len() == 2
            && notice_ok
            && requests.len() >= 2
            && requests.iter().skip(1).any(Option::is_some)
            && draws >= seen.len()
            && back.is_some(),
        json!({"model": model.id, "bytes": model.download_bytes, "ms": elapsed_ms,
               "before": format!("{before:?}"), "states_seen": format!("{seen:?}"), "draws": draws,
               "finished": finished, "engine": format!("{engine:?}"), "files": files, "notice_ok": notice_ok,
               "requests_range_starts": requests, "back_to_real_folder_ms": back}),
    );
}

/// Whether the onboarding window is open now.
fn onboarding_open(controller: &Entity<Controller>, cx: &mut App) -> bool {
    let Some(handle) = controller.read(cx).onboarding_window() else { return false };
    handle.update(cx, |_, _, _| ()).is_ok()
}

fn close_onboarding(controller: &Entity<Controller>, cx: &mut App) {
    if let Some(handle) = controller.read(cx).onboarding_window() {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    }
}

/// Draws the onboarding window. False if it isn't open or the draw failed.
fn draw_onboarding(controller: &Entity<Controller>, cx: &mut App) -> bool {
    let Some(handle) = controller.read(cx).onboarding_window() else { return false };
    handle.update(cx, |_, window, cx| window.draw(cx).clear(cx)).is_ok()
}

/// The names in `dir`, hidden ones included.
fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    names.sort();
    names
}

/// What a [`LocalServer`] serves, and how.
struct Serve {
    body: Body,
    /// Send `.0` bytes every `.1`, so a download takes long enough to watch and to cancel.
    /// `None`: as fast as the connection takes them.
    pace: Option<(usize, Duration)>,
    /// Close the first response's connection on reaching this byte of the file, as a dropped
    /// connection would, so the download has to pick up where it stopped (`Range`).
    cut_once_at: Option<u64>,
}

/// The file a [`LocalServer`] serves.
enum Body {
    Bytes(Vec<u8>),
    /// Read from disk as it's sent (a model: hundreds of MB).
    File(PathBuf),
}

/// Serves one file over plain HTTP on 127.0.0.1, with `Range` support. Stops when dropped.
struct LocalServer {
    port: u16,
    /// Each request's `Range` start, if it had one.
    requests: Arc<std::sync::Mutex<Vec<Option<u64>>>>,
    stop: Arc<AtomicBool>,
}

impl LocalServer {
    fn start(serve: Serve) -> std::io::Result<Self> {
        let len = match &serve.body {
            Body::Bytes(bytes) => bytes.len() as u64,
            Body::File(path) => std::fs::metadata(path)?.len(),
        };
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let (stop, requests) = (Arc::new(AtomicBool::new(false)), Arc::new(std::sync::Mutex::new(vec![])));
        let (stop2, requests2) = (stop.clone(), requests.clone());
        let (serve, cut) = (Arc::new(serve), Arc::new(AtomicBool::new(false)));
        std::thread::Builder::new().name("self-test-http".into()).spawn(move || {
            for stream in listener.incoming() {
                if stop2.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(stream) = stream else { continue };
                let (serve, cut, requests) = (serve.clone(), cut.clone(), requests2.clone());
                let _ = std::thread::Builder::new().spawn(move || serve_file(stream, &serve, len, &cut, &requests));
            }
        })?;
        Ok(Self { port, requests, stop })
    }

    fn url(&self, file: &str) -> String {
        format!("http://127.0.0.1:{}/resolve/0/{file}", self.port)
    }

    fn requests(&self) -> Vec<Option<u64>> {
        self.requests.lock().map_or_else(|_| vec![], |r| r.clone())
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(("127.0.0.1", self.port)); // wakes the accept loop
    }
}

fn serve_file(
    mut stream: TcpStream,
    serve: &Serve,
    len: u64,
    cut: &AtomicBool,
    requests: &std::sync::Mutex<Vec<Option<u64>>>,
) {
    let Ok(read_half) = stream.try_clone() else { return };
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return; // the shutdown wake-up
    }
    let mut range = None;
    loop {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("range")
        {
            range = value.trim().strip_prefix("bytes=").and_then(|v| v.trim_end_matches('-').parse::<u64>().ok());
        }
    }
    if let Ok(mut r) = requests.lock() {
        r.push(range);
    }
    let start = range.unwrap_or(0).min(len);
    let head = match range {
        None => format!("HTTP/1.1 200 OK\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n"),
        Some(_) => format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{}/{len}\r\n\
             Connection: close\r\n\r\n",
            len - start,
            len.saturating_sub(1)
        ),
    };
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    let end = match serve.cut_once_at {
        Some(at) if at > start && !cut.swap(true, Ordering::SeqCst) => at.min(len),
        _ => len,
    };
    let chunk = serve.pace.map_or(1024 * 1024, |(bytes, _)| bytes);
    let mut file = match &serve.body {
        Body::Bytes(_) => None,
        Body::File(path) => match std::fs::File::open(path).and_then(|mut f| f.seek(SeekFrom::Start(start)).map(|_| f))
        {
            Ok(f) => Some(f),
            Err(_) => return,
        },
    };
    let mut buf = vec![0; chunk];
    let mut at = start;
    while at < end {
        let n = chunk.min((end - at) as usize);
        let piece = match (&serve.body, file.as_mut()) {
            (Body::Bytes(bytes), _) => &bytes[at as usize..at as usize + n],
            (Body::File(_), Some(f)) => match f.read_exact(&mut buf[..n]) {
                Ok(()) => &buf[..n],
                Err(_) => return,
            },
            (Body::File(_), None) => return,
        };
        if stream.write_all(piece).is_err() {
            return; // the download was cancelled
        }
        at += n as u64;
        if let Some((_, every)) = serve.pace {
            std::thread::sleep(every);
        }
    }
    // A cut: the connection closes here, short of the length announced.
}

/// Mid-dictation, as if the system switched to the other appearance and back: the overlay
/// redraws on its own each time, in the right palette. EasyVox gets its own appearance for a
/// moment (`platform::set_app_appearance`); the system's setting is never touched.
async fn light_and_dark(controller: &Entity<Controller>, cx: &mut AsyncApp) -> (bool, Value) {
    let system_dark =
        cx.update(|cx| matches!(cx.window_appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark));
    let mut steps = vec![];
    // The other appearance, then back to following the system.
    for (forced, dark) in [(Some(!system_dark), !system_dark), (None, system_dark)] {
        let asked = cx.update(|cx| controller.read(cx).overlay.view.read(cx).redraws_asked);
        let set = platform::set_app_appearance(forced);
        let noticed =
            wait_for(cx, Duration::from_secs(2), |cx| controller.read(cx).overlay.view.read(cx).redraws_asked > asked)
                .await;
        // GPUI doesn't draw while the screen is locked, so draw it here, as the next frame would.
        let drawn = cx.update(|cx| draw_overlay(controller, cx));
        let drawn_dark = cx.update(|cx| controller.read(cx).overlay.view.read(cx).drawn_dark);
        let ok = set.is_ok() && noticed.is_some() && drawn && drawn_dark == Some(dark);
        steps.push(json!({"app_appearance": forced.map(|d| if d { "dark" } else { "light" }).unwrap_or("system"),
                          "set": set.err(), "redraw_asked_after_ms": noticed, "drawn": drawn,
                          "drawn_dark": drawn_dark, "ok": ok}));
    }
    let ok = steps.iter().all(|s| s["ok"] == true);
    (ok, json!({"system_dark": system_dark, "steps": steps}))
}

/// Draws the overlay now (GPUI doesn't draw while the screen is locked). False if it failed.
fn draw_overlay(controller: &Entity<Controller>, cx: &mut App) -> bool {
    let window = controller.read(cx).overlay.window;
    window.update(cx, |_, window, cx| window.draw(cx).clear(cx)).is_ok()
}

/// The overlay's counters: levels received, redraws asked for, draws, animation frames asked for.
fn overlay_counts(controller: &Entity<Controller>, cx: &App) -> (u32, u32, u32, u32) {
    let view = controller.read(cx).overlay.view.read(cx);
    (view.levels, view.redraws_asked, view.renders, view.frames_asked)
}

/// Draws the overlay at each redraw and animation frame it asks for, as the screen would (GPUI
/// doesn't draw while the screen is locked), or stops.
fn draw_each_redraw(controller: &Entity<Controller>, on: bool, cx: &mut AsyncApp) {
    cx.update(|cx| {
        let overlay = &controller.read(cx).overlay;
        let (view, window) = (overlay.view.clone(), overlay.window);
        view.update(cx, |v, _| v.draw_each_redraw = on.then_some(window));
    });
}

fn motion_json(m: &platform::MotionState) -> Value {
    json!({"opacity": m.opacity, "scale": [m.scale.0, m.scale.1], "centre_shift_pt": m.centre_shift,
           "resting_opacity": m.resting_opacity, "fade_secs": m.fade_secs, "scale_secs": m.scale_secs,
           "fade_from": m.fade_from, "scale_from": m.scale_from,
           "layer_size": [m.layer_size.0, m.layer_size.1], "anchor": [m.anchor.0, m.anchor.1],
           "window_size": [m.window_size.0, m.window_size.1]})
}

/// `--replay <wav>`: one real-time dictation of `wav` with the overlay up, nothing else, then a
/// report of what it cost the app: its CPU time and the overlay's redraws and animation frames,
/// with the wall-clock times of the session's start and end so `macmon`'s samples can be cut to
/// it (`docs/model-bench.md`, "Level meter" and "Overlay animation"). Whether the meter and the
/// animations are on comes from the settings file (`meter_interval_ms = 0` turns the meter off,
/// `overlay_words_ms = 0` and `overlay_glide_ms = 0` the pill's animations). Reduce motion is
/// taken as off, or as on with `--reduce-motion`. With `--repeat <n>`, it dictates the file `n`
/// times, each after the last one's close, and reports them together, closes included: for
/// measuring the open and close.
async fn replay(controller: &Entity<Controller>, options: &Options, wav: &Path, cx: &mut AsyncApp) {
    let unix_ms = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let waited = wait_for(cx, Duration::from_secs(120), |cx| {
        matches!(cx.global::<AppStatus>().engine, EngineStatus::Ready { .. })
    })
    .await;
    let audio_secs = easyvox_core::audio::load_wav_16k_mono(wav)
        .map_or(0., |pcm| pcm.len() as f64 / easyvox_core::SAMPLE_RATE as f64);
    let mut out = json!({"wav": wav, "audio_secs": audio_secs, "model_ready_ms": waited});
    if waited.is_some() && audio_secs > 0. {
        // The file may be a private recording: its words stay off the clipboard.
        cx.update(|cx| controller.update(cx, |c, _| c.copy_final = false));
        if options.draw {
            cx.update(|cx| {
                let window = controller.read(cx).overlay.window;
                controller.read(cx).overlay.view.clone().update(cx, |v, _| v.draw_each_redraw = Some(window));
            });
        }
        // A moment for the load's after-effects to settle, as a person would take.
        sleep(cx, 5000).await;
        let before = cx.update(|cx| overlay_counts(controller, cx));
        let drawing = || crate::overlay::DRAWING_NANOS.load(Ordering::Relaxed) as f64 / 1e9;
        let drawing0 = drawing();
        let (cpu0, start_ms, t0) = (easyvox_core::metrics::proc_stats().cpu_secs, unix_ms(), Instant::now());
        let mut finished = None;
        for n in 0..options.repeat {
            if n > 0 {
                // After the last one's close, and a moment.
                wait_for(cx, Duration::from_secs(10), |cx| !controller.read(cx).overlay.panel.is_visible()).await;
                sleep(cx, 300).await;
            }
            cx.update(|cx| {
                controller.update(cx, |c, cx| {
                    c.start_session(easyvox_core::AudioSource::WavReplay(wav.to_path_buf()), "replay", cx)
                })
            });
            // The session ends by itself at the end of the file.
            finished =
                wait_for(cx, Duration::from_secs_f64(audio_secs + 60.), |cx| controller.read(cx).is_idle()).await;
        }
        if options.repeat > 1 {
            // The last close, too.
            wait_for(cx, Duration::from_secs(10), |cx| !controller.read(cx).overlay.panel.is_visible()).await;
        }
        let (cpu, end_ms, wall) = (easyvox_core::metrics::proc_stats().cpu_secs - cpu0, unix_ms(), t0.elapsed());
        let after = cx.update(|cx| overlay_counts(controller, cx));
        let drawing_secs = drawing() - drawing0;
        let (tuning, app) = cx.update(|cx| (cx.global::<AppSettings>().tuning, cx.global::<AppSettings>().app));
        out = json!({
            "wav": wav, "audio_secs": audio_secs, "model_ready_ms": waited, "finished_ms": finished,
            "start_unix_ms": start_ms, "end_unix_ms": end_ms, "wall_secs": wall.as_secs_f64(),
            "cpu_secs": cpu, "cpu_pct_of_one_core": 100. * cpu / wall.as_secs_f64(),
            "meter_interval_ms": tuning.meter_interval_ms, "draw_each_redraw": options.draw,
            "levels_received": after.0 - before.0, "redraws_asked": after.1 - before.1, "renders": after.2 - before.2,
            "animation_frames": after.3 - before.3, "drawing_secs": drawing_secs,
            "drawing_pct_of_one_core": 100. * drawing_secs / wall.as_secs_f64(),
            "drawing_ms_per_render": 1e3 * drawing_secs / f64::from((after.2 - before.2).max(1)),
            "repeat": options.repeat, "reduce_motion": options.reduce_motion, "overlay_open_ms": app.overlay_open_ms, "overlay_close_ms": app.overlay_close_ms,
            "overlay_shrink_pct": app.overlay_shrink_pct, "overlay_words_ms": app.overlay_words_ms,
            "overlay_glide_ms": app.overlay_glide_ms, "overlay_frame_ms": app.overlay_frame_ms,
        });
    }
    match std::fs::write(&options.out, serde_json::to_string_pretty(&out).unwrap_or_default()) {
        Ok(()) => log::info!("self-test: replay report written to {}", options.out.display()),
        Err(e) => log::error!("self-test: couldn't write {}: {e}", options.out.display()),
    }
    let _ = std::fs::remove_dir_all(&options.dir);
    cx.update(|cx| cx.quit());
}

fn panel_state(controller: &Entity<Controller>, cx: &mut AsyncApp) -> PanelState {
    cx.update(|cx| controller.read(cx).overlay.panel.state())
}

fn panel_json(s: &PanelState) -> Value {
    json!({"class": s.class, "visible": s.visible, "key": s.key, "can_become_key": s.can_become_key,
           "ignores_mouse": s.ignores_mouse, "borderless": s.borderless, "nonactivating": s.nonactivating})
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
