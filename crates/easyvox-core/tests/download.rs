//! `download()` against a small HTTP server on 127.0.0.1, with every fault injected locally:
//! cut connections, stalls, redirects, a server that ignores `Range`, wrong bytes, 404. No
//! internet: this runs in the default `cargo test`, CI included. The live checks against
//! Hugging Face are in `download_live.rs`, and ignored.
//!
//! After each test, the shared checks: progress never goes back, a success ends at the
//! catalog's `download_bytes` with the model and its license notice in place, and a failure
//! leaves the model's folder empty.

#![cfg(target_os = "macos")]

use easyvox_core::download::{
    self, CancelToken, DownloadError, DownloadOptions, Progress, download_with, remove_partials,
};
use easyvox_core::models::{self, ModelInfo};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MIB: usize = 1024 * 1024;
const PAYLOAD_LEN: usize = 4 * MIB;

/// Short waits, so the retry tests take about a second.
fn options() -> DownloadOptions {
    DownloadOptions { idle_timeout: Duration::from_secs(1), max_failures: 3, first_backoff: Duration::from_millis(10) }
}

// ---------------------------------------------------------------------------------------------
// The server

/// How the server answers. Every field applies to every request unless it says otherwise.
#[derive(Clone, Default)]
struct Behaviour {
    /// `/resolve/…` answers 302 to `/cdn/…`, like huggingface.co to its CDN.
    redirect: bool,
    /// Answer this status, with no body, instead of the file.
    status: Option<u16>,
    /// Close the connection on reaching this byte of the file, for the first `n` file
    /// requests (`usize::MAX`: all of them). A cut waits until the client has reported bytes
    /// ([`Server::saw`]): see [`serve`].
    cut_at: Option<(usize, usize)>,
    /// Send the first `bytes` of the response, then hold the socket open for `hold`, for the
    /// first file request only.
    stall: Option<(usize, Duration)>,
    /// Send `chunk` bytes every `every`.
    trickle: Option<(usize, Duration)>,
    /// Always answer 200 with the whole file, whatever `Range` says.
    ignore_range: bool,
    /// Serve the file as if it were only this long, with no `Content-Length`: the response ends
    /// when the connection closes, early but cleanly.
    truncate_to: Option<usize>,
}

/// One request the server saw.
#[derive(Clone, Debug, PartialEq)]
struct Request {
    path: String,
    /// The start of its `Range: bytes=N-` header.
    range_start: Option<usize>,
}

struct Server {
    port: u16,
    log: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
    /// Set by [`Server::saw`] once the client has reported bytes.
    client_has_bytes: Arc<AtomicBool>,
}

impl Server {
    fn start(payload: Vec<u8>, behaviour: Behaviour) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let log: Arc<Mutex<Vec<Request>>> = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        let client_has_bytes = Arc::new(AtomicBool::new(false));
        let (payload, log2, stop2, has_bytes2) =
            (Arc::new(payload), log.clone(), stop.clone(), client_has_bytes.clone());
        let file_requests = Arc::new(Mutex::new(0usize));
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if stop2.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(stream) = stream else { continue };
                let (payload, log, behaviour, file_requests, has_bytes) =
                    (payload.clone(), log2.clone(), behaviour.clone(), file_requests.clone(), has_bytes2.clone());
                std::thread::spawn(move || serve(stream, &payload, &behaviour, &log, &file_requests, &has_bytes));
            }
        });
        Server { port, log, stop, client_has_bytes }
    }

    /// Hands the server the client's progress, for a test that cuts the connection: the cut
    /// waits for the client's first bytes.
    fn saw(&self, p: Progress) {
        if matches!(p, Progress::Downloading { done, .. } if done > 0) {
            self.client_has_bytes.store(true, Ordering::Relaxed);
        }
    }

    /// The pinned-style URL a catalog entry would have.
    fn url(&self) -> String {
        format!("http://127.0.0.1:{}/resolve/0123abcd/test.gguf", self.port)
    }

    fn requests(&self) -> Vec<Request> {
        self.log.lock().unwrap().clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(("127.0.0.1", self.port)); // wakes the accept loop
    }
}

