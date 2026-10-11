//! The settings file: `~/.config/easyvox/config.toml` (see `easyvox_core::paths::config_file`
//! for `XDG_CONFIG_HOME` and `EASYVOX_CONFIG`). Agents and dotfiles can configure EasyVox through
//! it, and the app applies outside edits as soon as they're saved.
//!
//! The defaults live in code ([`AppSettings::default`], `AppTuning`, `easyvox_core::Tuning`);
//! the file only overrides them, key by key:
//!
//! - **Top level:** the keys the Settings window shows (`model`, `shortcut`, `hold_right_option`,
//!   `keep_model_loaded`). The window writes one when you change it there, and nothing else, so
//!   the file keeps your comments, your order and every other key as you wrote them.
//! - **`[advanced]`:** everything else, from the app's timings to every field of the pipeline's
//!   `Tuning`. The app never writes these: a key you didn't set follows the default, so a
//!   release can improve it.
//!
//! A missing key is its default. A value of the wrong type or out of range falls back to that
//! key's default, with a warning. An unknown key is ignored, with a warning. A file that isn't
//! valid TOML keeps the last good settings, logs the error, and is never overwritten.
//! `easyvox --print-default-config` prints every key at its default, with what it does.

use crate::state::{AppSettings, AppTuning};
use easyvox_core::Tuning;
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, Item, Value};

/// The table that holds every key the Settings window doesn't show.
pub const ADVANCED: &str = "advanced";

/// The keys the Settings window owns, at the top of the file, with what they do.
const UI_KEYS: [(&str, &str); 4] = [
    (
        "model",
        "The speech model: the id of one the app offers (`scripts/fetch-model.sh --list` marks them\n\
         default or alternative). Settings → Models downloads it, and the next dictation uses it.",
    ),
    (
        "shortcut",
        "The push-to-talk shortcut: hold it and speak, let go to paste. Modifiers (ctrl, alt, shift,\n\
         cmd) and a key, joined by \"-\", e.g. \"ctrl-shift-space\" or \"f5\".",
    ),
    ("hold_right_option", "Also dictate while the right Option (⌥) key is held on its own. Needs Input Monitoring."),
    (
        "keep_model_loaded",
        "Keep the speech model in memory while you're not dictating. Off, it's freed after\n\
         `unload_after_secs` (below) and loads again as your next dictation starts.",
    ),
];

/// The top of a settings file the app creates.
const NEW_FILE_HEADER: &str = "\
# EasyVox settings. The Settings window writes the keys it shows here; add any other key to
# override its default (advanced keys go under [advanced]). EasyVox applies changes as soon as
# you save. Every key, its default and what it does: `easyvox --print-default-config`, or
# docs/settings.md (https://github.com/brandonbews/easyvox/blob/main/docs/settings.md).

";

/// Settings read from a file, and what was wrong with it.
#[derive(Debug)]
pub struct Loaded {
    pub settings: AppSettings,
    /// One line per key that was ignored or put back to its default.
    pub warnings: Vec<String>,
}

