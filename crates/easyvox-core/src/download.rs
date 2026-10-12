//! Downloading a catalog model: fetch it, check its size and sha256, then rename it into place.
//!
//! [`download`] is the one way the app gets a model, the default and the alternatives alike.
//! Its guarantees:
//!
//! - **Nothing at the model's path unless it's verified.** Bytes go to a hidden
//!   `.<file>.part` next to the model, hashed as they arrive. Only when the size and sha256 match
//!   the catalog is the file flushed to disk and renamed into place, which is atomic.
//! - **Nothing left behind on failure or cancel.** The part file is deleted on every way out,
//!   panics included. A part file left by a crash or a force quit is swept at the next launch
//!   ([`remove_partials`]).
//! - **Resume only within one call.** A dropped or stalled connection is picked up where it
//!   stopped (`Range: bytes=N-`), from the pinned Hugging Face URL each time, up to
//!   [`DownloadOptions::max_failures`] tries in a row without new bytes. That covers Wi-Fi
//!   dropping out and the Mac sleeping. A cancelled or failed download starts again from zero.
//! - **No request for a model that's already there.** A copy that checks out is kept as it is.
//! - **The license travels with the file.** Once a model checks out, its license, credit,
//!   modification note and disclaimer are written next to it, as `<file>.LICENSE.txt`
//!   ([`ModelInfo::license_notice`]). [`delete`] removes both.
//!
//! On macOS the bytes come through NSURLSession (`download/macos.rs`), so System Settings
//! proxies apply. The rest of this module is plain Rust. `scripts/fetch-model.sh` does the same
//! for developers, with its own `.part.XXXXXX` temp files, so the two never touch each other's.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as transport;

use crate::models::ModelInfo;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Free space a download leaves on top of the model itself.
const SPACE_MARGIN: u64 = 64 * 1024 * 1024;
/// Write buffer, and read size when hashing a copy already on disk.
const CHUNK: usize = 1024 * 1024;
/// The most often [`Progress::Downloading`] is reported.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);
/// How often a backoff wait looks at the cancel token.
const BACKOFF_SLICE: Duration = Duration::from_millis(100);

/// Cancels a running [`download`] from any thread. Cheap to clone.
#[derive(Clone, Default, Debug)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// What [`download`] reports while it runs. `done` never decreases, and a success always ends
/// with `Downloading { done: total, total }`. At most about 4 events a second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    /// Checking the sha256 of a copy that is already on disk.
    Verifying,
    /// Bytes downloaded and hashed so far, out of the catalog's `download_bytes`.
    Downloading { done: u64, total: u64 },
    /// The connection dropped or stalled. After a short wait, try number `attempt` starts from
    /// `done`; tries are counted since bytes last arrived, and after `max` of them that bring
    /// nothing the download fails.
    Reconnecting { attempt: u32, max: u32, done: u64, total: u64 },
}

/// Why a download failed. Whatever the reason, nothing is left behind: no file at the model's
/// path, no partial file. `Display` gives one sentence for the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DownloadError {
    Cancelled,
    /// The catalog entry isn't a single file with a checksum (the bench-only sherpa archives).
    NotDownloadable,
    /// This model is already downloading (here or in another process).
    Busy,
    NotEnoughSpace {
        needed: u64,
        available: u64,
    },
    /// No connection to the server after retrying (no network, DNS, connection refused).
    Offline(String),
    /// Another network failure after retrying, or one retrying can't fix (TLS, bad redirect).
    Network(String),
    /// The server answered with an error status (404 means the catalog URL is wrong).
    Http(u16),
    SizeMismatch {
        expected: u64,
        actual: u64,
    },
    ChecksumMismatch {
        expected: String,
        actual: String,
    },
    /// Reading or writing the models folder failed.
    Disk(String),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "The download was cancelled."),
            Self::NotDownloadable => write!(f, "This model can't be downloaded in the app."),
            Self::Busy => write!(f, "This model is already downloading."),
            Self::NotEnoughSpace { needed, available } => write!(
                f,
                "Not enough disk space: the download needs {}, and {} is free.",
                format_size(*needed),
                format_size(*available)
            ),
            Self::Offline(e) => write!(f, "Couldn't reach the server. {}", sentence(e)),
            Self::Network(e) => write!(f, "The download failed. {}", sentence(e)),
            Self::Http(404) => write!(f, "The server doesn't have this model (HTTP 404)."),
            Self::Http(code) => write!(f, "The server answered with an error (HTTP {code})."),
            Self::SizeMismatch { expected, actual } => write!(
                f,
                "The server sent a file of the wrong size ({actual} bytes instead of {expected}). Nothing was kept."
            ),
            Self::ChecksumMismatch { .. } => {
                write!(f, "The downloaded file isn't the expected one (its sha256 doesn't match). Nothing was kept.")
            }
            Self::Disk(e) => write!(f, "Couldn't save the model. {}", sentence(e)),
        }
    }
}