fn serve(
    mut stream: TcpStream,
    payload: &[u8],
    b: &Behaviour,
    log: &Mutex<Vec<Request>>,
    file_requests: &Mutex<usize>,
    client_has_bytes: &AtomicBool,
) {
    let Ok(read_half) = stream.try_clone() else { return };
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return; // the shutdown wake-up
    }
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
    let mut range_start = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("range")
        {
            range_start = value.trim().strip_prefix("bytes=").and_then(|v| v.trim_end_matches('-').parse().ok());
        }
    }
    log.lock().unwrap().push(Request { path: path.clone(), range_start });

    if let Some(status) = b.status {
        let _ = write!(stream, "HTTP/1.1 {status} Nope\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        return;
    }
    if b.redirect && path.starts_with("/resolve/") {
        let target = path.replacen("/resolve/", "/cdn/", 1);
        let _ = write!(
            stream,
            "HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        return;
    }
    let nth = {
        let mut n = file_requests.lock().unwrap();
        *n += 1;
        *n
    };

    if let Some(len) = b.truncate_to {
        let _ = write!(stream, "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n");
        let _ = stream.write_all(&payload[..len]);
        return;
    }
    let start = if b.ignore_range { 0 } else { range_start.unwrap_or(0).min(payload.len()) };
    let body = &payload[start..];
    let head = if start == 0 && (b.ignore_range || range_start.is_none()) {
        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len())
    } else {
        format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{}/{}\r\nConnection: close\r\n\r\n",
            body.len(),
            payload.len() - 1,
            payload.len()
        )
    };
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    // Where this response stops early, if it does.
    let mut end = body.len();
    if let Some((cut, first_n)) = b.cut_at
        && nth <= first_n
    {
        end = end.min(cut.saturating_sub(start));
    }
    if let Some((bytes, hold)) = b.stall
        && nth == 1
    {
        let _ = stream.write_all(&body[..bytes.min(end)]);
        std::thread::sleep(hold);
        return;
    }
    match b.trickle {
        Some((chunk, every)) => {
            for piece in body[..end].chunks(chunk) {
                if stream.write_all(piece).is_err() {
                    return;
                }
                std::thread::sleep(every);
            }
        }
        None => {
            let _ = stream.write_all(&body[..end]);
        }
    }
    // A cut: hold the connection until the client has reported bytes, so a resume always has
    // something to resume from. When a response ends early, NSURLSession reports the lost
    // connection and drops whatever it had received but not yet handed to its delegate (the
    // kernel delivers every byte: it's the client that discards them). How much that is depends
    // on how busy the machine is, so without this wait the client could come back with nothing,
    // and tests compare against what the client reported, never against the cut point.
    if end < body.len() {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !client_has_bytes.load(Ordering::Relaxed) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    // Dropping the stream closes it: early if `end` was cut short.
}

// ---------------------------------------------------------------------------------------------
// Helpers