/// Reads a settings file's text: the defaults, overridden by every valid key in it. `Err` only
/// when the text isn't TOML at all.
pub fn parse(text: &str) -> Result<Loaded, String> {
    let doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| describe(&e, text))?;
    let mut s = AppSettings::default();
    let mut warnings = Vec::new();
    for (key, item) in doc.iter() {
        match key {
            "model" => match item.as_str() {
                Some(id) => match model_problem(id) {
                    None => s.model = id.into(),
                    Some(problem) => warnings.push(format!("model: {id:?} {problem}; using the default")),
                },
                None => warnings.push(wrong_type(key, item, "a model id in quotes")),
            },
            "shortcut" => match item.as_str() {
                Some(k) if crate::hotkey::to_hotkey(k).is_ok() => s.shortcut = k.into(),
                Some(k) => warnings.push(format!("shortcut: can't read {k:?} as a shortcut; using the default")),
                None => warnings.push(wrong_type(key, item, "a shortcut in quotes, like \"alt-space\"")),
            },
            "hold_right_option" | "keep_model_loaded" => match item.as_bool() {
                Some(on) if key == "hold_right_option" => s.hold_right_option = on,
                Some(on) => s.keep_model_loaded = on,
                None => warnings.push(wrong_type(key, item, "true or false")),
            },
            ADVANCED => match item.as_table_like() {
                Some(table) => {
                    for (key, item) in table.iter() {
                        if let Err(e) = set_advanced(&mut s, key, item) {
                            warnings.push(format!("[{ADVANCED}] {e}"));
                        }
                    }
                }
                None => warnings.push(format!(
                    "{ADVANCED} should be a table (a line [advanced], then its keys), not {}; it's ignored",
                    item.type_name()
                )),
            },
            _ if is_advanced(key) => {
                warnings.push(format!("{key} belongs under [{ADVANCED}], so it's ignored here; using the default"));
            }
            _ => warnings.push(format!("unknown key {key:?}, ignored{}", did_you_mean(key))),
        }
    }
    for problem in s.app.validate() {
        warnings.push(format!("[{ADVANCED}] {problem}"));
    }
    for problem in s.tuning.validate() {
        warnings.push(format!("[{ADVANCED}] {problem}"));
    }
    Ok(Loaded { settings: s, warnings })
}

/// Sets one `[advanced]` key from the file. Out-of-range values are left for `validate`.
fn set_advanced(s: &mut AppSettings, key: &str, item: &Item) -> Result<(), String> {
    if !is_advanced(key) {
        return Err(if UI_KEYS.iter().any(|(k, _)| *k == key) {
            format!("{key} belongs at the top of the file, before [{ADVANCED}], so it's ignored here")
        } else {
            format!("unknown key {key:?}, ignored{}", did_you_mean(key))
        });
    }
    let text = match item.as_value() {
        Some(Value::Integer(n)) => n.value().to_string(),
        Some(Value::Float(x)) => x.value().to_string(),
        _ => return Err(wrong_type(key, item, "a number")),
    };
    let result = if AppTuning::KEYS.contains(&key) { s.app.set(key, &text) } else { s.tuning.set(key, &text) };
    result.map_err(|e| format!("{e}; using the default"))
}

fn is_advanced(key: &str) -> bool {
    AppTuning::KEYS.contains(&key) || Tuning::KEYS.contains(&key)
}

/// Why `id` can't be the app's model, or `None` if it can: it must be offered in the app (not
/// bench only) and run in this build.
fn model_problem(id: &str) -> Option<&'static str> {
    match easyvox_core::models::find(id) {
        Ok(m) if !m.offering.in_app() => Some("is for easyvox-bench only, not offered in the app"),
        Ok(m) if m.runtime.is_compiled() => None,
        _ => Some("isn't a model this app can run"),
    }
}

fn wrong_type(key: &str, item: &Item, expected: &str) -> String {
    format!("{key} should be {expected}, not {}; using the default", item.type_name())
}

/// " (did you mean `x`?)" for the known key nearest a misspelt one, if one is near.
fn did_you_mean(key: &str) -> String {
    let known = UI_KEYS.iter().map(|(k, _)| *k).chain([ADVANCED]).chain(AppTuning::KEYS.iter().copied());
    known
        .chain(Tuning::KEYS.iter().copied())
        .map(|k| (edit_distance(key, k), k))
        .filter(|(d, k)| *d <= 3 && *d < k.len() / 2)
        .min()
        .map_or(String::new(), |(_, k)| format!(" (did you mean {k:?}?)"))
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitution = diagonal + usize::from(ca != *cb);
            diagonal = row[j + 1];
            row[j + 1] = substitution.min(row[j] + 1).min(row[j + 1] + 1);
        }
    }
    row[b.len()]
}

/// A TOML error on one line: "line 3, column 9: invalid string".
fn describe(e: &toml_edit::TomlError, text: &str) -> String {
    let message = e.message().trim().replace('\n', "; ");
    match e.span() {
        Some(span) => {
            let before = &text[..span.start.min(text.len())];
            let line = before.matches('\n').count() + 1;
            let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
            format!("line {line}, column {column}: {message}")
        }
        None => message,
    }
}