impl std::error::Error for DownloadError {}

/// `e` as a sentence: starts with a capital and ends with a full stop.
fn sentence(e: &str) -> String {
    let e = e.trim();
    let mut chars = e.chars();
    let mut s: String = chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default();
    if !s.ends_with(['.', '!', '?']) {
        s.push('.');
    }
    s
}

/// A size in decimal units, as Finder shows it: "85 MB", "539 MB", "1.2 GB".
pub fn format_size(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} GB", bytes as f64 / 1e9)
    } else {
        format!("{} MB", (bytes as f64 / 1e6).round() as u64)
    }
}

/// Tuning knobs. `Default` is what the app uses; tests shorten the waits.
#[derive(Clone, Debug)]
pub struct DownloadOptions {
    /// An attempt fails after this long without a byte (the Mac slept, the network went away).
    pub idle_timeout: Duration,
    /// Give up after this many tries in a row that brought no new bytes.
    pub max_failures: u32,
    /// The wait before a retry, doubling with each try in a row that brings nothing.
    pub first_backoff: Duration,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self { idle_timeout: Duration::from_secs(30), max_failures: 5, first_backoff: Duration::from_secs(1) }
    }
}

/// Blocking: call it off the UI thread. Downloads `model` into `models_dir`, checks the size
/// and sha256 while streaming, and only then renames it into place. A copy that's already
/// there and checks out returns at once, without a request. `progress` runs on this thread.
pub fn download(
    model: &ModelInfo,
    models_dir: &Path,
    cancel: &CancelToken,
    progress: impl FnMut(Progress),
) -> Result<PathBuf, DownloadError> {
    download_with(model, models_dir, &DownloadOptions::default(), cancel, progress)
}

