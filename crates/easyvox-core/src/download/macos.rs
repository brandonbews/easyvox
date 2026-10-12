//! The macOS transport for [`super::download`]: one NSURLSession data task per attempt.
//!
//! NSURLSession is Apple's own networking stack, so downloads honor the proxies, PAC files and
//! certificates set in System Settings, survive network changes, and show up as EasyVox in
//! firewall apps. The session is ephemeral: no cookies, cache or credentials on disk.
//!
//! The delegate runs on a serial queue the session creates. It only copies what it's given into
//! a bounded channel, so the progress callback and all file I/O happen on the caller's thread.
//! A full channel blocks the delegate queue, and TCP slows the server down.
//!
//! **A delegate method must never panic**: objc2 methods are `extern "C-unwind"`, and a panic
//! would unwind into CFNetwork. So no `unwrap`, `expect` or slice indexing in them, only channel
//! sends (whose errors are ignored) and plain Objective-C calls.

use super::{CancelToken, FetchError, Fetched};
use block2::DynBlock;
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_foundation::{
    NSError, NSHTTPURLResponse, NSMutableCopying, NSMutableURLRequest, NSNumber, NSString, NSURL, NSURLRequest,
    NSURLResponse, NSURLSession, NSURLSessionConfiguration, NSURLSessionDataDelegate, NSURLSessionDataTask,
    NSURLSessionDelegate, NSURLSessionResponseDisposition, NSURLSessionTask, NSURLSessionTaskDelegate,
    NSURLVolumeAvailableCapacityForImportantUsageKey, ns_string,
};
use std::path::Path;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel};
use std::time::Duration;

/// How often the waiting thread looks at the cancel token.
const CANCEL_POLL: Duration = Duration::from_millis(100);

/// What the delegate passes to the thread running [`fetch`].
enum Msg {
    Response {
        status: isize,
        content_range: Option<String>,
        content_length: i64,
    },
    Data(Vec<u8>),
    /// The task ended: cleanly (`None`), or with an error.
    Done(Option<TaskError>),
}

struct TaskError {
    domain: String,
    code: isize,
    description: String,
}

struct Ivars {
    tx: SyncSender<Msg>,
    /// The `Range` header, put back on every redirect: CFNetwork isn't guaranteed to copy it.
    range: Option<String>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and `DownloadDelegate` doesn't
    // implement `Drop`.
    #[unsafe(super(NSObject))]
    #[name = "EasyVoxDownloadDelegate"]
    #[ivars = Ivars]
    struct DownloadDelegate;

    unsafe impl NSObjectProtocol for DownloadDelegate {}

    unsafe impl NSURLSessionDelegate for DownloadDelegate {}

    unsafe impl NSURLSessionTaskDelegate for DownloadDelegate {
        /// Follows the redirect with the same `Range` header, but never from https to anything
        /// else: refusing it delivers the redirect itself, which `fetch` reports as an HTTP error.
        #[unsafe(method(URLSession:task:willPerformHTTPRedirection:newRequest:completionHandler:))]
        fn will_redirect(
            &self,
            _session: &NSURLSession,
            _task: &NSURLSessionTask,
            response: &NSHTTPURLResponse,
            request: &NSURLRequest,
            completion: &DynBlock<dyn Fn(*mut NSURLRequest)>,
        ) {
            let scheme = |url: Option<Retained<NSURL>>| url.and_then(|u| u.scheme()).map(|s| s.to_string());
            let from = scheme(response.URL());
            let to = scheme(request.URL());
            if from.as_deref() == Some("https") && to.as_deref() != Some("https") {
                completion.call((std::ptr::null_mut(),));
                return;
            }
            let next: Retained<NSMutableURLRequest> = request.mutableCopy();
            if let Some(range) = &self.ivars().range {
                next.setValue_forHTTPHeaderField(Some(&NSString::from_str(range)), ns_string!("Range"));
            }
            let next: &NSURLRequest = &next;
            completion.call((next as *const NSURLRequest as *mut NSURLRequest,));
        }

        #[unsafe(method(URLSession:task:didCompleteWithError:))]
        fn did_complete(&self, _session: &NSURLSession, _task: &NSURLSessionTask, error: Option<&NSError>) {
            let error = error.map(|e| TaskError {
                domain: e.domain().to_string(),
                code: e.code(),
                description: e.localizedDescription().to_string(),
            });
            let _ = self.ivars().tx.send(Msg::Done(error));
        }
    }