/// The file's state as the app last saw it.
pub struct ConfigFile {
    path: PathBuf,
    /// The text last read or written (empty for a missing file), so a reload can tell an outside
    /// edit from the app's own write or a repeated event. `None` forces the next reload to
    /// apply whatever is there.
    last_text: Option<String>,
}

/// What a reload found.
#[derive(Debug)]
pub enum Reload {
    /// The text is what the app last read or wrote: its own save, or a repeated event.
    Unchanged,
    /// Boxed: the settings are far bigger than the other variants.
    Changed(Box<Loaded>),
    /// Not TOML: keep the settings in force.
    Invalid(String),
}

impl ConfigFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path, last_text: None }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file's text, or "" when there's no file.
    fn read(&self) -> Result<String, String> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => Ok(text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(format!("can't read {}: {e}", self.path.display())),
        }
    }

    /// At startup: the file's settings, or the defaults (and the error) when it can't be used.
    pub fn load(&mut self) -> (Loaded, Option<String>) {
        self.last_text = None;
        let defaults = || Loaded { settings: AppSettings::default(), warnings: vec![] };
        match self.reload() {
            Reload::Changed(loaded) => (*loaded, None),
            Reload::Invalid(error) => (defaults(), Some(error)),
            Reload::Unchanged => (defaults(), None), // can't happen: nothing was read before
        }
    }

    /// Reads the file again after a change was reported.
    pub fn reload(&mut self) -> Reload {
        let text = match self.read() {
            Ok(text) => text,
            Err(e) => return Reload::Invalid(e),
        };
        if self.last_text.as_deref() == Some(text.as_str()) {
            return Reload::Unchanged;
        }
        let result = parse(&text).map_err(|e| format!("{}: {e}", self.path.display()));
        // Remember broken text too, so repeated events for it don't log the error again.
        self.last_text = Some(text);
        match result {
            Ok(loaded) => Reload::Changed(Box::new(loaded)),
            Err(e) => Reload::Invalid(e),
        }
    }

    /// Writes the Settings window's keys that differ from what the file says (its value, or the
    /// default when it doesn't set one), and nothing else: comments, order and every other key
    /// stay as they are. Returns whether it wrote. Refuses to touch a file that isn't valid TOML.
    pub fn save(&mut self, settings: &AppSettings) -> Result<bool, String> {
        let text = self.read()?;
        let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| {
            format!("{} has an error, so it wasn't changed: {}", self.path.display(), describe(&e, &text))
        })?;
        let current = parse(&text)?.settings;
        let mut changed = false;
        let mut set = |key: &str, value: Value| {
            changed = true;
            match doc.get_mut(key) {
                // Keep the comment after the old value.
                Some(Item::Value(old)) => {
                    let decor = old.decor().clone();
                    *old = value;
                    *old.decor_mut() = decor;
                }
                _ => {
                    doc.insert(key, Item::Value(value));
                }
            }
        };
        if settings.model != current.model {
            set("model", settings.model.as_str().into());
        }
        if settings.shortcut != current.shortcut {
            set("shortcut", settings.shortcut.as_str().into());
        }
        if settings.hold_right_option != current.hold_right_option {
            set("hold_right_option", settings.hold_right_option.into());
        }
        if settings.keep_model_loaded != current.keep_model_loaded {
            set("keep_model_loaded", settings.keep_model_loaded.into());
        }
        if !changed {
            return Ok(false);
        }
        let new_text = if text.trim().is_empty() {
            format!("{NEW_FILE_HEADER}{}", doc.to_string().trim_start())
        } else {
            doc.to_string()
        };
        write_atomically(&self.path, &new_text).map_err(|e| format!("can't write {}: {e}", self.path.display()))?;
        // If the file had changed since the app last read it, the next reload applies that
        // outside edit (now merged with this one); otherwise this write is the app's own, and
        // the reload its watcher triggers finds nothing new.
        let up_to_date = self.last_text.as_deref() == Some(text.as_str());
        self.last_text = up_to_date.then_some(new_text);
        Ok(true)
    }
}