/// [`download`] with its own [`DownloadOptions`].
pub fn download_with(
    model: &ModelInfo,
    models_dir: &Path,
    options: &DownloadOptions,
    cancel: &CancelToken,
    mut progress: impl FnMut(Progress),
) -> Result<PathBuf, DownloadError> {
    let Some(expected) = model.sha256 else { return Err(DownloadError::NotDownloadable) };
    let total = model.download_bytes;
    let target = model.resolve(models_dir);
    let mut reporter = Reporter::new(&mut progress);

    if target.exists() {
        reporter.emit(Progress::Verifying);
        if copy_checks_out(&target, total, expected, cancel)? {
            write_license_notice(model, models_dir);
            reporter.finish(total);
            return Ok(target);
        }
        log::warn!("download: {} is damaged; downloading it again", target.display());
        fs::remove_file(&target).map_err(|e| disk(&target, e))?;
    }

    let part_path = model.partial_path(models_dir);
    let dir = part_path.parent().unwrap_or(models_dir);
    fs::create_dir_all(dir).map_err(|e| disk(dir, e))?;
    let part = PartFile::create(&part_path)?;
    let needed = total + SPACE_MARGIN;
    check_space(needed, available_space(dir).map_err(|e| disk(dir, e))?)?;

    let mut out = BufWriter::with_capacity(CHUNK, &part.file);
    let mut hasher = Sha256::new();
    let mut done = 0u64;
    let mut failures = 0u32;
    loop {
        let before = done;
        let mut skip = 0u64;
        let mut sink = |chunk: Fetched| -> Result<(), DownloadError> {
            match chunk {
                Fetched::Start { offset, total: announced } => {
                    if let Some(actual) = announced.filter(|&n| n != total) {
                        return Err(DownloadError::SizeMismatch { expected: total, actual });
                    }
                    if offset > done {
                        return Err(DownloadError::Network(format!(
                            "The server resumed at byte {offset}, past the {done} bytes already here"
                        )));
                    }
                    // 0 for a 206; with a 200 the bytes already here arrive again and are skipped.
                    skip = done - offset;
                }
                Fetched::Data(mut bytes) => {
                    let skipped = skip.min(bytes.len() as u64);
                    bytes = bytes.get(skipped as usize..).unwrap_or_default();
                    skip -= skipped;
                    if bytes.is_empty() {
                        return Ok(());
                    }
                    let len = bytes.len() as u64;
                    if done + len > total {
                        return Err(DownloadError::SizeMismatch { expected: total, actual: done + len });
                    }
                    out.write_all(bytes).map_err(|e| write_error(&part.path, e, done, needed))?;
                    hasher.update(bytes);
                    done += len;
                    reporter.downloading(done, total);
                }
            }
            Ok(())
        };
        let result = transport::fetch(model.download_url, before, options.idle_timeout, cancel, &mut sink);
        let retry = match result {
            Ok(()) if done == total => break,
            Ok(()) => Retry::Transient("The connection closed before the end of the file".into()),
            Err(FetchError::Cancelled) => return Err(DownloadError::Cancelled),
            Err(FetchError::Sink(e)) => return Err(e),
            Err(FetchError::Fatal(e)) => return Err(DownloadError::Network(e)),
            Err(FetchError::Http(code)) if code == 408 || code == 429 || (500..600).contains(&code) => {
                Retry::Http(code)
            }
            Err(FetchError::Http(code)) => return Err(DownloadError::Http(code)),
            Err(FetchError::Offline(e)) => Retry::Offline(e),
            Err(FetchError::Transient(e)) => Retry::Transient(e),
        };
        if cancel.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }
        // Tries in a row without new bytes. One that brought bytes starts the count again, so
        // a connection that keeps dropping but keeps making progress is never given up on.
        failures = if done > before { 0 } else { failures + 1 };
        log::warn!(
            "download: {}: stopped at byte {done} ({failures} tries in a row with nothing new): {retry:?}",
            model.id
        );
        if failures >= options.max_failures {
            return Err(retry.into_error());
        }
        reporter.emit(Progress::Reconnecting { attempt: failures + 1, max: options.max_failures, done, total });
        // 1, 1, 2, 4, 8 s by default.
        let wait = options.first_backoff.saturating_mul(1 << failures.saturating_sub(1).min(16));
        if !sleep_unless_cancelled(wait, cancel) {
            return Err(DownloadError::Cancelled);
        }
    }

    let actual = hex(&hasher.finalize());
    if actual != expected {
        return Err(DownloadError::ChecksumMismatch { expected: expected.to_string(), actual });
    }
    out.flush().map_err(|e| write_error(&part.path, e, done, needed))?;
    drop(out);
    part.activate(&target)?;
    write_license_notice(model, models_dir);
    reporter.finish(total);
    log::info!("download: {} verified and saved to {}", model.id, target.display());
    Ok(target)
}

/// Writes the model's license notice next to it, replacing any older one in one step (through a
/// hidden `.part` file, which [`remove_partials`] sweeps if a crash leaves it). The model is
/// already verified and in place, so a failure here is logged, not returned.
fn write_license_notice(model: &ModelInfo, models_dir: &Path) {
    let path = model.license_notice_path(models_dir);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.part"));
    let result = fs::write(&tmp, model.license_notice())
        .and_then(|()| fs::set_permissions(&tmp, fs::Permissions::from_mode(0o644)))
        .and_then(|()| fs::rename(&tmp, &path));
    if let Err(e) = result {
        log::warn!("download: couldn't write {}: {e}", path.display());
        let _ = fs::remove_file(&tmp);
    }
}

/// What a failed attempt was, for the error if it's the last one.
#[derive(Debug)]
enum Retry {
    Offline(String),
    Transient(String),
    Http(u16),
}

impl Retry {
    fn into_error(self) -> DownloadError {
        match self {
            Retry::Offline(e) => DownloadError::Offline(e),
            Retry::Transient(e) => DownloadError::Network(e),
            Retry::Http(code) => DownloadError::Http(code),
        }
    }
}

/// Room for `needed` bytes when `available` are free?
fn check_space(needed: u64, available: u64) -> Result<(), DownloadError> {
    if available < needed { Err(DownloadError::NotEnoughSpace { needed, available }) } else { Ok(()) }
}

/// Waits `wait`, a slice at a time. False if the download was cancelled meanwhile.
fn sleep_unless_cancelled(wait: Duration, cancel: &CancelToken) -> bool {
    let until = Instant::now() + wait;
    while !cancel.is_cancelled() {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return true;
        }
        std::thread::sleep(left.min(BACKOFF_SLICE));
    }
    false
}

