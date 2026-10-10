//! Live checks against Hugging Face, run by hand when the catalog changes. They need the
//! internet, so they're ignored, and they also do nothing unless `EASYVOX_LIVE_DOWNLOADS=1` is
//! set: the engine checks (`cargo test -p easyvox-core --release -- --ignored`) run every
//! ignored test, and must never download anything.
//!
//!   EASYVOX_LIVE_DOWNLOADS=1 cargo test -p easyvox-core --test download_live -- --ignored
//!
//! - `pinned_urls_match_hugging_face`: a HEAD of every offered model's URL. Three tiny requests,
//!   no body: it catches a catalog typo before anyone downloads 539 MB.
//! - `downloads_whisper_base_en`: a real `download()` of the smallest offered model (85 MB) into
//!   a temporary folder, deleted afterwards. Redirect, CDN, sha256 and activation, end to end.

#![cfg(target_os = "macos")]

use easyvox_core::download::{CancelToken, Progress, download};
use easyvox_core::models;

fn enabled(test: &str) -> bool {
    let on = std::env::var_os("EASYVOX_LIVE_DOWNLOADS").is_some_and(|v| v == "1");
    if !on {
        eprintln!("{test}: skipped; set EASYVOX_LIVE_DOWNLOADS=1 to run it (it uses the internet)");
    }
    on
}

/// Hugging Face answers a `resolve/<commit>/<file>` URL with a redirect to its CDN, carrying
/// the file's size and sha256 (`x-linked-size`, `x-linked-etag`). They must match the catalog.
#[test]
#[ignore = "uses the internet; see the module docs"]
fn pinned_urls_match_hugging_face() {
    if !enabled("pinned_urls_match_hugging_face") {
        return;
    }
    for m in models::offered() {
        // curl is fine in a dev-only test; it doesn't follow the redirect, so no body moves.
        let out = std::process::Command::new("/usr/bin/curl")
            .args(["--silent", "--show-error", "--head", "--max-time", "30", m.download_url])
            .output()
            .expect("run /usr/bin/curl");
        assert!(out.status.success(), "{}: {}", m.id, String::from_utf8_lossy(&out.stderr));
        let headers = String::from_utf8_lossy(&out.stdout).to_lowercase();
        let header = |name: &str| {
            headers
                .lines()
                .find_map(|l| l.strip_prefix(name)?.strip_prefix(':').map(|v| v.trim().trim_matches('"').to_string()))
                .unwrap_or_else(|| panic!("{}: no {name} header in:\n{headers}", m.id))
        };
        assert_eq!(header("x-linked-size"), m.download_bytes.to_string(), "{}: size", m.id);
        assert_eq!(Some(header("x-linked-etag").as_str()), m.sha256, "{}: sha256", m.id);
    }
}

#[test]
#[ignore = "uses the internet; see the module docs"]
fn downloads_whisper_base_en() {
    if !enabled("downloads_whisper_base_en") {
        return;
    }
    let m = models::find("whisper-base.en-q8_0-gguf").unwrap();
    let dir = std::env::temp_dir().join(format!("easyvox-download-live-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let started = std::time::Instant::now();
    let mut events = vec![];
    let result = download(m, &dir, &CancelToken::new(), |p| events.push(p));
    let secs = started.elapsed().as_secs_f64();
    let path = result.unwrap_or_else(|e| panic!("download failed: {e}"));
    eprintln!(
        "downloads_whisper_base_en: {} MB in {secs:.1} s ({:.1} MB/s), {} progress events, {} reconnects",
        m.download_bytes / 1_000_000,
        m.download_bytes as f64 / 1e6 / secs,
        events.len(),
        events.iter().filter(|p| matches!(p, Progress::Reconnecting { .. })).count()
    );
    assert!(m.is_available(&dir));
    assert_eq!(std::fs::metadata(&path).unwrap().len(), m.download_bytes);
    let total = m.download_bytes;
    assert_eq!(events.last(), Some(&Progress::Downloading { done: total, total }));
    let names: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(names.len(), 1, "only the model is left: {names:?}");

    // A second call finds the copy, checks it and makes no request.
    let mut again = vec![];
    download(m, &dir, &CancelToken::new(), |p| again.push(p)).unwrap();
    assert_eq!(again, [Progress::Verifying, Progress::Downloading { done: total, total }]);
    std::fs::remove_dir_all(&dir).unwrap();
}