/// `len` reproducible pseudo-random bytes (xorshift), different for each seed.
fn payload(seed: u64, len: usize) -> Vec<u8> {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// A catalog entry for `bytes` at `url`: the recommended model's, with its file fields
/// replaced. `download()` takes a `ModelInfo`, not a catalog id.
fn model(url: &str, bytes: &[u8]) -> ModelInfo {
    ModelInfo {
        id: "test",
        path: "gguf/test.gguf",
        download_url: String::leak(url.to_string()),
        sha256: Some(String::leak(sha256_hex(bytes))),
        download_bytes: bytes.len() as u64,
        disk_bytes: bytes.len() as u64,
        ..models::find(easyvox_core::RECOMMENDED_MODEL).unwrap().clone()
    }
}

/// An empty temporary models folder for one test.
fn temp_models_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("easyvox-download-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The names in `dir/gguf`, hidden ones included.
fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir.join("gguf"))
        .map(|entries| entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    names.sort();
    names
}

/// Runs `download_with` and records its progress events.
fn run(
    m: &ModelInfo,
    dir: &Path,
    cancel: &CancelToken,
    mut on_progress: impl FnMut(Progress),
) -> (Result<PathBuf, DownloadError>, Vec<Progress>) {
    let mut events = vec![];
    let result = download_with(m, dir, &options(), cancel, |p| {
        events.push(p);
        on_progress(p);
    });
    (result, events)
}

/// The checks every test makes: `done` never decreases; a success ends at `download_bytes`
/// with the file and its license notice in place and nothing else; a failure leaves the folder
/// empty.
fn check(m: &ModelInfo, dir: &Path, result: &Result<PathBuf, DownloadError>, events: &[Progress]) {
    let done: Vec<u64> = events
        .iter()
        .filter_map(|p| match *p {
            Progress::Downloading { done, .. } | Progress::Reconnecting { done, .. } => Some(done),
            Progress::Verifying => None,
        })
        .collect();
    assert!(done.windows(2).all(|w| w[0] <= w[1]), "progress went back: {done:?}");
    for p in events {
        if let Progress::Downloading { total, .. } | Progress::Reconnecting { total, .. } = p {
            assert_eq!(*total, m.download_bytes);
        }
    }
    match result {
        Ok(path) => {
            assert_eq!(events.last(), Some(&Progress::Downloading { done: m.download_bytes, total: m.download_bytes }));
            assert_eq!(*path, m.resolve(dir));
            assert_eq!(listing(dir), ["test.gguf", "test.gguf.LICENSE.txt"], "only the model and its notice");
            let notice = std::fs::read_to_string(m.license_notice_path(dir)).unwrap();
            assert_eq!(notice, m.license_notice());
        }
        Err(_) => assert!(listing(dir).is_empty(), "a failure left files behind: {:?}", listing(dir)),
    }
}

/// `done` at each `Reconnecting` event: where each retry picked up.
fn retries_from(events: &[Progress]) -> Vec<usize> {
    events
        .iter()
        .filter_map(|p| match *p {
            Progress::Reconnecting { done, .. } => Some(done as usize),
            _ => None,
        })
        .collect()
}

/// Where the first retry picked up.
fn resumed_at(events: &[Progress]) -> usize {
    *retries_from(events).first().expect("a Reconnecting event")
}

fn assert_installed(m: &ModelInfo, dir: &Path, bytes: &[u8]) {
    let path = m.resolve(dir);
    assert!(std::fs::read(&path).unwrap() == bytes, "the installed file isn't the payload");
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o644);
    assert!(m.is_available(dir));
}

// ---------------------------------------------------------------------------------------------
// The tests (★ = acceptance cases for model downloads)

