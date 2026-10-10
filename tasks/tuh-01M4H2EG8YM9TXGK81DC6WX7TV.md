# Core: download a model in the app — download, sha256 verify, activate

`tuh-01M4H2EG8YM9TXGK81DC6WX7TV`

- **Status:** open — ready
- **Priority:** 1
- **Labels:** `models` `engine`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: no model ships inside the app; each goes through the same flow: download, sha256 check, then activate (DESIGN.md "Models"). Today only scripts/fetch-model.sh downloads. Onboarding (the default model) and Settings (the alternatives) both need it in the app.

The ask: a core API that downloads a catalog model to its path under the models folder, reports progress, checks size and sha256 against the catalog, and only then makes it available (write to a temp file, rename into place). Cancelable. Same guarantees as fetch-model.sh: a model already there that checks out isn't downloaded again, and a failed or interrupted download leaves nothing behind.

Acceptance: tests against a local HTTP server or file fixture, no internet in the default test run: success; wrong sha256 (nothing left, clear error); interrupted mid-file (nothing at the final path); cancel; already present (no request). Progress is monotonic and ends at the catalog's download_bytes.

Pointers: crates/easyvox-core/src/models.rs (download_url, sha256, download_bytes, path), paths.rs (EASYVOX_MODELS_DIR, EASYVOX_DATA_DIR); scripts/fetch-model.sh; crates/easyvox-core/tests/fetch_script.rs.

Constraints: model downloads are the only network use EasyVox makes (DESIGN.md "Requirements"). Use the pinned Hugging Face commit URLs as they are. Keep the HTTP dependency light: idle memory matters.