/// Replaces `path` in one step (a reader never sees half a file), keeping its permissions.
/// Through a symlink it writes where the link points, so a link into a dotfiles repo stays.
fn write_atomically(path: &Path, text: &str) -> std::io::Result<()> {
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let dir = target.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = target.file_name().map_or("config.toml".into(), |n| n.to_string_lossy());
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    std::fs::write(&tmp, text)?;
    if let Ok(meta) = std::fs::metadata(&target) {
        let _ = std::fs::set_permissions(&tmp, meta.permissions());
    }
    std::fs::rename(&tmp, &target).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Keeps the settings file watched while it lives.
pub struct Watcher {
    _inner: notify::RecommendedWatcher,
}

/// Calls `on_change` (on a background thread) whenever the settings file may have changed:
/// written, replaced, created or deleted, also through a symlink to it. It uses FSEvents, so
/// nothing runs while nothing changes. The file's folder is created if it's missing, since
/// FSEvents can only watch a folder that exists. Repeated calls for one save are normal; the
/// caller reads the file again and compares (see [`ConfigFile::reload`]).
pub fn watch(path: &Path, on_change: impl Fn() + Send + 'static) -> anyhow::Result<Watcher> {
    use notify::Watcher as _;
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    // Watch the real folders (FSEvents reports real paths): the file's, and where a symlinked
    // file points.
    let mut dirs = vec![std::fs::canonicalize(dir)?];
    let mut names = vec![path.file_name().map(ToOwned::to_owned).unwrap_or_default()];
    if let Ok(target) = std::fs::canonicalize(path)
        && let (Some(target_dir), Some(name)) = (target.parent(), target.file_name())
    {
        if !dirs.iter().any(|d| d == target_dir) {
            dirs.push(target_dir.to_path_buf());
        }
        names.push(name.to_owned());
    }
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
        Ok(event) => {
            let ours = event.paths.iter().any(|p| p.file_name().is_some_and(|n| names.iter().any(|m| m == n)));
            if ours || event.need_rescan() {
                on_change();
            }
        }
        Err(e) => log::warn!("settings file watch: {e}"),
    })?;
    for dir in &dirs {
        watcher.watch(dir, notify::RecursiveMode::NonRecursive)?;
    }
    Ok(Watcher { _inner: watcher })
}

