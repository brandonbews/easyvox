//! The workspace's test clips, for unit tests. The spoken ones aren't in the repo (Apple's
//! license doesn't allow publishing its voices): `testdata/generate.sh` makes them. A test that
//! needs them skips without them, so CI, which has none, still runs the rest.

use std::io::Write;
use std::path::PathBuf;

/// `testdata/<name>`, or `None` if it isn't there, after a line on stderr saying the calling
/// test skips. The test then returns.
pub fn clip(name: &str) -> Option<PathBuf> {
    let path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata")).join(name);
    if path.exists() {
        return Some(path);
    }
    // libtest names each test's thread after the test.
    let current = std::thread::current();
    let test = current.name().unwrap_or("a test");
    // Straight to stderr, past the test harness's capture, so a plain `cargo test` shows it.
    let _ = writeln!(std::io::stderr(), "{test} skipped: testdata/{name} is missing (run testdata/generate.sh)");
    None
}

/// Every clip `testdata/manifest.tsv` lists, or `None` as for [`clip`].
pub fn listed() -> Option<Vec<PathBuf>> {
    let manifest = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/manifest.tsv"))
        .expect("read testdata/manifest.tsv");
    manifest
        .lines()
        .filter_map(|l| l.split('\t').next().filter(|id| !id.is_empty()))
        .map(|id| clip(&format!("{id}.wav")))
        .collect()
}