/// ★ The file lands at its path with the right bytes and mode, after one request.
#[test]
fn success() {
    let bytes = payload(1, PAYLOAD_LEN);
    let server = Server::start(bytes.clone(), Behaviour::default());
    let (m, dir) = (model(&server.url(), &bytes), temp_models_dir("success"));
    let (result, events) = run(&m, &dir, &CancelToken::new(), |_| {});
    check(&m, &dir, &result, &events);
    result.unwrap();
    assert_installed(&m, &dir, &bytes);
    assert_eq!(server.requests().len(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// ★ Right size, wrong bytes: a clear error with both hashes, and nothing kept.
#[test]
fn wrong_sha256() {
    let expected = payload(1, PAYLOAD_LEN);
    let served = payload(2, PAYLOAD_LEN);
    let server = Server::start(served.clone(), Behaviour::default());
    let (m, dir) = (model(&server.url(), &expected), temp_models_dir("wrong-sha256"));
    let (result, events) = run(&m, &dir, &CancelToken::new(), |_| {});
    check(&m, &dir, &result, &events);
    let err = result.unwrap_err();
    assert_eq!(err, DownloadError::ChecksumMismatch { expected: sha256_hex(&expected), actual: sha256_hex(&served) });
    assert!(err.to_string().contains("sha256"), "{err}");
    assert_eq!(server.requests().len(), 1, "no automatic retry after a checksum mismatch");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// ★ The first connection drops at 1 MiB; the second asks for the rest and the file completes.
#[test]
fn interrupted_then_resumed() {
    let bytes = payload(3, PAYLOAD_LEN);
    let server = Server::start(bytes.clone(), Behaviour { cut_at: Some((MIB, 1)), ..Default::default() });
    let (m, dir) = (model(&server.url(), &bytes), temp_models_dir("resumed"));
    let (result, events) = run(&m, &dir, &CancelToken::new(), |p| server.saw(p));
    check(&m, &dir, &result, &events);
    result.unwrap();
    assert_installed(&m, &dir, &bytes);
    // NSURLSession drops what it hadn't handed over yet when the connection breaks (see
    // `serve`), so the resume starts at or before the cut: wherever `done` was.
    let resumed_at = resumed_at(&events);
    assert!(resumed_at > 0 && resumed_at <= MIB, "{resumed_at}");
    let starts: Vec<_> = server.requests().iter().map(|r| r.range_start).collect();
    assert_eq!(starts, [None, Some(resumed_at)]);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// ★ Every connection drops at 1 MiB: after `max_failures` tries without new bytes it gives up,
/// with nothing at the model's path and no part file.
#[test]
fn interrupted_every_time() {
    let bytes = payload(4, PAYLOAD_LEN);
    let server = Server::start(bytes.clone(), Behaviour { cut_at: Some((MIB, usize::MAX)), ..Default::default() });
    let (m, dir) = (model(&server.url(), &bytes), temp_models_dir("interrupted"));
    let (result, events) = run(&m, &dir, &CancelToken::new(), |p| server.saw(p));
    check(&m, &dir, &result, &events);
    assert!(matches!(result, Err(DownloadError::Network(_))), "{result:?}");
    // Tries that got bytes, then max_failures (3) in a row that got nothing, all from where the
    // client got stuck: the cut, or before it if NSURLSession dropped the end (see `serve`).
    let stuck_at = *retries_from(&events).last().expect("a Reconnecting event");
    assert!(stuck_at > 0 && stuck_at <= MIB, "{events:?}");
    let requests = server.requests();
    let stuck: Vec<_> = requests.iter().rev().take(3).map(|r| r.range_start).collect();
    assert_eq!(stuck, [Some(stuck_at); 3], "{requests:?}");
    assert!(requests.len() >= 4, "{requests:?}");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// ★ A truncated file (the server ends the response early, cleanly, with no length): retried,
/// then refused, and nothing kept.
#[test]
fn truncated_file() {
    let bytes = payload(5, PAYLOAD_LEN);
    let server = Server::start(bytes.clone(), Behaviour { truncate_to: Some(PAYLOAD_LEN / 2), ..Default::default() });
    let (m, dir) = (model(&server.url(), &bytes), temp_models_dir("truncated"));
    let (result, events) = run(&m, &dir, &CancelToken::new(), |_| {});
    check(&m, &dir, &result, &events);
    assert!(matches!(result, Err(DownloadError::Network(_))), "{result:?}");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// ★ Cancelled from the progress callback while the server trickles: `Cancelled` within a
/// second, and nothing left.
#[test]
fn cancel() {
    let bytes = payload(6, PAYLOAD_LEN);
    let behaviour = Behaviour { trickle: Some((64 * 1024, Duration::from_millis(50))), ..Default::default() };
    let server = Server::start(bytes.clone(), behaviour);
    let (m, dir) = (model(&server.url(), &bytes), temp_models_dir("cancel"));
    let cancel = CancelToken::new();
    let mut cancelled_at = None;
    let (result, events) = run(&m, &dir, &cancel, |p| {
        if matches!(p, Progress::Downloading { .. }) && cancelled_at.is_none() {
            cancel.cancel();
            cancelled_at = Some(Instant::now());
        }
    });
    let took = cancelled_at.expect("a progress event").elapsed();
    check(&m, &dir, &result, &events);
    assert_eq!(result, Err(DownloadError::Cancelled));
    assert!(took < Duration::from_secs(1), "cancel took {took:?}");
    drop(server);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// ★ A copy already there that checks out: no request at all.
#[test]
fn already_present() {
    let bytes = payload(7, PAYLOAD_LEN);
    let server = Server::start(bytes.clone(), Behaviour::default());
    let (m, dir) = (model(&server.url(), &bytes), temp_models_dir("present"));
    std::fs::create_dir_all(dir.join("gguf")).unwrap();
    std::fs::write(m.resolve(&dir), &bytes).unwrap();
    let (result, events) = run(&m, &dir, &CancelToken::new(), |_| {});
    check(&m, &dir, &result, &events);
    result.unwrap();
    assert!(server.requests().is_empty(), "{:?}", server.requests());
    let total = m.download_bytes;
    assert_eq!(events, [Progress::Verifying, Progress::Downloading { done: total, total }]);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A copy already there but damaged (right size, wrong bytes) is replaced.
#[test]
fn already_present_but_damaged() {
    let bytes = payload(8, PAYLOAD_LEN);
    let server = Server::start(bytes.clone(), Behaviour::default());
    let (m, dir) = (model(&server.url(), &bytes), temp_models_dir("damaged"));
    std::fs::create_dir_all(dir.join("gguf")).unwrap();
    std::fs::write(m.resolve(&dir), payload(9, PAYLOAD_LEN)).unwrap();
    let (result, events) = run(&m, &dir, &CancelToken::new(), |_| {});
    check(&m, &dir, &result, &events);
    result.unwrap();
    assert_installed(&m, &dir, &bytes);
    assert_eq!(events.first(), Some(&Progress::Verifying));
    assert_eq!(server.requests().len(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// ★ 404 (a wrong catalog URL): no retries, nothing left.
#[test]
fn not_found() {
    let bytes = payload(10, PAYLOAD_LEN);
    let server = Server::start(bytes.clone(), Behaviour { status: Some(404), ..Default::default() });
    let (m, dir) = (model(&server.url(), &bytes), temp_models_dir("404"));
    let (result, events) = run(&m, &dir, &CancelToken::new(), |_| {});
    check(&m, &dir, &result, &events);
    assert_eq!(result, Err(DownloadError::Http(404)));
    assert_eq!(server.requests().len(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A 5xx is retried, then reported as the HTTP error.
#[test]
fn server_error_is_retried() {
    let bytes = payload(11, PAYLOAD_LEN);
    let server = Server::start(bytes.clone(), Behaviour { status: Some(503), ..Default::default() });
    let (m, dir) = (model(&server.url(), &bytes), temp_models_dir("503"));
    let (result, events) = run(&m, &dir, &CancelToken::new(), |_| {});
    check(&m, &dir, &result, &events);
    assert_eq!(result, Err(DownloadError::Http(503)));
    assert_eq!(server.requests().len(), 3);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The Hugging Face case: a 302 to the CDN, cut once. The resumed request reaches the
/// redirect target with its `Range` header.
#[test]
fn redirect_then_resume() {
    let bytes = payload(12, PAYLOAD_LEN);
    let behaviour = Behaviour { redirect: true, cut_at: Some((MIB, 1)), ..Default::default() };
    let server = Server::start(bytes.clone(), behaviour);
    let (m, dir) = (model(&server.url(), &bytes), temp_models_dir("redirect"));
    let (result, events) = run(&m, &dir, &CancelToken::new(), |p| server.saw(p));
    check(&m, &dir, &result, &events);
    result.unwrap();
    assert_installed(&m, &dir, &bytes);
    let requests = server.requests();
    let paths: Vec<_> = requests.iter().map(|r| r.path.split('/').nth(1).unwrap_or("")).collect();
    assert_eq!(paths, ["resolve", "cdn", "resolve", "cdn"], "every try starts from the pinned URL");
    let resumed_at = resumed_at(&events);
    assert!(resumed_at > 0, "{events:?}");
    assert_eq!(requests[2].range_start, Some(resumed_at));
    assert_eq!(requests[3].range_start, Some(resumed_at), "the redirect target gets the Range header");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A server that ignores `Range` (always 200, the whole file): the resume still works, by
/// skipping what's already here.
#[test]
fn range_ignored() {
    let bytes = payload(13, PAYLOAD_LEN);
    let behaviour = Behaviour { ignore_range: true, cut_at: Some((MIB, 1)), ..Default::default() };
    let server = Server::start(bytes.clone(), behaviour);
    let (m, dir) = (model(&server.url(), &bytes), temp_models_dir("range-ignored"));
    let (result, events) = run(&m, &dir, &CancelToken::new(), |p| server.saw(p));
    check(&m, &dir, &result, &events);
    result.unwrap();
    assert_installed(&m, &dir, &bytes);
    let resumed_at = resumed_at(&events);
    assert!(resumed_at > 0, "{events:?}");
    assert_eq!(server.requests()[1].range_start, Some(resumed_at), "it asked for the rest");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A connection that goes quiet for longer than the idle timeout (the Mac slept, the Wi-Fi
/// dropped) is given up and resumed.
#[test]
fn stall() {
    let bytes = payload(14, PAYLOAD_LEN);
    let behaviour = Behaviour { stall: Some((MIB, Duration::from_secs(4))), ..Default::default() };
    let server = Server::start(bytes.clone(), behaviour);
    let (m, dir) = (model(&server.url(), &bytes), temp_models_dir("stall"));
    let started = Instant::now();
    let (result, events) = run(&m, &dir, &CancelToken::new(), |_| {});
    check(&m, &dir, &result, &events);
    result.unwrap();
    assert_installed(&m, &dir, &bytes);
    assert!(events.iter().any(|p| matches!(p, Progress::Reconnecting { .. })), "{events:?}");
    assert!(started.elapsed() < Duration::from_secs(3), "took {:?}", started.elapsed());
    assert_eq!(server.requests()[1].range_start, Some(resumed_at(&events)));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A server announcing the wrong size is refused before the body is read.
#[test]
fn wrong_size_announced() {
    let bytes = payload(15, PAYLOAD_LEN);
    let server = Server::start(bytes.clone(), Behaviour::default());
    let mut m = model(&server.url(), &bytes);
    m.download_bytes += 1;
    let dir = temp_models_dir("wrong-size");
    let (result, events) = run(&m, &dir, &CancelToken::new(), |_| {});
    check(&m, &dir, &result, &events);
    assert_eq!(
        result,
        Err(DownloadError::SizeMismatch { expected: PAYLOAD_LEN as u64 + 1, actual: PAYLOAD_LEN as u64 })
    );
    assert!(events.is_empty(), "no bytes were taken: {events:?}");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A second download of a model that's already downloading gets `Busy`, and leaves the first
/// one alone.
#[test]
fn busy() {
    let bytes = payload(16, PAYLOAD_LEN);
    let behaviour = Behaviour { trickle: Some((256 * 1024, Duration::from_millis(50))), ..Default::default() };
    let server = Server::start(bytes.clone(), behaviour);
    let (m, dir) = (model(&server.url(), &bytes), temp_models_dir("busy"));
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let first = {
        let (m, dir) = (m.clone(), dir.clone());
        std::thread::spawn(move || {
            run(&m, &dir, &CancelToken::new(), |p| {
                if matches!(p, Progress::Downloading { .. }) {
                    let _ = started_tx.send(());
                }
            })
        })
    };
    started_rx.recv_timeout(Duration::from_secs(5)).expect("the first download started");
    let second = download_with(&m, &dir, &options(), &CancelToken::new(), |_| {});
    assert_eq!(second, Err(DownloadError::Busy));
    let (result, events) = first.join().unwrap();
    check(&m, &dir, &result, &events);
    result.unwrap();
    assert_installed(&m, &dir, &bytes);
    assert_eq!(server.requests().len(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// ★ A part file left by a crash goes at the next launch, unless a running download holds it.
/// The script's temp files and anything else stay.
#[test]
fn remove_partials_sweeps_only_stale_part_files() {
    let dir = temp_models_dir("sweep");
    let gguf = dir.join("gguf");
    std::fs::create_dir_all(&gguf).unwrap();
    for name in [".test.gguf.part", ".held.gguf.part", ".x.gguf.part.AB12CD", "x.gguf", "notes.part"] {
        std::fs::write(gguf.join(name), b"half a model").unwrap();
    }
    let held = std::fs::File::open(gguf.join(".held.gguf.part")).unwrap();
    held.lock().unwrap();
    remove_partials(&dir);
    assert_eq!(listing(&dir), [".held.gguf.part", ".x.gguf.part.AB12CD", "notes.part", "x.gguf"]);
    drop(held);
    remove_partials(&dir);
    assert_eq!(listing(&dir), [".x.gguf.part.AB12CD", "notes.part", "x.gguf"]);
    remove_partials(&dir.join("missing")); // no folder: nothing to do
    std::fs::remove_dir_all(&dir).unwrap();
}

/// ★ A corrupt part file left behind (before the sweep ran) is never used: `download()` starts
/// the file over.
#[test]
fn leftover_part_file_is_not_used() {
    let bytes = payload(17, PAYLOAD_LEN);
    let server = Server::start(bytes.clone(), Behaviour::default());
    let (m, dir) = (model(&server.url(), &bytes), temp_models_dir("leftover"));
    std::fs::create_dir_all(dir.join("gguf")).unwrap();
    std::fs::write(m.partial_path(&dir), payload(18, 3 * MIB)).unwrap();
    let (result, events) = run(&m, &dir, &CancelToken::new(), |_| {});
    check(&m, &dir, &result, &events);
    result.unwrap();
    assert_installed(&m, &dir, &bytes);
    assert_eq!(server.requests()[0].range_start, None, "starts from byte 0");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// An entry without a checksum (the bench-only sherpa archives) isn't downloaded.
#[test]
fn no_checksum_no_download() {
    let bytes = payload(19, 1024);
    let server = Server::start(bytes.clone(), Behaviour::default());
    let mut m = model(&server.url(), &bytes);
    m.sha256 = None;
    let dir = temp_models_dir("no-checksum");
    let (result, events) = run(&m, &dir, &CancelToken::new(), |_| {});
    assert_eq!(result, Err(DownloadError::NotDownloadable));
    assert!(events.is_empty() && server.requests().is_empty());
    assert!(listing(&dir).is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Nothing listens on the port: `Offline` after the retries, nothing left.
#[test]
fn connection_refused_is_offline() {
    let bytes = payload(20, 1024);
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port(); // closed again
    let m = model(&format!("http://127.0.0.1:{port}/resolve/0123abcd/test.gguf"), &bytes);
    let dir = temp_models_dir("refused");
    let (result, events) = run(&m, &dir, &CancelToken::new(), |_| {});
    check(&m, &dir, &result, &events);
    assert!(matches!(result, Err(DownloadError::Offline(_))), "{result:?}");
    assert_eq!(events.iter().filter(|p| matches!(p, Progress::Reconnecting { .. })).count(), 2);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// `delete` removes the model, its license notice and its stale part file, and is fine with
/// none of them there.
#[test]
fn delete_removes_the_model() {
    let bytes = payload(21, 1024);
    let m = model("http://127.0.0.1:9/unused", &bytes);
    let dir = temp_models_dir("delete");
    std::fs::create_dir_all(dir.join("gguf")).unwrap();
    std::fs::write(m.resolve(&dir), &bytes).unwrap();
    std::fs::write(m.license_notice_path(&dir), m.license_notice()).unwrap();
    std::fs::write(m.partial_path(&dir), b"half").unwrap();
    std::fs::write(dir.join("gguf/other.gguf"), b"not ours").unwrap();
    download::delete(&m, &dir).unwrap();
    assert_eq!(listing(&dir), ["other.gguf"]);
    download::delete(&m, &dir).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}