---
IMPLEMENTATION PLAN (2026-10-09, read-only research agent; Brandon's answers to section 5 get recorded above this line once decided)

# Plan: download a model in the app (download → sha256 verify → activate)

Ledger task: `tuh-01M4H2EG8YM9TXGK81DC6WX7TV` ("Core: download a model in the app"). It blocks
onboarding (`tuh-01M4H2EG9QB2ETECD30F9RBR58`) and Settings click-to-download
(`tuh-01M4H2EGAHMQ8590ASYQ6FGZ9M`).

Decisions already made, not reopened here: nothing is bundled (even the default model downloads
during onboarding); the flow is download → sha256 verify → activate; URLs stay pinned to a
Hugging Face commit; the default is about to become Parakeet TDT 0.6B v2 Q5_K_M
(`tuh-01M4HET091632AY8RKAC4NWA8J`); macOS only; fewer special cases, one code path.

Everything below keys off the catalog (`RECOMMENDED_MODEL`, `Offering`, `download_bytes`,
`sha256`). Nothing names a specific model, so the default switch can land before or after this
work without touching it. Never hard-code "541 MB" in UI text: format it from `download_bytes`.

> **Before you start:** the working tree has uncommitted changes from another agent (model
> unload: `controller.rs`, `state.rs`, `settings.rs`, `lib.rs` and others). Build on top of them
> and don't revert them. Ask before building or testing: someone may be benchmarking, and the
> machine has to stay quiet while they do. Never touch `recordings/`.

---

## 0. The decisions in one screen

| Question | Decision |
|---|---|
| HTTP | **NSURLSession** (Apple's own networking API) through `objc2-foundation`, which core already depends on: a data task plus a small delegate class. No new crates. Plan B is `/usr/bin/curl` behind the same function signature, if the step-0 gate fails. |
| Hash | `sha2 = "0.11"` in core. That exact crate and version is **already compiled into the app** (gpui-kit-assets → rust-embed → rust-embed-utils), so the app gets no new code and Cargo.lock gets no new package. It uses the ARMv8 SHA-2 instructions automatically. |
| Where bytes go | A hidden `models/gguf/.<file>.part` next to the model, written by our own code, hashed as it streams, `flock`ed while in use. |
| Activate (file) | Size and sha256 match → `sync_all` → `chmod 644` → `rename` into place (atomic, same folder). |
| Activate (app) | The controller's `use_model(id)` swaps engines only while idle, freeing the old model before loading the new one. |
| Resume | Within one `download()` call, on a dropped or stalled connection: up to 5 tries in a row with no new bytes, sent with `Range: bytes=N-`, each try starting again from the pinned `huggingface.co` URL. Never across calls or launches: per the ledger, "a failed or interrupted download leaves nothing behind". |
| Core API | A blocking `download(model, models_dir, &CancelToken, progress_fn) -> Result<PathBuf, DownloadError>`, like `Engine::load`: the caller picks the thread. |
| App threading | One named `std::thread` per download. Progress goes over the existing `async_channel` as `UiEvent::Download`. State lives in the `AppStatus` global. |
| Tests | A small `std::net` HTTP server on `127.0.0.1:0` inside the integration tests. All fault injection is local and runs in the default `cargo test`. Real Hugging Face checks are `#[ignore]`d. |

---

## 1. Download mechanism

### Options weighed

| | NSURLSession (objc2) | `/usr/bin/curl` subprocess | `ureq` 3.4 | `reqwest` 0.12 |
|---|---|---|---|---|
| New crates in the app | **0** (feature flags on `objc2-foundation`) | **0** | about 12: ureq, ureq-proto, rustls, ring (C and asm via `cc`), rustls-webpki, rustls-pki-types, webpki-roots, http, httparse, base64, … | about 40: tokio, hyper, h2, hyper-util, tower, rustls/hyper-rustls, system-configuration, … |
| Binary (est., not built) | ~0 | 0 | +1–2 MB | +3–5 MB, plus a tokio runtime and its threads while it runs |
| TLS and trust | System (Keychain, roots installed by MDM, corporate TLS inspection works) | SecureTransport in Apple's curl 8.7.1 = system Keychain | rustls with Mozilla roots baked in; ignores Keychain roots unless you add `platform-verifier` (more crates) | rustls or native-tls |
| Proxies | **System Settings proxies, PAC, auto-discovery**, Keychain proxy credentials | `*_proxy` env vars only, which a Finder-launched app doesn't have | env vars only (`ALL_PROXY`/`HTTPS_PROXY`/`NO_PROXY`) | System Configuration on macOS (feature) |
| Redirects (huggingface.co → `us.aws.cdn.hf.co`) | Automatic, and the delegate can enforce Range and https | `-L` | Automatic (10) | Automatic |
| Resume (Range) | Manual header, re-applied on redirect in the delegate | `-C <offset>` (also checks Content-Range) | Manual header | Manual header |
| Stall / idle timeout | `timeoutIntervalForRequest` (idle, resets on every byte) | `--speed-limit/--speed-time` | **None**: ureq 3 only has `recv_body`, a total budget that "is not restarted for each read", so a dead socket after sleep can block forever | `read_timeout` (async client) |
| Cancel | `task.cancel()`, immediate | Kill the child, immediate | Only between reads; a blocked read can't be interrupted | Drop the future |
| Progress | Bytes in `didReceiveData` | Bytes read from curl's stdout | Bytes read | Bytes read |
| Sleep and wake | The OS stack fails the task (connection lost or timeout) and we resume | `--speed-time` fires and we resume | May hang (no idle timeout) | `read_timeout` and resume |
| Error text | `NSError.localizedDescription`, localized ("The Internet connection appears to be offline.") | Exit codes plus stderr parsing | Typed errors | Typed errors |
| Firewall apps (Little Snitch, LuLu) | The connection shows as **EasyVox** | Shows as `curl` (via EasyVox) | EasyVox | EasyVox |
| Tests against `http://127.0.0.1` | Works: `nscurl` (NSURLSession, macOS 26.6 SDK plist, **no ATS keys**) fetched plain-HTTP `127.0.0.1` and `localhost` on this Mac. Re-checked in step 0. | Works | Works | Works |
| Linux later | Needs another transport | Works | Works | Works |
| Unsafe code | About 200 lines of delegate glue (`define_class!`) | None | None | None |

### Decision: NSURLSession data task with a delegate

Why it fits this app:

- **The download is mandatory.** With nothing bundled, a failed onboarding download means the app
  can't be used at all. Only NSURLSession gets every network setup right without special code:
  system proxies and PAC, corporate roots in the Keychain, network changes, sleep and wake, and
  HTTP/2 to the CDN.
- **Light.** It adds no crates and costs nothing while idle. CFNetwork is already loaded by
  AppKit. No runtime, and no threads except while a task runs.
- **One code path.** The default model and the alternatives, onboarding and Settings, the first
  try and every resume all go through the same `fetch(url, offset)`.
- **Privacy.** An ephemeral session stores no cookies, cache or credentials on disk, and the
  connection is attributed to EasyVox. That matches "the only network use is downloading
  models" and is easy to audit.

What it costs:

- About 200 lines of Objective-C bridging in core, with `unsafe`. objc2 0.6 delegate methods are
  `extern "C-unwind"`, so **a delegate method must never panic**: no `unwrap`, no indexing, only
  channel sends and plain ObjC calls.
- An NSURLSession retains its delegate until it's invalidated. Invalidate every session from a
  `Drop` guard, so an early return or a panic in a progress callback doesn't leak it.
- It's macOS-only. That's accepted. On other platforms core gets a stub that returns an error
  pointing at `scripts/fetch-model.sh`.

Why not the others:

- **curl** is the runner-up, and Plan B if step 0 fails. It's simple and safe, but it ignores
  system proxies, its errors are exit codes, and it shows up as `curl` in firewalls.
- **ureq** has no idle timeout and can't interrupt a blocked read. A laptop that sleeps
  mid-download can hang until the TCP keepalive, which is off by default. It also brings
  rustls and ring into a binary that has no TLS stack today.
- **reqwest** is heavier than the job needs (a tokio stack) when the ledger asks for "keep the
  HTTP dependency light".

### NSURLSession details for the implementer

- **Configuration.** `NSURLSessionConfiguration::ephemeralSessionConfiguration()`,
  `timeoutIntervalForRequest = idle_timeout` (30 s), `waitsForConnectivity = false` (our retry
  loop does the waiting, so "offline" surfaces as one clear error), `URLCache = None`.
  Per-request headers: `User-Agent: EasyVox/<CARGO_PKG_VERSION>` and
  `Accept-Encoding: identity`, so byte offsets are never changed by transparent decompression.
- **One session per attempt.** `sessionWithConfiguration_delegate_delegateQueue(cfg, Some(delegate), None)`.
  A `None` queue means the session creates its own serial delegate queue, and no run loop is
  needed. Call `finishTasksAndInvalidate()` on success, `invalidateAndCancel()` otherwise, from a
  guard's `Drop`.
- **Delegate class** (`define_class!`, superclass `NSObject`, ivars
  `Mutex<Option<SyncSender<Msg>>>`; any thread, not `MainThreadOnly`). It implements
  `NSURLSessionDelegate`, `NSURLSessionTaskDelegate` and `NSURLSessionDataDelegate`:
  - `URLSession:dataTask:didReceiveResponse:completionHandler:`: downcast to
    `NSHTTPURLResponse`, then send `Msg::Response { status, content_range, expected_len }`. Call
    the handler with `Allow` for 200 or 206 and `Cancel` for anything else.
  - `URLSession:dataTask:didReceiveData:`: send `Msg::Data(data.to_vec())`. Use a bounded
    `sync_channel(32)`: if the worker falls behind, the delegate queue blocks and TCP applies
    backpressure. Ignore send errors.
  - `URLSession:task:willPerformHTTPRedirection:newRequest:completionHandler:`: `mutableCopy`
    the new request, set the same `Range` header, and refuse (`None`) a redirect from https to
    anything else. Don't rely on CFNetwork copying headers by itself.
  - `URLSession:task:didCompleteWithError:`: send `Msg::Done(Option<(domain, code, localizedDescription)>)`.
- **The worker side** (the thread that called `download()`): `task.resume()`, then loop on
  `rx.recv_timeout(100 ms)`. On a timeout, if the `CancelToken` is set, call `task.cancel()`. Each
  `Msg` drives the sink (see §2). So the progress callback and all file I/O run on our thread,
  never on an ObjC queue.
- **Classifying errors** (domain `NSURLErrorDomain`):
  - offline, retried: `-1009` not connected, `-1003` cannot find host, `-1004` cannot connect,
    `-1006` DNS, `-1018`/`-1020` data not allowed;
  - transient, retried: `-1001` timed out (this is the idle timeout), `-1005` connection lost,
    and HTTP 408, 429 and 5xx;
  - fatal: `-1200…-1206` TLS and certificates (a captive portal or a TLS-inspecting proxy:
    retrying won't help), `-999` cancelled, other HTTP 4xx.
- **objc2-foundation features to add in core** (methods are gated by the features of every type
  in their signature, so add whatever the compiler asks for): `NSURLSession`, `NSURLRequest`,
  `NSURLResponse`, `NSURL`, `NSData`, `NSError`, `NSOperation`, `NSDate` (for `NSTimeInterval`),
  `NSDictionary`, `NSValue` (`NSNumber`), `NSURLError`, `block2`. Core already depends on
  `objc2`, `objc2-foundation` and `block2` 0.6.

### Redirects, resume and sleep, as they apply to Hugging Face (checked today)

- `HEAD https://huggingface.co/handy-computer/whisper-base.en-gguf/resolve/<commit>/whisper-base.en-Q8_0.gguf`
  returns **302** to `https://us.aws.cdn.hf.co/xet-bridge-us/…?Expires=…&Signature=…`. The signed
  URL expires about an hour later. The 302 also carries `x-linked-size: 84886208` and
  `x-linked-etag: "3b46ca40…7652"`, which match the catalog's `download_bytes` and `sha256`,
  plus `x-repo-commit` and `accept-ranges: bytes`.
- A ranged GET (`-r 100-115`), with redirects followed, came back **206** with
  `content-range: bytes 100-115/84886208` from CloudFront. So Range works through the redirect.
- So every attempt, resumes included, starts again from the **pinned huggingface.co URL** and
  never reuses the CDN URL. Expiry then never matters.
- **Sleep and wake:** while the Mac sleeps, nothing runs. On wake the in-flight task fails with
  `-1005` or `-1001`, and the retry loop resumes at byte N with backoff 1, 2, 4 and 8 s. That
  covers the few seconds Wi-Fi takes to rejoin. The app holds GPUI's
  `prevent_app_nap("EasyVox model download")` while a download runs, the same guard dictation
  uses (`UserInitiatedAllowingIdleSystemSleep`). So App Nap doesn't throttle the download, while
  idle sleep is still allowed (see open question 3).

---

## 2. Files on disk

### Layout (unchanged, plus one hidden file)

```
<models_dir>/gguf/parakeet-tdt-0.6b-v2-Q5_K_M.gguf     the model (only ever appears complete and verified)
<models_dir>/gguf/.parakeet-tdt-0.6b-v2-Q5_K_M.gguf.part   the download in progress (hidden; flock'ed)
```

`models_dir` comes from `paths::models_dir()` (`EASYVOX_MODELS_DIR`, otherwise
`EASYVOX_DATA_DIR/models`, otherwise `~/Library/Application Support/EasyVox/models`). Add
`ModelInfo::partial_path(models_dir)`, which returns the parent folder joined with
`.{file_name}.part`.

The script uses `.<file>.part.XXXXXX` (from mktemp) and the app uses `.<file>.part` (a fixed name
plus a lock), so the two never touch each other's files. Each tool cleans up only its own
leftovers.

### The `download()` algorithm (core, pure Rust; the transport is the only macOS part)

1. If `model.sha256` is `None`, return `NotDownloadable`. Only the bench-only sherpa archives
   have no sha256; the catalog test already requires one for every offered model.
2. **Already there?** If the final path exists, emit `Progress::Verifying` and hash it (1 MiB
   reads, checking for cancel between reads). If it matches, emit
   `Downloading { done: total, total }` and return `Ok(path)` **without a request**. If it
   doesn't, delete it ("damaged copy") and carry on.
3. `create_dir_all(parent)`. Open the part file with create, write and truncate, then
   `libc::flock(fd, LOCK_EX | LOCK_NB)`. If the lock is taken, return `Busy` (another download of
   this model is running, in this app or another process). Arm a `PartGuard` that deletes the
   part file on `Drop` unless it's disarmed. That covers every error path, cancel, and a panic in
   the progress callback.
4. **Disk space:** if `available_space(parent) < download_bytes + 64 MiB`, return
   `NotEnoughSpace { needed, available }`. On macOS, `available_space` reads the NSURL resource
   value `NSURLVolumeAvailableCapacityForImportantUsageKey`, which counts space APFS can purge.
   Plain `statvfs` under-reports on full disks and would wrongly refuse downloads. Fall back to
   `statvfs` if the key fails. **Also** map `io::ErrorKind::StorageFull` from writes to the same
   error, since space can run out mid-download.
5. **The streaming loop.** `done = 0`, `hasher = Sha256::new()`, `failures = 0`, and a
   `BufWriter` with a 1 MiB capacity over the part file. Each attempt calls
   `fetch(url, done, …)` with a sink:
   - `Start { offset, total }`: if `total` is known and isn't `download_bytes`, return
     `SizeMismatch` **at once**, rather than after 500 MB of the wrong thing (an HTML error page
     with status 200, say). If `offset > done`, it's an error. Otherwise set
     `skip = done - offset`. With 206 that's 0. With 200 (the server ignored Range) the bytes we
     already have are thrown away as they arrive. **One path for both cases**, and progress stays
     monotonic.
   - `Data(bytes)`: drop the first `skip` bytes. If `done + len > download_bytes`, return
     `SizeMismatch`. Then `write_all`, `hasher.update` and `done += len`, and emit
     `Downloading { done, total }` at most every 250 ms.
   - Afterwards: `Ok` with `done == total` ends the loop. `Ok` with `done < total`, or a
     transient error, counts as a failure, reset to zero whenever the attempt brought new bytes.
     After `max_failures` (5) in a row, give up with `Offline` or `Network`. Otherwise emit
     `Reconnecting { attempt, max, done, total }` and back off 1, 2, 4, 8 s in 100 ms slices that
     check for cancel. A fatal error or HTTP 4xx returns at once. Cancel returns `Cancelled`.
6. **Verify:** `done == download_bytes` and `hex(hasher.finalize()) == sha256`. Otherwise return
   `ChecksumMismatch { expected, actual }`, and the guard deletes the part file. **No automatic
   retry**: a mismatch means the file on the server differs, and fetching another 541 MB won't
   fix that. The UI offers Retry.
   Format the digest by hand (`write!(s, "{b:02x}")` per byte): `sha2` 0.11 returns a
   hybrid-array, so don't count on `{:x}` working for it.
7. **Activate the file:** flush, then `sync_all()` (`F_FULLFSYNC` on macOS, so the data is on
   disk before the name appears), `set_permissions(0o644)` (like the script's `chmod 644`),
   `rename(part, final)` (atomic, same folder), disarm the guard, and close (which releases the
   lock). Emit the final `Downloading { done: total, total }` and return `Ok(final)`.

   The parent folder isn't fsynced. The worst a power cut can then do is lose the rename, and the
   model shows up as not downloaded. It never leaves a half file under the model's name.

Progress guarantee, which the tests assert: `done` never decreases across all events, and every
`Ok` ends with `done == download_bytes`.

### What happens to a half or corrupt file on the next launch

- **A half file** only ever exists as `.<file>.part`. `download::remove_partials(models_dir)`
  runs once at app start, on a background thread. It removes every hidden `*.part` file in the
  folders the catalog downloads into (`gguf/` today) **whose lock it can take**. So it can never
  delete the file of a download running in another instance or process. `download()` also
  truncates its own part file when it starts, so a leftover never gets used.
- **A corrupt file at the final path** can't be made by `download()`. It could come from
  something else (a manual copy, bit rot).
  - Launch does a **size check only**: `ModelInfo::is_available` also requires
    `metadata.len() == download_bytes` for single-file entries (the ones with a sha256). It
    stays `exists()` for the bench-only sherpa folders. A truncated copy then shows as not
    downloaded and offers Download.
  - **No sha256 at launch.** Hashing 541 MB on every launch would cost energy for nothing.
  - The full hash runs when `download()` finds an existing copy (any Download or Retry click),
    and right after every download.
  - If a model passes the size check but fails to load, the app shows the load error with
    Re-download. `download()` then re-verifies it and replaces it if it's damaged.

### Multi-file models

They don't matter here. The sherpa-onnx entries are `BenchOnly`, are `.tar.bz2` archives with no
sha256, and their runtime isn't compiled into the app. `download()` refuses them
(`NotDownloadable`). Every offered model is a single GGUF.

If a future model needs several files, the catalog would gain a list of `(path, url, sha256,
bytes)` and `download()` would loop over it, only marking the model available once all of them
are renamed in. Don't build that now.

### Cleanup and delete

- `download::delete(model, models_dir) -> io::Result<()>` removes the model file, plus its part
  file if it isn't locked. Unlinking a file ggml has mapped is safe on macOS: the loaded model
  keeps working until it's freed.
- The app refuses to delete the active model or a model that's downloading. The UI for that is
  the Settings task.
- **Never delete files the catalog doesn't know.** The models folder may be shared with the bench
  (`EASYVOX_MODELS_DIR`) and may hold old catalog files, such as the retired v2 Q8_0. Settings
  can show them, but nothing removes them automatically.

---

## 3. Threading, UI and the core–app API

### Core API sketch (`crates/easyvox-core/src/download.rs`, with the transport in `download/macos.rs`)

```rust
//! Downloading a catalog model: fetch it, check its size and sha256, then rename it into place.

/// Cancels a running `download()` from any thread. Cheap to clone.
#[derive(Clone, Default, Debug)]
pub struct CancelToken(Arc<AtomicBool>);
impl CancelToken { pub fn new() -> Self; pub fn cancel(&self); pub fn is_cancelled(&self) -> bool; }

/// What `download()` reports while it runs. `done` never decreases, and a success always ends
/// with `Downloading { done: total, total }`. At most about 4 events a second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    /// Checking the sha256 of a copy that is already on disk.
    Verifying,
    /// Bytes downloaded and hashed so far, out of the catalog's `download_bytes`.
    Downloading { done: u64, total: u64 },
    /// The connection dropped or stalled. Trying again (from `done`) after a short wait.
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
    NotEnoughSpace { needed: u64, available: u64 },
    /// No connection to the server after retrying (no network, DNS, connection refused).
    Offline(String),
    /// Another network failure after retrying, or one retrying can't fix (TLS, bad redirect).
    Network(String),
    /// The server answered with an error status (404 means the catalog URL is wrong).
    Http(u16),
    SizeMismatch { expected: u64, actual: u64 },
    ChecksumMismatch { expected: String, actual: String },
    /// Reading or writing the models folder failed.
    Disk(String),
}
impl std::fmt::Display for DownloadError { /* user-facing sentences */ }
impl std::error::Error for DownloadError {}

/// Tuning knobs. `Default` is what the app uses; tests shorten the waits.
#[derive(Clone, Debug)]
pub struct DownloadOptions {
    pub idle_timeout: Duration,  // 30 s without a byte fails the attempt
    pub max_failures: u32,       // 5 attempts in a row without new bytes, then give up
    pub first_backoff: Duration, // 1 s, doubling
}

/// Blocking: call it off the UI thread. Downloads `model` into `models_dir`, checks the size
/// and sha256 while streaming, and only then renames it into place. A copy that's already
/// there and checks out returns at once, without a request. `progress` runs on this thread.
pub fn download(model: &ModelInfo, models_dir: &Path, cancel: &CancelToken,
                progress: impl FnMut(Progress)) -> Result<PathBuf, DownloadError>;
pub fn download_with(model: &ModelInfo, models_dir: &Path, options: &DownloadOptions,
                     cancel: &CancelToken, progress: impl FnMut(Progress)) -> Result<PathBuf, DownloadError>;

/// Removes partial files left by downloads that were killed (crash, force quit). Skips any
/// that a running download holds. Call it once at startup.
pub fn remove_partials(models_dir: &Path);

/// Deletes a downloaded model's file.
pub fn delete(model: &ModelInfo, models_dir: &Path) -> std::io::Result<()>;
```

Internal transport, `download/macos.rs` (with a stub returning `Fatal` on other platforms):

```rust
pub(super) enum Fetched<'a> { Start { offset: u64, total: Option<u64> }, Data(&'a [u8]) }
pub(super) enum FetchError { Cancelled, Http(u16), Offline(String), Transient(String), Fatal(String), Sink(DownloadError) }
/// One GET from `offset` to the end of the file, following redirects (Range is re-applied on each).
pub(super) fn fetch(url: &str, offset: u64, idle_timeout: Duration, cancel: &CancelToken,
                    sink: &mut dyn FnMut(Fetched) -> Result<(), DownloadError>) -> Result<(), FetchError>;
pub(super) fn available_space(dir: &Path) -> std::io::Result<u64>;
```

Why a blocking function rather than a `Session`-style handle with its own thread: it matches
`Engine::load` (blocking, the caller picks the thread), tests call it directly with no threads or
channels, and the app already owns its threads and its event channel. Re-export it as
`easyvox_core::download` (`pub mod download;` in `lib.rs`).

### App plumbing (shared by onboarding and Settings; do it once, here)

`state.rs`:

```rust
pub struct AppStatus {
    pub engine: EngineStatus,
    /// Downloads running or failed, by catalog id. Removed on success and on cancel.
    pub downloads: HashMap<&'static str, DownloadStatus>,
    /* …existing fields… */
}
#[derive(Clone, Debug)]
pub enum DownloadStatus { Running(easyvox_core::download::Progress), Failed { error: String } }
pub enum EngineStatus { /* … */ Missing, /* the chosen model isn't downloaded */ }
```

`controller.rs`:

```rust
pub enum UiEvent { /* … */ Download { model: &'static str, update: DownloadUpdate } }
pub enum DownloadUpdate { Progress(Progress), Done(Result<PathBuf, DownloadError>) }

struct DownloadJob { cancel: CancelToken, _no_app_nap: ActivityGuard }
// Controller gains: downloads: HashMap<&'static str, DownloadJob>, pending_model: Option<String>

pub fn download_model(&mut self, id: &str, cx: &mut Context<Self>);  // no-op if it's already running
pub fn cancel_download(&mut self, id: &str);                           // Done(Err(Cancelled)) follows and clears the status
pub fn use_model(&mut self, id: &str, cx: &mut Context<Self>);        // the one way to switch models (see below)
pub fn delete_model(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String>; // refuses the active model or a running download
fn on_download(&mut self, model: &'static str, update: DownloadUpdate, cx: &mut Context<Self>);
```

- **The worker thread.** `download_model` spawns `std::thread::Builder::new().name(format!("download-{id}"))`.
  The thread calls `easyvox_core::download::download(info, &paths::models_dir(), &cancel, |p| tx.send_blocking(UiEvent::Download { model: id, update: Progress(p) }))`,
  then sends `Done(result)`. It's a dedicated thread, not `background_spawn`: GPUI's background
  executor is GCD, and parking one of its threads for minutes is the wrong use. The channel is
  the same `async_channel` that already carries tray, hot key and transcript events. Core
  throttles to about 4 events a second, and the UI handles each one with `cx.refresh_windows()`,
  as every other status change does.
- **`on_download`.** `Progress` updates `AppStatus.downloads`. `Done(Ok)` removes the job and the
  status, and if `id == settings.model && self.engine.is_none()` (first launch, or the active
  model was missing), it calls `use_model(id)`. That load is the shader warm-up onboarding wants.
  `Done(Err(Cancelled))` clears the status, and any other error sets `Failed { error }`.
  Downloads are keyed by id with no limit, so a concurrency rule would be one more rule to keep.
- **`load_engine`** (exists today): if the model isn't available, set `EngineStatus::Missing`
  and don't try to load it. `start_session`'s notice for `Missing` reads "Download a speech model
  first: menu bar icon → Settings…".
- **Startup** (`main.rs`): run `remove_partials` on a background thread. The existing
  `load_engine` call then shows `Missing` instead of a load error on a fresh data folder.
- **Quit** (`MENU_QUIT`): cancel every running download, best-effort. The next launch's sweep
  removes whatever part file the exit leaves behind.

### How the engine switches to the new model

`use_model(id)` is the only way to switch. Settings' dropdown calls it instead of `load_engine`,
and so does `on_download`.

1. **While dictating** (`!is_idle()`), store `pending_model = Some(id)` and stop. The `Final`
   handler calls `use_model` again after `deliver`. A switch never happens in the middle of a
   session.
2. Set `settings.model = id`. If the file isn't available, set `Missing` and stop.
3. **Free the old engine first** (`self.engine.take()`, dropped on the background executor), then
   `load_engine(id)`. The Settings task requires that "only one model is ever loaded". Today's
   `load_engine` holds the old engine until the new one is ready, so for a moment there are two
   (about 1.3 GB). A key press during the load (about 0.2 s, or about 14 s on the very first
   launch) gets the existing "still loading" notice.
4. The shader cache is per bundle id, so only the first-ever load pays the ~14 s compile.
   Switching models later is quick.

### A download running while the user dictates

They're independent. The download is network and disk I/O on its own thread. Hashing at
100 Mb/s with the ARMv8 SHA instructions takes well under 1 % of a core, and the buffers are at
most about 2 MB. It never touches the engine, its decode lock or the GPU. Progress events share
the UI channel with transcript events but never touch the session.

The only place the two meet is a model switch, which `pending_model` defers until the session's
`Final`. Onboarding can't overlap with dictation anyway: there's no engine until its download
finishes.

### Minimal UI in this task (optional, about 40 lines, for a hands-on check)

So the plumbing can be tried before onboarding lands: in Settings → General → Model, when
`engine` is `Missing`, show "Not downloaded (<size>)" with a **Download** button. While it runs,
show "Downloading… 120 of 539 MB" with **Cancel**. After a failure, show the error with
**Retry**. Build every size from `download_bytes`, in decimal MB like Finder and the script. The
full model list (every offered model, delete, disk usage) is the Settings task.

---

## 4. Tests

### Where and how

- **`crates/easyvox-core/tests/download.rs`**, using a local HTTP/1.1 server in the same file
  (or `tests/support/http.rs`), about 120 lines:
  - `std::net::TcpListener::bind("127.0.0.1:0")` with an accept thread and one thread per
    connection;
  - it parses the request line and headers up to `\r\n\r\n` and replies `Connection: close`;
  - it logs every request (`path`, `Range` start) in `Arc<Mutex<Vec<…>>>`, so tests can assert
    "no request" and "the resume sent `Range: bytes=N-`";
  - each test sets a behaviour: serve with Range support (206 and `Content-Range`), redirect
    `/resolve/…` → `/cdn/…` with 302, 404, cut after N bytes (for the first k requests), ignore
    Range (always 200), stall (send some bytes, then hold the socket longer than the idle
    timeout), trickle (64 KiB every 50 ms), and serve different bytes of the right size.
  - Use `127.0.0.1`, not `localhost`. ATS exempts IP literals, and `nscurl` confirmed plain HTTP
    to a loopback server works with an Info.plist that has no ATS keys.
- **The payload** is 4 MiB from a xorshift seed: enough for many `didReceiveData` chunks, and
  quick. The expected sha256 is computed in the test with `sha2`. The test `ModelInfo` is the
  catalog default with its fields overridden: `ModelInfo { id: "test", path: "gguf/test.gguf",
  download_url: String::leak(url), sha256: Some(String::leak(hex)), download_bytes: n,
  disk_bytes: n, ..models::find(RECOMMENDED_MODEL).unwrap().clone() }`. Every field is `pub`,
  and `download()` takes `&ModelInfo`, not a catalog lookup.
- **Temp folders** follow the existing pattern (`std::env::temp_dir()` with the pid and test
  name, removed at the end), with no `tempfile` crate.
- **Short waits** come from `download_with(…, &DownloadOptions { idle_timeout: 1 s,
  max_failures: 3, first_backoff: 10 ms }, …)`.
- **A shared helper** asserts after every test: progress `done` never decreases; a success ends
  with `done == download_bytes`; a failure leaves the `gguf/` folder **empty** (no model, no
  part file).

### Test list (★ = the ledger's acceptance cases)

1. ★ **Success:** the file is at its path with the right sha256 and mode 0644, there's no part
   file, there was exactly one request, and progress is monotonic and ends at `download_bytes`.
2. ★ **Wrong sha256** (right size, different bytes): `ChecksumMismatch` with both hex strings,
   and the folder is empty.
3. ★ **Interrupted mid-file, recovered:** the first request is cut at 1 MiB. The second request
   carries `Range: bytes=1048576-`, and the download succeeds with monotonic progress.
4. ★ **Interrupted every time:** cut on every request, giving `Network` or `Offline` after
   `max_failures` attempts, with **nothing at the final path** and no part file.
5. ★ **Cancel:** with a trickling server, cancel from the progress callback after the first
   `Downloading` event. Expect `Err(Cancelled)` within 1 s, and an empty folder.
6. ★ **Already present:** write the correct payload first. The result is `Ok`, with **zero
   requests** in the server log and `Verifying` then `Downloading { total, total }` as the
   progress events.
7. **Already present but damaged:** it's replaced, with one request.
8. **404:** `Http(404)` after one request (no retries), and an empty folder.
9. **Redirect plus resume:** a 302 to `/cdn/…`, cut once. The resumed request *at the redirect
   target* carries the Range header. This is the Hugging Face case.
10. **Range ignored** (always 200): it still succeeds, progress stays monotonic, and the bytes
    are right.
11. **Stall:** the first request stalls for longer than the 1 s idle timeout, then the server
    serves normally. Expect a `Reconnecting` event and success.
12. **Wrong size announced** (`Content-Length` ≠ `download_bytes`): `SizeMismatch` before the
    body is read.
13. **Busy:** a second `download()` of the same model while the first is trickling gets `Busy`.
    The first finishes fine.
14. **`remove_partials`:** it deletes a stale `.test.gguf.part`, keeps a locked one (hold `flock`
    in the test), and keeps `.x.gguf.part.AB12CD` (the script's) and unrelated files.
15. **No checksum:** an entry with `sha256: None` gets `NotDownloadable`, with no request.
16. **Unit tests** (in `download.rs`): the space decision (`needed` vs `available`), the
    `StorageFull` → `NotEnoughSpace` mapping, the hex formatting, and the progress throttle
    (always emits the last event).
17. **`ModelInfo::is_available`:** false for a file of the wrong size, true for the right size
    (in `models.rs` tests, with a sparse file of `download_bytes` made via `set_len`, no real
    model).

The existing drift guard (`tests/fetch_script.rs`, script table vs catalog) stays as it is.

### CI (macOS, private repo, minutes are scarce)

- All of the above runs in the existing `cargo test --workspace --locked` step, with **no
  internet**: the server is on loopback and the payloads are 4 MiB in memory. Expect a few
  seconds of runtime; the stall test is the slowest at about 1.2 s. No `ci.yml` change.
- Nothing new to download or cache: `sha2` 0.11 is already built for the app, and Cargo.lock
  only gains an edge.
- **Live checks are `#[ignore]`d**, in `crates/easyvox-core/tests/download_live.rs`, run by hand
  when the catalog changes:
  - `pinned_urls_match_hugging_face`: a `HEAD` of every offered model's URL (via `/usr/bin/curl
    -sI`, which is fine in a dev-only test) checks that `x-linked-size == download_bytes` and
    `x-linked-etag == sha256`. Three tiny requests and no body: it catches a catalog typo before
    anyone downloads 541 MB.
  - `downloads_whisper_base_en`: a real end-to-end `download()` of the 85 MB model into a temp
    folder.
  - Run them with `cargo test -p easyvox-core --test download_live -- --ignored`. That's a
    separate test file, so the existing `--ignored` engine run in the README never downloads
    anything.

---

## 5. Open questions for Brandon

1. **The HTTP mechanism: NSURLSession (about 200 lines of Objective-C bridging in core) or
   `/usr/bin/curl` (no unsafe code, same tool as the script)?**
   *Recommendation: NSURLSession.* Every user has to get the onboarding download through
   whatever network they're on, and only NSURLSession honours System Settings proxies and PAC
   and shows the connection as EasyVox in firewall apps. curl ignores system proxies. Both add
   no crates.
2. **Keep `scripts/fetch-model.sh` and its drift-guard test, or replace them with a CLI on the
   core downloader (for example `easyvox-bench fetch <id>`), so there's literally one download
   code path?**
   *Recommendation: keep the script for now.* It works on a fresh clone before the first
   2-minute build, needs nothing but macOS, and the drift guard already keeps it honest.
   Revisit once onboarding ships and developers can just use the app.
3. **During a download, let the Mac idle-sleep (the download resumes after wake), or keep it
   awake until the download finishes?**
   *Recommendation: let it sleep.* That's GPUI's existing App Nap guard, so no new code, and
   resume after wake is built in. Keeping the Mac awake is about 10 lines if you prefer it for
   onboarding.

Assumed rather than asked, from the ledger: no resume across calls or app launches (a cancelled
or failed download leaves nothing behind), and no automatic retry after a checksum mismatch.

---

## 6. Steps, file by file

Claim the task first (`tuhdoo protocol`, then claim `tuh-01M4H2EG8YM9TXGK81DC6WX7TV`), and note
the step-0 result on it.

### Step 0: go/no-go gate (about 30 minutes, throwaway)

A scratch integration test in core: NSURLSession `dataTask` against the local test server on
`http://127.0.0.1:<port>`, with a 302 → `/cdn/` route and `Range: bytes=10-`. Confirm the
following:

- ATS lets plain HTTP to `127.0.0.1` through from a `cargo test` binary;
- with `willPerformHTTPRedirection` re-applying the header, the Range header reaches the redirect
  target;
- `didReceiveData` gets several chunks for 4 MiB.

**If any of these fail, switch to Plan B (curl)** behind the same `fetch()` signature:
`Command::new("/usr/bin/curl")` with `--fail --location --silent --show-error --connect-timeout
30 --speed-limit 1 --speed-time <idle> --continue-at <offset> --output - --write-out
'%{stderr}%{http_code}\n'`. Read stdout in 256 KiB chunks and `kill()` the child on cancel. Map
the exit codes:

- 6, 7 → offline;
- 18, 28, 56, 92 → transient;
- 22 → `Http` (from the write-out);
- 33 → range not honoured, so restart from 0 through the same skip logic;
- 35, 51, 58, 60 → fatal TLS.

In production, add `--proto =https --proto-redir =https` (left out only for `http://127.0.0.1`
in tests). Everything else in this plan stays the same.

### Phase 1: core (the ledger task's acceptance)

1. `crates/easyvox-core/Cargo.toml`
   - Add `sha2 = "0.11"` under `[dependencies]`, with a short comment: the same version the app
     already builds through gpui-kit, so it adds nothing there.
   - Under the macOS `objc2-foundation` features, add the NSURLSession set from §1. Add only
     what the compiler needs.
   - Commit the Cargo.lock change, which should be an edge only, with no new package.
2. `crates/easyvox-core/src/models.rs`
   - Add `ModelInfo::partial_path(&self, models_dir) -> PathBuf`.
   - Have `is_available` also check `len == download_bytes` when `sha256.is_some()`, and update
     its doc comment.
   - Update the module doc: "`download.rs` downloads these entries; `scripts/fetch-model.sh`
     mirrors the GGUF entries".
   - Add the test from list item 17.
3. `crates/easyvox-core/src/download.rs` (new; it can become `download/mod.rs`)
   - The public API from §3 and the algorithm from §2.
   - `PartGuard`, `flock` through `libc` (already a dependency), the progress throttle,
     `remove_partials`, `delete`, `Display` for `DownloadError`, and the unit tests from item 16.
   - Module docs covering the guarantees: nothing at the final path unless verified; nothing
     left behind on failure or cancel; resume only within a call.
4. `crates/easyvox-core/src/download/macos.rs` (new): the delegate class, `fetch`,
   `available_space`, and the session guard whose `Drop` invalidates the session. Then
   `download/other.rs` (or a `#[cfg(not(target_os = "macos"))]` block): a `fetch` that returns
   `Fatal("in-app downloads are macOS-only; use scripts/fetch-model.sh")`, and
   `available_space` via `statvfs`.
5. `crates/easyvox-core/src/lib.rs`: `pub mod download;`, plus one line in the crate docs
   pointing at it.
6. `crates/easyvox-core/tests/download.rs` (new): the server plus tests 1–15.
7. `crates/easyvox-core/tests/download_live.rs` (new): the two `#[ignore]` tests from §4.

### Phase 2: app plumbing (so onboarding and Settings only add views)

8. `crates/easyvox-app/src/state.rs`: `DownloadStatus`, `AppStatus.downloads`,
   `EngineStatus::Missing`, with doc comments.
9. `crates/easyvox-app/src/controller.rs`:
   - `UiEvent::Download` and `DownloadUpdate`; route `handle` to `on_download`;
   - the `DownloadJob` map and `pending_model`;
   - `download_model`, `cancel_download`, `use_model`, `delete_model`;
   - `load_engine` sets `Missing` when the file isn't there;
   - the `Final` handler applies `pending_model`;
   - the `start_session` notice for `Missing`;
   - `MENU_QUIT` cancels downloads;
   - update the module doc ("downloads run on their own thread; …").
10. `crates/easyvox-app/src/main.rs`: `remove_partials` on a background thread before
    `load_engine`.
11. `crates/easyvox-app/src/settings.rs`: the dropdown calls `use_model`. Add the `Missing` arm
    to the engine-status text, plus the optional minimal Download, Cancel and Retry row from §3.
12. `crates/easyvox-app/src/selftest.rs`: no behaviour change. Make sure the new variant compiles
    in its `matches!`. The self-test must never touch the network.

### Phase 3: docs and ledger

13. `DESIGN.md`
    - "Models": add the in-app flow in two or three sentences (hidden `.part` next to the model,
      hashed while streaming, renamed into place; resume only within a download; nothing left
      behind).
    - "Local storage": the `.part` files.
    - A line naming the hosts contacted (`huggingface.co` and its CDN, `*.hf.co`), and that
      downloads go through NSURLSession, so system proxy settings apply. That's for
      privacy-minded users with firewalls.
    - "Engine and macOS gotchas": delegate methods must never panic (C-unwind into CFNetwork).
14. `README.md`, step 4: say the app downloads models itself (Settings, and onboarding once it
    lands), and that the script stays for development. Mention the
    `download_live -- --ignored` check.
15. Ledger:
    - note the step-0 result and the transport choice on the task, then close it;
    - file a **held `human, hands-on`** task: "Hands-on: download a model in the app — progress,
      cancel, Wi-Fi off mid-download, sleep and wake mid-download, behind a system proxy if you
      have one";
    - file an inbox idea if open question 2 stays open.

---

## 7. Acceptance checks for the implementer (run them when the machine is free)

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo clippy -p easyvox-app --all-targets --features self-test --locked -- -D warnings`
- `cargo test -p easyvox-core --test download`: all pass, in seconds, offline. As a one-time
  sanity check, run it with Wi-Fi off: it must still pass.
- `cargo test --workspace --locked`: green, including `fetch_script.rs` and the catalog tests.
- `git diff Cargo.lock` adds only `"sha2"` to easyvox-core's dependency list, with no new
  `[[package]]`.
- `rg -n 'unwrap|expect|\[' crates/easyvox-core/src/download/macos.rs`: none inside delegate
  methods.
- When allowed (network):
  `cargo test -p easyvox-core --test download_live -- --ignored` passes. The HEAD check covers
  all offered models, and Whisper base.en is downloaded and verified into a temp folder.
- App, by hand: `EASYVOX_DATA_DIR=$(mktemp -d) open dist/EasyVox.app` (after
  `scripts/bundle.sh --release`). Settings shows "Not downloaded", and Download shows progress.
  Cancel leaves `models/gguf/` empty. A second Download finishes and the model loads (`Ready`).
  A relaunch skips the download, and `models/gguf/` has no `.part` file.

## 8. Size

| Area | Lines (approx.) |
|---|---|
| Core: `download.rs` orchestration | 260 |
| Core: `download/macos.rs` NSURLSession plus disk space | 230 |
| Core: `models.rs`, `lib.rs`, `Cargo.toml` | 30 |
| Core tests: server, 15 integration tests, units, live | 500 |
| App: state, controller, main, settings (with the minimal row) | 200 |
| Docs | 30 |
| **Total** | **about 1,250**, roughly 40 % tests. One focused agent session for Phase 1, plus half a session for Phase 2. |

## Appendix: facts checked for this plan (2026-10-09)

- **No HTTP or TLS stack in the app's macOS build today.** `cargo tree -i` shows that `hyper`,
  `rustls` and `tokio` only come in through `gpui-pre-reqwest` behind a wasm-only target, and
  `ureq` 2.12.1 only as a build dependency of `sherpa-onnx-sys` (not in the app). `sha2 0.11.0`
  *is* in the app's normal dependencies, through `gpui-kit-assets → rust-embed →
  rust-embed-utils`.
- **ureq 3.4.2:** default features are `rustls` (ring) and `gzip`; it reads the `ALL_PROXY`,
  `HTTPS_PROXY`, `HTTP_PROXY` and `NO_PROXY` env vars; its only timeouts are totals
  (`timeout_recv_body`: "The budget is not restarted for each read"). Sources:
  [docs.rs Timeouts](https://docs.rs/ureq/latest/ureq/config/struct.Timeouts.html),
  [ConfigBuilder](https://docs.rs/ureq/latest/ureq/config/struct.ConfigBuilder.html),
  [Proxy](https://docs.rs/ureq/latest/ureq/struct.Proxy.html),
  [features](https://docs.rs/crate/ureq/latest/features).
- **`/usr/bin/curl`:** 8.7.1, `(SecureTransport) LibreSSL/3.3.6`, MultiSSL, HTTP2.
- **Hugging Face:** a 302 to `us.aws.cdn.hf.co` (signed, expires in about 1 h); `x-linked-size`
  and `x-linked-etag` match the catalog for Whisper base.en; the CDN answers a Range request
  with 206 and `Content-Range`.
- **ATS:** `/usr/bin/nscurl` (NSURLSession, embedded Info.plist from the macOS 26.6 SDK with no
  ATS keys) fetched `http://127.0.0.1:<port>/` and `http://localhost:<port>/` from a local
  server. Background reading:
  [Apple forums: ATS and local IP addresses](https://developer.apple.com/forums/thread/66417).
- **objc2-foundation 0.3.2** has every NSURLSession delegate method this plan uses
  (`URLSession_dataTask_didReceiveData`, `…didReceiveResponse_completionHandler`,
  `…willPerformHTTPRedirection…`, `…didCompleteWithError`), `ephemeralSessionConfiguration`,
  `setTimeoutIntervalForRequest`, `setWaitsForConnectivity`, `NSData::to_vec`, and
  `NSURLVolumeAvailableCapacityForImportantUsageKey`. objc2 0.6.5 `define_class!` methods are
  `extern "C-unwind"`.
- **GPUI's `prevent_app_nap`** uses `NSActivityOptions::UserInitiatedAllowingIdleSystemSleep`.

## History

### 2026-10-10 00:07 UTC — edit by `brandon`

description edited