    unsafe impl NSURLSessionDataDelegate for DownloadDelegate {
        /// Lets 200 and 206 through; anything else is cancelled here and reported by `fetch`.
        #[unsafe(method(URLSession:dataTask:didReceiveResponse:completionHandler:))]
        fn did_receive_response(
            &self,
            _session: &NSURLSession,
            _task: &NSURLSessionDataTask,
            response: &NSURLResponse,
            completion: &DynBlock<dyn Fn(NSURLSessionResponseDisposition)>,
        ) {
            let msg = match response.downcast_ref::<NSHTTPURLResponse>() {
                Some(http) => Msg::Response {
                    status: http.statusCode(),
                    content_range: http.valueForHTTPHeaderField(ns_string!("Content-Range")).map(|s| s.to_string()),
                    content_length: http.expectedContentLength(),
                },
                // Not HTTP (a file: URL, say): report it as an unusable status.
                None => Msg::Response { status: 0, content_range: None, content_length: -1 },
            };
            let allow = matches!(msg, Msg::Response { status: 200 | 206, .. });
            let _ = self.ivars().tx.send(msg);
            let disposition =
                if allow { NSURLSessionResponseDisposition::Allow } else { NSURLSessionResponseDisposition::Cancel };
            completion.call((disposition,));
        }

        #[unsafe(method(URLSession:dataTask:didReceiveData:))]
        fn did_receive_data(
            &self,
            _session: &NSURLSession,
            _task: &NSURLSessionDataTask,
            data: &objc2_foundation::NSData,
        ) {
            let _ = self.ivars().tx.send(Msg::Data(data.to_vec()));
        }
    }
);

impl DownloadDelegate {
    fn new(tx: SyncSender<Msg>, range: Option<String>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(Ivars { tx, range });
        // SAFETY: NSObject's `init`, on an allocated object whose ivars are set.
        unsafe { msg_send![super(this), init] }
    }
}

/// Invalidates the session when `fetch` returns, however it returns. A session retains its
/// delegate until it's invalidated, so without this every attempt would leak one.
struct SessionGuard(Retained<NSURLSession>);

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.0.invalidateAndCancel();
    }
}

/// One GET of `url` from byte `offset` to the end, following redirects. Feeds the response to
/// `sink` as it arrives: `Start` once the status is 200 or 206, then `Data` chunks. Returns
/// `Ok` when the server ended the response cleanly (which may be before the end of the file).
pub(super) fn fetch(
    url: &str,
    offset: u64,
    idle_timeout: Duration,
    cancel: &CancelToken,
    sink: &mut dyn FnMut(Fetched) -> Result<(), super::DownloadError>,
) -> Result<(), FetchError> {
    let Some(ns_url) = NSURL::URLWithString(&NSString::from_str(url)) else {
        return Err(FetchError::Fatal(format!("not a valid URL: {url}")));
    };
    let range = (offset > 0).then(|| format!("bytes={offset}-"));
    let (tx, rx) = sync_channel(32);
    let delegate = DownloadDelegate::new(tx, range.clone());

    let config = NSURLSessionConfiguration::ephemeralSessionConfiguration();
    // The idle timeout: it restarts with every byte that arrives.
    config.setTimeoutIntervalForRequest(idle_timeout.as_secs_f64());
    // Fail at once when offline: `download` does the waiting and retrying.
    config.setWaitsForConnectivity(false);
    config.setURLCache(None);
    // SAFETY: a `None` queue makes the session create its own serial delegate queue, and the
    // delegate is safe to call from any thread (it only sends on a channel).
    let session = unsafe {
        NSURLSession::sessionWithConfiguration_delegate_delegateQueue(
            &config,
            Some(ProtocolObject::from_ref(&*delegate)),
            None,
        )
    };
    let session = SessionGuard(session);

    let request = NSMutableURLRequest::requestWithURL(&ns_url);
    let agent = format!("EasyVox/{}", env!("CARGO_PKG_VERSION"));
    request.setValue_forHTTPHeaderField(Some(&NSString::from_str(&agent)), ns_string!("User-Agent"));
    // No transparent decompression, so byte counts and offsets are the file's own.
    request.setValue_forHTTPHeaderField(Some(ns_string!("identity")), ns_string!("Accept-Encoding"));
    if let Some(range) = &range {
        request.setValue_forHTTPHeaderField(Some(&NSString::from_str(range)), ns_string!("Range"));
    }
    let task = session.0.dataTaskWithRequest(&request);
    task.resume();
    // `receive` owns the receiver, so it's gone before the session is torn down: a delegate
    // blocked on a full channel gets an error and returns.
    let result = receive(rx, offset, cancel, sink);
    if result.is_err() {
        task.cancel();
    }
    result
}