/// Whether the file at `path` is `size` bytes with sha256 `expected`. Reads it in 1 MiB
/// chunks, stopping if the download is cancelled.
fn copy_checks_out(path: &Path, size: u64, expected: &str, cancel: &CancelToken) -> Result<bool, DownloadError> {
    let mut file = File::open(path).map_err(|e| disk(path, e))?;
    if file.metadata().map_err(|e| disk(path, e))?.len() != size {
        return Ok(false);
    }
    let mut hasher = Sha256::new();
    let mut buf = vec![0; CHUNK];
    loop {
        if cancel.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(buf.get(..n).unwrap_or_default()),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(disk(path, e)),
        }
    }
    Ok(hex(&hasher.finalize()) == expected)
}

/// Lowercase hex, formatted by hand: `sha2` 0.11 returns a hybrid-array, whose `{:x}` support
/// isn't something to count on.
fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn disk(path: &Path, e: io::Error) -> DownloadError {
    DownloadError::Disk(format!("{}: {e}", path.display()))
}

/// A failed write: out of space mid-download is [`DownloadError::NotEnoughSpace`]. `done` bytes
/// sit in the part file, which is about to be deleted, so they count as free.
fn write_error(path: &Path, e: io::Error, done: u64, needed: u64) -> DownloadError {
    match e.kind() {
        io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => {
            let dir = path.parent().unwrap_or(path);
            let available = available_space(dir).unwrap_or(0).saturating_add(done);
            DownloadError::NotEnoughSpace { needed, available }
        }
        _ => disk(path, e),
    }
}

/// The hidden part file of a running download: created and locked by [`PartFile::create`],
/// deleted on drop unless [`activate`](PartFile::activate) renamed it into place. The lock
/// (`flock`, through [`File::try_lock`]) tells other downloads and [`remove_partials`] that it's
/// in use, in this process or another; it's released when the file closes.
struct PartFile {
    path: PathBuf,
    file: File,
    keep: bool,
}

impl PartFile {
    /// Opens and locks `path`, then empties it. [`DownloadError::Busy`] if another download
    /// holds it.
    fn create(path: &Path) -> Result<Self, DownloadError> {
        // Truncating only once the lock is ours, so a running download's file is never touched.
        let file = lock_existing_or_new(path)?.ok_or(DownloadError::Busy)?;
        file.set_len(0).map_err(|e| disk(path, e))?;
        Ok(Self { path: path.to_path_buf(), file, keep: false })
    }

    /// Flushes the file to disk, makes it readable like the script's models (0644), and renames
    /// it to `target`: atomic, since both are in the same folder.
    fn activate(mut self, target: &Path) -> Result<(), DownloadError> {
        // `sync_all` is F_FULLFSYNC on macOS: the data is on disk before the name appears.
        self.file.sync_all().map_err(|e| disk(&self.path, e))?;
        fs::set_permissions(&self.path, fs::Permissions::from_mode(0o644)).map_err(|e| disk(&self.path, e))?;
        fs::rename(&self.path, target).map_err(|e| disk(target, e))?;
        self.keep = true;
        Ok(())
    }
}

impl Drop for PartFile {
    fn drop(&mut self) {
        // Deleted while still locked, so nobody else can take the lock on it first. The file
        // closes (and unlocks) after this.
        if !self.keep
            && let Err(e) = fs::remove_file(&self.path)
            && e.kind() != io::ErrorKind::NotFound
        {
            log::warn!("download: couldn't remove {}: {e}", self.path.display());
        }
    }
}

/// Opens `path` (creating it if needed) and takes its lock without waiting. `None` if someone
/// holds it. Tries again if the file was deleted between the open and the lock, so the lock is
/// always on the file at `path`, never on one that's gone.
fn lock_existing_or_new(path: &Path) -> Result<Option<File>, DownloadError> {
    for _ in 0..3 {
        let file = OpenOptions::new().create(true).truncate(false).write(true).open(path).map_err(|e| disk(path, e))?;
        if !try_lock(&file).map_err(|e| disk(path, e))? {
            return Ok(None);
        }
        if is_same_file(&file, path) {
            return Ok(Some(file));
        }
    }
    Err(DownloadError::Disk(format!("{}: keeps disappearing", path.display())))
}