/// Every key at its default, with what it does, its unit and its range, as TOML comments:
/// what `easyvox --print-default-config` prints and `docs/settings.md` shows.
pub fn default_config_text() -> String {
    let defaults = AppSettings::default();
    let mut out = String::from(
        "\
# EasyVox settings: every key, at its default.
#
# This is a reference, not a file to copy whole. Your settings file only needs the keys you
# change: anything you leave out follows the default, so a release that improves one reaches
# you. The file is ~/.config/easyvox/config.toml ($XDG_CONFIG_HOME/easyvox/config.toml when
# that's set; EASYVOX_CONFIG points at another file). EasyVox applies a change as soon as you
# save it: the [advanced] numbers for the live preview, the chunks, the pauses and the level
# meter from your next dictation, the same for every model, and the overlay's animations from
# the next time it opens.
",
    );
    // Doc comments come one line per source line, each starting with a space.
    let comment = |out: &mut String, doc: &str| {
        for line in doc.lines().map(str::trim) {
            let line = format!("# {line}");
            out.push_str(line.trim_end());
            out.push('\n');
        }
    };
    let quoted = |s: &str| Value::from(s).to_string().trim().to_string();
    for (key, doc) in UI_KEYS {
        out.push('\n');
        comment(&mut out, doc);
        let value = match key {
            "model" => quoted(&defaults.model),
            "shortcut" => quoted(&defaults.shortcut),
            "hold_right_option" => defaults.hold_right_option.to_string(),
            _ => defaults.keep_model_loaded.to_string(),
        };
        out.push_str(&format!("{key} = {value}\n"));
    }
    out.push_str(&format!(
        "\n[{ADVANCED}]\n# Settings the window doesn't show: how long the model stays loaded, how long the\n\
         # overlay stays up and how it moves, then the numbers behind the live preview, the\n\
         # chunks, what counts as speech or a pause, and the overlay's level meter.\n"
    ));
    for field in defaults.app.fields().into_iter().chain(defaults.tuning.fields()) {
        out.push('\n');
        comment(&mut out, field.doc);
        out.push_str(&format!("# In {}, from {} to {}.\n", field.unit(), field.min, field.max));
        out.push_str(&format!("{} = {}\n", field.key, field.default));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// A fresh, empty folder for one test.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("easyvox-config-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn loaded(text: &str) -> Loaded {
        parse(text).unwrap()
    }

    #[test]
    fn an_empty_file_is_the_defaults() {
        let l = loaded("");
        assert_eq!(l.settings, AppSettings::default());
        assert!(l.warnings.is_empty());
    }

    #[test]
    fn keys_override_their_defaults_one_by_one() {
        let l = loaded(
            "keep_model_loaded = true\nshortcut = \"ctrl-shift-k\"\n\n[advanced]\nunload_after_secs = 60\n\
             preview_interval_ms = 800\ngate_silence_dbfs = -48.5\npause_quiet_db = -26\n",
        );
        assert!(l.warnings.is_empty(), "{:?}", l.warnings);
        let mut expected =
            AppSettings { keep_model_loaded: true, shortcut: "ctrl-shift-k".into(), ..Default::default() };
        expected.app.unload_after_secs = 60;
        expected.tuning.preview_interval_ms = 800;
        expected.tuning.gate_silence_dbfs = -48.5;
        expected.tuning.pause_quiet_db = -26.0;
        assert_eq!(l.settings, expected);
    }

    /// Loudness is in decibels in the file, and the pipeline turns it into the levels it
    /// compares: -48.5 dBFS is about 0.0038 of full scale.
    #[test]
    fn loudness_is_read_in_decibels() {
        let t = loaded("[advanced]\ngate_silence_dbfs = -48.5\ngate_quiet_db = -6\n").settings.tuning;
        assert!((easyvox_core::tuning::db_to_linear(t.gate_silence_dbfs) - 0.003_758).abs() < 1e-6);
        assert!((easyvox_core::tuning::db_to_linear(t.gate_quiet_db) - 0.501_2).abs() < 1e-4);
        // A linear level from before the switch is out of range: it falls back, with a warning.
        // A linear level from before the switch (always above 0 dB) is out of range: it falls
        // back to the default, with a warning.
        let l = loaded("[advanced]\ngate_silence_dbfs = 0.0018\ngate_quiet_db = 0.1\n");
        assert_eq!(l.settings.tuning, Tuning::default());
        let joined = l.warnings.join("\n");
        assert!(joined.contains("gate_silence_dbfs = 0.0018 is outside -120..=0, so it stays at its default, -55"));
        assert!(joined.contains("gate_quiet_db = 0.1 is outside -60..=0"), "{joined}");
    }

    #[test]
    fn an_out_of_range_value_falls_back_for_that_key_only() {
        let l = loaded(
            "[advanced]\npreview_interval_ms = 60000\nchunk_ms = 15000\noverlay_max_chars = 3\n\
             preview_window_ms = -5\ngate_quiet_db = nan\n",
        );
        let s = &l.settings;
        assert_eq!(s.tuning, Tuning { chunk_ms: 15_000, ..Tuning::default() });
        assert_eq!(s.app, AppTuning::default());
        let joined = l.warnings.join("\n");
        assert_eq!(l.warnings.len(), 4, "{joined}");
        assert!(joined.contains("preview_interval_ms = 60000 is outside 0..=10000, so it stays at its default, 1600"));
        assert!(joined.contains("overlay_max_chars = 3 is outside 10..=500"));
        assert!(
            joined.contains(r#"preview_window_ms: can't read "-5" as a whole number, 0 or more; using the default"#)
        );
        assert!(joined.contains("gate_quiet_db = NaN is outside"), "{joined}");
    }

    #[test]
    fn wrong_types_and_bad_values_fall_back_with_a_warning() {
        let l = loaded(
            "model = \"no-such-model\"\nshortcut = 5\nhold_right_option = \"yes\"\nkeep_model_loaded = true\n\
             [advanced]\nchunk_ms = \"30 s\"\n",
        );
        assert_eq!(l.settings, AppSettings { keep_model_loaded: true, ..Default::default() });
        let joined = l.warnings.join("\n");
        assert_eq!(l.warnings.len(), 4, "{joined}");
        assert!(joined.contains("model: \"no-such-model\" isn't a model this app can run"), "{joined}");
        assert!(joined.contains("shortcut should be a shortcut in quotes"), "{joined}");
        assert!(joined.contains("hold_right_option should be true or false, not string"), "{joined}");
        assert!(joined.contains("chunk_ms should be a number, not string"), "{joined}");
    }

    /// Parakeet Unified left the app on 10 October 2026 (bench only). A file that still names it
    /// gets the default model, with a warning, and isn't rewritten for it.
    #[test]
    fn a_bench_only_model_falls_back_to_the_default() {
        let l = loaded("model = \"parakeet-unified-en-0.6b-q5_k_m-gguf\"\nkeep_model_loaded = true\n");
        assert_eq!(l.settings, AppSettings { keep_model_loaded: true, ..Default::default() });
        assert_eq!(l.settings.model, easyvox_core::RECOMMENDED_MODEL);
        assert_eq!(
            l.warnings,
            [
                r#"model: "parakeet-unified-en-0.6b-q5_k_m-gguf" is for easyvox-bench only, not offered in the app; using the default"#
            ]
        );
        // The offered alternative is taken as it is.
        let l = loaded("model = \"whisper-base.en-q8_0-gguf\"\n");
        assert!(l.warnings.is_empty(), "{:?}", l.warnings);
        assert_eq!(l.settings.model, "whisper-base.en-q8_0-gguf");
    }

    #[test]
    fn unknown_and_misplaced_keys_warn_but_dont_fail() {
        let l = loaded(
            "colour = \"blue\"\npreview_interval_ms = 800\nkeep_model_loaded = true\n\n[advanced]\n\
             previw_interval_ms = 900\nhold_right_option = true\nchunk_ms = 20000\n\n[extras]\nx = 1\n",
        );
        assert_eq!(l.settings.tuning, Tuning { chunk_ms: 20_000, ..Tuning::default() });
        assert!(l.settings.keep_model_loaded && !l.settings.hold_right_option);
        let joined = l.warnings.join("\n");
        assert_eq!(l.warnings.len(), 5, "{joined}");
        assert!(joined.contains(r#"unknown key "colour", ignored"#), "{joined}");
        assert!(joined.contains("preview_interval_ms belongs under [advanced]"), "{joined}");
        assert!(joined.contains(r#"unknown key "previw_interval_ms", ignored (did you mean "preview_interval_ms"?)"#));
        assert!(joined.contains("hold_right_option belongs at the top of the file"), "{joined}");
        assert!(joined.contains(r#"unknown key "extras""#), "{joined}");
    }

    #[test]
    fn invalid_toml_is_an_error_with_its_line() {
        let err = parse("keep_model_loaded = true\nshortcut = alt-space\n").unwrap_err();
        assert!(err.starts_with("line 2, column 12:"), "{err}");
    }

    #[test]
    fn the_reference_parses_back_to_the_defaults_and_lists_every_key_once() {
        let text = default_config_text();
        let l = loaded(&text);
        assert_eq!(l.settings, AppSettings::default());
        assert!(l.warnings.is_empty(), "{:?}", l.warnings);
        let doc: DocumentMut = text.parse().unwrap();
        let mut keys: Vec<&str> = doc.iter().map(|(k, _)| k).filter(|k| *k != ADVANCED).collect();
        keys.extend(doc[ADVANCED].as_table().unwrap().iter().map(|(k, _)| k));
        let mut expected: Vec<&str> = UI_KEYS.iter().map(|(k, _)| *k).collect();
        expected.extend(AppTuning::KEYS);
        expected.extend(Tuning::KEYS);
        assert_eq!(keys, expected);
        assert_eq!(keys.len(), 45);
        for key in AppTuning::KEYS.iter().chain(Tuning::KEYS) {
            assert!(text.contains(&format!("\n{key} = ")), "{key}");
            assert!(!easyvox_core::tuning::unit_of(key).is_empty(), "{key}: end the key in a unit");
        }
        assert!(text.contains("# In percent, from 0 to 30.\noverlay_shrink_pct = 15\n"));
        assert!(
            text.contains("# In dBFS (decibels relative to full scale), from -120 to 0.\ngate_silence_dbfs = -55\n")
        );
    }

    /// docs/settings.md shows `easyvox --print-default-config` word for word, so it can't drift
    /// from the defaults in code. After changing a key, its docs or a default, paste the new
    /// output in (`cargo run -p easyvox-app -- --print-default-config`).
    #[test]
    fn the_settings_doc_shows_the_current_reference() {
        let doc = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/settings.md")).unwrap();
        let block = format!("```toml\n{}```", default_config_text());
        assert!(doc.contains(&block), "docs/settings.md is out of date: paste in `easyvox --print-default-config`");
    }

    #[test]
    fn settings_round_trip_through_the_file() {
        let dir = temp_dir("round-trip");
        let path = dir.join("config.toml");
        let mut file = ConfigFile::new(path.clone());
        let (first, error) = file.load();
        assert_eq!((first.settings, error), (AppSettings::default(), None));
        let mut settings = AppSettings {
            shortcut: "ctrl-shift-space".into(),
            hold_right_option: true,
            keep_model_loaded: true,
            ..Default::default()
        };
        settings.tuning.chunk_ms = 15_000; // advanced: in memory only, never written
        assert!(file.save(&settings).unwrap());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# EasyVox settings."), "{text}");
        let (again, error) = ConfigFile::new(path.clone()).load();
        assert_eq!(error, None);
        assert_eq!(again.settings, AppSettings { tuning: Tuning::default(), ..settings.clone() });
        // Only the keys that differ from the defaults were written: no model, nothing advanced.
        let doc: DocumentMut = text.parse().unwrap();
        let keys: Vec<&str> = doc.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, ["shortcut", "hold_right_option", "keep_model_loaded"], "{text}");
        // Saving the same settings again writes nothing.
        assert!(!file.save(&settings).unwrap());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn saving_keeps_comments_order_and_advanced_keys() {
        let dir = temp_dir("keep");
        let path = dir.join("config.toml");
        let original = "# my settings\nkeep_model_loaded = false  # for now\n\n[advanced]\n# snappier\n\
                        preview_interval_ms = 800\n";
        std::fs::write(&path, original).unwrap();
        let mut file = ConfigFile::new(path.clone());
        let (l, _) = file.load();
        let settings = AppSettings { keep_model_loaded: true, shortcut: "f5".into(), ..l.settings };
        assert!(file.save(&settings).unwrap());
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            "# my settings\nkeep_model_loaded = true  # for now\nshortcut = \"f5\"\n\n[advanced]\n# snappier\n\
             preview_interval_ms = 800\n"
        );
        assert_eq!(parse(&text).unwrap().settings, settings);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_invalid_file_keeps_the_last_good_settings_and_is_never_overwritten() {
        let dir = temp_dir("invalid");
        let path = dir.join("config.toml");
        std::fs::write(&path, "keep_model_loaded = true\n").unwrap();
        let mut file = ConfigFile::new(path.clone());
        let (good, _) = file.load();
        assert!(good.settings.keep_model_loaded);
        let broken = "keep_model_loaded = tru\n";
        std::fs::write(&path, broken).unwrap();
        match file.reload() {
            Reload::Invalid(e) => assert!(e.contains("line 1, column 21"), "{e}"),
            other => panic!("{other:?}"),
        }
        // A second event for the same broken text doesn't report it again.
        assert!(matches!(file.reload(), Reload::Unchanged));
        let err = file.save(&AppSettings { hold_right_option: true, ..good.settings.clone() }).unwrap_err();
        assert!(err.contains("has an error, so it wasn't changed"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
        // At startup, a broken file means the defaults, and the error to show.
        let (l, error) = ConfigFile::new(path.clone()).load();
        assert_eq!(l.settings, AppSettings::default());
        assert!(error.is_some_and(|e| e.contains("config.toml: line 1")));
        // Fixed, it applies again.
        std::fs::write(&path, "hold_right_option = true\n").unwrap();
        match file.reload() {
            Reload::Changed(l) => assert!(l.settings.hold_right_option && !l.settings.keep_model_loaded),
            other => panic!("{other:?}"),
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn saving_through_a_symlink_keeps_the_link() {
        let dir = temp_dir("symlink");
        let real = dir.join("dotfiles.toml");
        std::fs::write(&real, "[advanced]\nchunk_ms = 20000\n").unwrap();
        let link = dir.join("config.toml");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let mut file = ConfigFile::new(link.clone());
        let (l, _) = file.load();
        assert!(file.save(&AppSettings { keep_model_loaded: true, ..l.settings }).unwrap());
        assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&real).unwrap(), "keep_model_loaded = true\n[advanced]\nchunk_ms = 20000\n");
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// The watcher reports an outside edit, and the reload applies it. The app's own save is
    /// reported too (FSEvents can't tell who wrote), but the reload finds nothing new, so
    /// nothing is applied and nothing is written: no loop.
    #[test]
    fn outside_edits_reload_live_and_own_writes_dont_loop() {
        let dir = temp_dir("watch");
        let path = dir.join("config.toml");
        std::fs::write(&path, "keep_model_loaded = false\n").unwrap();
        let mut file = ConfigFile::new(path.clone());
        let (l, _) = file.load();
        let (tx, rx) = mpsc::channel();
        let _watcher = watch(&path, move || {
            let _ = tx.send(());
        })
        .unwrap();
        let drain = |rx: &mpsc::Receiver<()>| {
            std::thread::sleep(Duration::from_millis(300));
            while rx.try_recv().is_ok() {}
        };

        std::fs::write(&path, "keep_model_loaded = true\n[advanced]\noverlay_max_chars = 40\n").unwrap();
        rx.recv_timeout(Duration::from_secs(10)).expect("no event for an outside edit");
        let settings = match file.reload() {
            Reload::Changed(l) => l.settings,
            other => panic!("{other:?}"),
        };
        assert!(settings.keep_model_loaded && settings.app.overlay_max_chars == 40);
        assert_ne!(settings, l.settings);
        drain(&rx);

        assert!(file.save(&AppSettings { hold_right_option: true, ..settings }).unwrap());
        let written = std::fs::read_to_string(&path).unwrap();
        rx.recv_timeout(Duration::from_secs(10)).expect("no event for the app's own write");
        assert!(matches!(file.reload(), Reload::Unchanged), "the app's own write was reloaded");
        drain(&rx);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), written, "the reload wrote the file");
        // A late event for the same write (FSEvents may split one) finds nothing new either.
        if rx.recv_timeout(Duration::from_millis(500)).is_ok() {
            assert!(matches!(file.reload(), Reload::Unchanged));
            drain(&rx);
        }

        // A replace by rename (how many editors save) is seen too.
        let tmp = dir.join("config.toml.swp");
        std::fs::write(&tmp, "hold_right_option = false\n").unwrap();
        std::fs::rename(&tmp, &path).unwrap();
        rx.recv_timeout(Duration::from_secs(10)).expect("no event for a rename");
        assert!(matches!(file.reload(), Reload::Changed(l) if !l.settings.hold_right_option));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn did_you_mean_suggests_only_near_keys() {
        assert_eq!(did_you_mean("chunk_msec"), r#" (did you mean "chunk_ms"?)"#);
        assert_eq!(did_you_mean("advance"), r#" (did you mean "advanced"?)"#);
        assert_eq!(did_you_mean("colour"), "");
    }
}