/// Drives `sink` from the delegate's messages until the task ends, fails or is cancelled.
fn receive(
    rx: Receiver<Msg>,
    offset: u64,
    cancel: &CancelToken,
    sink: &mut dyn FnMut(Fetched) -> Result<(), super::DownloadError>,
) -> Result<(), FetchError> {
    loop {
        if cancel.is_cancelled() {
            return Err(FetchError::Cancelled);
        }
        let msg = match rx.recv_timeout(CANCEL_POLL) {
            Ok(msg) => msg,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => {
                return Err(FetchError::Transient("the connection ended unexpectedly".into()));
            }
        };
        match msg {
            Msg::Response { status: 200, content_length, .. } => {
                // A 200 is the whole file, whatever was asked for.
                let total = u64::try_from(content_length).ok();
                sink(Fetched::Start { offset: 0, total }).map_err(FetchError::Sink)?;
            }
            Msg::Response { status: 206, content_range, .. } => {
                let Some((start, total)) = content_range.as_deref().and_then(parse_content_range) else {
                    return Err(FetchError::Fatal(format!(
                        "the server sent part of the file without saying which part (asked for byte {offset} on)"
                    )));
                };
                sink(Fetched::Start { offset: start, total }).map_err(FetchError::Sink)?;
            }
            Msg::Response { status, .. } => return Err(FetchError::Http(u16::try_from(status).unwrap_or(0))),
            Msg::Data(bytes) => sink(Fetched::Data(&bytes)).map_err(FetchError::Sink)?,
            Msg::Done(None) => return Ok(()),
            Msg::Done(Some(e)) => return Err(classify(&e, cancel)),
        }
    }
}

/// `Content-Range: bytes 100-115/84886208` → `(100, Some(84886208))`; the total may be `*`.
fn parse_content_range(value: &str) -> Option<(u64, Option<u64>)> {
    let rest = value.trim().strip_prefix("bytes")?.trim_start();
    let (span, total) = rest.split_once('/')?;
    let (start, _end) = span.split_once('-')?;
    Some((start.trim().parse().ok()?, total.trim().parse().ok()))
}

/// Sorts an NSURLSession error into retry (offline, transient) or give up (fatal).
fn classify(e: &TaskError, cancel: &CancelToken) -> FetchError {
    let message = e.description.clone();
    if e.domain != "NSURLErrorDomain" {
        return FetchError::Transient(message);
    }
    match e.code {
        -999 if cancel.is_cancelled() => FetchError::Cancelled,
        // Not connected, can't find or reach the host, DNS, data not allowed (cellular, roaming).
        -1009 | -1003 | -1004 | -1006 | -1018 | -1020 => FetchError::Offline(message),
        // Cancelled by something else, bad or unsupported URL, too many redirects, a redirect to
        // nowhere, App Transport Security, TLS and certificates (a captive portal or a TLS
        // inspecting proxy: trying again won't help).
        -999 | -1000 | -1002 | -1007 | -1010 | -1022 | -1206..=-1200 => FetchError::Fatal(message),
        // The idle timeout (-1001), a dropped connection (-1005) and the rest.
        _ => FetchError::Transient(message),
    }
}

/// Free space for a new file in `dir`, counting space macOS can free on demand (purgeable
/// files, local snapshots), which `statvfs` leaves out.
pub(super) fn available_space(dir: &Path) -> std::io::Result<u64> {
    let url = NSURL::fileURLWithPath(&NSString::from_str(&dir.to_string_lossy()));
    let mut value = None;
    // SAFETY: the key's value is an NSNumber, which is what we downcast to.
    let fetched =
        unsafe { url.getResourceValue_forKey_error(&mut value, NSURLVolumeAvailableCapacityForImportantUsageKey) };
    let important = value.and_then(|v| v.downcast::<NSNumber>().ok()).map(|n| n.as_i64());
    match (fetched, important) {
        (Ok(()), Some(bytes)) if bytes > 0 => Ok(bytes as u64),
        _ => super::statvfs_available(dir),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_range_is_parsed() {
        assert_eq!(parse_content_range("bytes 100-115/84886208"), Some((100, Some(84_886_208))));
        assert_eq!(parse_content_range("bytes 0-0/*"), Some((0, None)));
        assert_eq!(parse_content_range("items 1-2/3"), None);
        assert_eq!(parse_content_range("bytes */100"), None);
    }

    #[test]
    fn available_space_is_reported() {
        let free = available_space(&std::env::temp_dir()).unwrap();
        assert!(free > 0);
    }
}