/// Takes `file`'s exclusive lock without waiting (`flock(LOCK_EX | LOCK_NB)` on macOS): true if
/// it's now ours, false if someone else holds it.
fn try_lock(file: &File) -> io::Result<bool> {
    match file.try_lock() {
        Ok(()) => Ok(true),
        Err(fs::TryLockError::WouldBlock) => Ok(false),
        Err(fs::TryLockError::Error(e)) => Err(e),
    }
}

/// Whether `path` still names the open `file`.
fn is_same_file(file: &File, path: &Path) -> bool {
    match (file.metadata(), fs::metadata(path)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    }
}

/// Removes partial files left by downloads that were killed (a crash, a force quit). Skips any
/// that a running download holds, in this process or another. Call it once at startup. Only
/// this module's hidden `.<file>.part` files go, in the folders the catalog downloads into;
/// models and anything else stay.
pub fn remove_partials(models_dir: &Path) {
    let mut dirs: Vec<PathBuf> = crate::models::catalog()
        .iter()
        .filter(|m| m.sha256.is_some())
        .filter_map(|m| m.resolve(models_dir).parent().map(Path::to_path_buf))
        .collect();
    dirs.sort();
    dirs.dedup();
    for dir in dirs {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !(name.starts_with('.') && name.ends_with(".part")) || !entry.file_type().is_ok_and(|t| t.is_file()) {
                continue;
            }
            let path = entry.path();
            let Ok(file) = OpenOptions::new().read(true).open(&path) else { continue };
            if try_lock(&file).unwrap_or(false) && is_same_file(&file, &path) {
                match fs::remove_file(&path) {
                    Ok(()) => log::info!("download: removed the partial file {}", path.display()),
                    Err(e) => log::warn!("download: couldn't remove {}: {e}", path.display()),
                }
            }
        }
    }
}

/// Deletes a downloaded model's file and its license notice, and its partial file unless a
/// download holds it. A model that isn't there is fine. Unlinking a model that's loaded is
/// safe: it stays usable until it's freed.
pub fn delete(model: &ModelInfo, models_dir: &Path) -> io::Result<()> {
    let ok_if_missing = |r: io::Result<()>| match r {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        r => r,
    };
    ok_if_missing(fs::remove_file(model.resolve(models_dir)))?;
    ok_if_missing(fs::remove_file(model.license_notice_path(models_dir)))?;
    let part = model.partial_path(models_dir);
    if let Ok(file) = OpenOptions::new().read(true).open(&part)
        && try_lock(&file)?
        && is_same_file(&file, &part)
    {
        ok_if_missing(fs::remove_file(&part))?;
    }
    Ok(())
}

/// Sends progress to the caller: `done` only grows, `Downloading` at most every
/// [`PROGRESS_INTERVAL`], and the last one always goes out.
struct Reporter<'a> {
    progress: &'a mut dyn FnMut(Progress),
    last_sent: Option<Instant>,
    last: Option<Progress>,
}

impl<'a> Reporter<'a> {
    fn new(progress: &'a mut dyn FnMut(Progress)) -> Self {
        Self { progress, last_sent: None, last: None }
    }

    fn emit(&mut self, p: Progress) {
        self.last = Some(p);
        (self.progress)(p);
    }

    /// Throttled.
    fn downloading(&mut self, done: u64, total: u64) {
        let now = Instant::now();
        if self.last_sent.is_none_or(|t| now.duration_since(t) >= PROGRESS_INTERVAL) {
            self.last_sent = Some(now);
            self.emit(Progress::Downloading { done, total });
        }
    }

    /// The closing `Downloading { total, total }`, unless it was the last event sent.
    fn finish(&mut self, total: u64) {
        let p = Progress::Downloading { done: total, total };
        if self.last != Some(p) {
            self.emit(p);
        }
    }
}

/// What the transport hands the sink.
pub(crate) enum Fetched<'a> {
    /// The server answered 200 or 206: the body starts at byte `offset` of a file of `total`
    /// bytes (when it says).
    Start {
        offset: u64,
        total: Option<u64>,
    },
    Data(&'a [u8]),
}

/// How one attempt ended, when it didn't end cleanly.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) enum FetchError {
    Cancelled,
    /// An error status, other than 200 and 206.
    Http(u16),
    /// No route to the server (offline, DNS, refused). Retried.
    Offline(String),
    /// A dropped or stalled connection. Retried.
    Transient(String),
    /// Retrying won't help (TLS, a bad URL or redirect).
    Fatal(String),
    /// The sink refused the data.
    Sink(DownloadError),
}

/// Free space for a new file in `dir`.
fn available_space(dir: &Path) -> io::Result<u64> {
    #[cfg(target_os = "macos")]
    return transport::available_space(dir);
    #[cfg(not(target_os = "macos"))]
    return statvfs_available(dir);
}

/// Free space in `dir` for an unprivileged user, from `statvfs`.
fn statvfs_available(dir: &Path) -> io::Result<u64> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(dir.as_os_str().as_bytes()).map_err(io::Error::other)?;
    // SAFETY: an all-zero statvfs is a valid value to pass for the call to fill in.
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `path` is NUL-terminated and `stat` is writable.
    if unsafe { libc::statvfs(path.as_ptr(), &mut stat) } != 0 {
        return Err(io::Error::last_os_error());
    }
    #[allow(clippy::unnecessary_cast)] // the field types differ between platforms
    Ok(stat.f_bavail as u64 * stat.f_frsize as u64)
}

/// In-app downloads need NSURLSession; elsewhere, `scripts/fetch-model.sh` does the job.
#[cfg(not(target_os = "macos"))]
mod transport {
    use super::{CancelToken, DownloadError, FetchError, Fetched};
    use std::time::Duration;

    pub(super) fn fetch(
        _url: &str,
        _offset: u64,
        _idle_timeout: Duration,
        _cancel: &CancelToken,
        _sink: &mut dyn FnMut(Fetched) -> Result<(), DownloadError>,
    ) -> Result<(), FetchError> {
        Err(FetchError::Fatal("In-app downloads are macOS-only; use scripts/fetch-model.sh".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_check_keeps_a_margin() {
        let needed = 539_012_608 + SPACE_MARGIN;
        assert_eq!(check_space(needed, needed), Ok(()));
        assert_eq!(
            check_space(needed, needed - 1),
            Err(DownloadError::NotEnoughSpace { needed, available: needed - 1 })
        );
    }

    #[test]
    fn a_full_disk_mid_download_is_not_enough_space() {
        let path = std::env::temp_dir().join(".x.gguf.part");
        let full = io::Error::from(io::ErrorKind::StorageFull);
        match write_error(&path, full, 1000, 5000) {
            DownloadError::NotEnoughSpace { needed: 5000, available } => assert!(available >= 1000),
            other => panic!("{other:?}"),
        }
        let denied = io::Error::from(io::ErrorKind::PermissionDenied);
        assert!(matches!(write_error(&path, denied, 0, 1), DownloadError::Disk(_)));
    }

    #[test]
    fn hex_is_lowercase_and_padded() {
        assert_eq!(hex(&[0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
        assert_eq!(hex(&Sha256::digest(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn progress_is_throttled_and_the_last_event_always_goes_out() {
        let mut seen = vec![];
        let mut record = |p| seen.push(p);
        let mut reporter = Reporter::new(&mut record);
        for done in 1..=1000 {
            reporter.downloading(done, 1000); // far quicker than PROGRESS_INTERVAL
        }
        reporter.finish(1000);
        reporter.finish(1000); // not sent twice
        assert_eq!(
            seen,
            [Progress::Downloading { done: 1, total: 1000 }, Progress::Downloading { done: 1000, total: 1000 }]
        );
    }

    #[test]
    fn errors_read_as_sentences() {
        let offline = DownloadError::Offline("The Internet connection appears to be offline.".into());
        assert_eq!(offline.to_string(), "Couldn't reach the server. The Internet connection appears to be offline.");
        let space = DownloadError::NotEnoughSpace { needed: 603_000_000, available: 84_000_000 };
        assert_eq!(space.to_string(), "Not enough disk space: the download needs 603 MB, and 84 MB is free.");
        assert_eq!(DownloadError::Network("timed out".into()).to_string(), "The download failed. Timed out.");
        assert_eq!(DownloadError::Http(404).to_string(), "The server doesn't have this model (HTTP 404).");
    }

    #[test]
    fn sizes_are_decimal_like_finder() {
        assert_eq!(format_size(84_886_208), "85 MB");
        assert_eq!(format_size(539_012_608), "539 MB");
        assert_eq!(format_size(1_250_000_000), "1.2 GB");
    }

    #[test]
    fn cancel_token_is_shared_between_clones() {
        let token = CancelToken::new();
        let clone = token.clone();
        assert!(!token.is_cancelled());
        clone.cancel();
        assert!(token.is_cancelled());
        assert!(!sleep_unless_cancelled(Duration::from_secs(10), &token), "returns at once");
    }
}
