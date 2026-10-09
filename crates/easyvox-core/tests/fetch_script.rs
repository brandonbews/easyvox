//! `scripts/fetch-model.sh` hard-codes the catalog's GGUF entries (bash can't read
//! `models.rs`). This keeps its table in step with the catalog: same models, same ids, offerings,
//! sizes, checksums, paths and URLs, and the same default.

use easyvox_core::RECOMMENDED_MODEL;
use easyvox_core::models::{self, ModelInfo, Offering, Runtime};

const SCRIPT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts/fetch-model.sh"));

/// One line of the script's `MODELS=( … )` table: `id offering bytes sha256 path url`.
#[derive(Debug)]
struct Row<'a> {
    id: &'a str,
    offering: &'a str,
    bytes: &'a str,
    sha256: &'a str,
    path: &'a str,
    url: &'a str,
}

fn script_rows() -> Vec<Row<'static>> {
    let table: Vec<&str> = SCRIPT
        .lines()
        .skip_while(|l| l.trim() != "MODELS=(")
        .skip(1)
        .take_while(|l| l.trim() != ")")
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    assert!(!table.is_empty(), "no `MODELS=(` … `)` table in scripts/fetch-model.sh");
    table
        .into_iter()
        .map(|line| {
            let unquoted = line.strip_prefix('"').and_then(|l| l.strip_suffix('"'));
            let fields: Vec<&str> = unquoted.expect("one quoted string per model").split_whitespace().collect();
            let [id, offering, bytes, sha256, path, url] = fields[..] else {
                panic!("expected `id offering bytes sha256 path url`, got: {line}");
            };
            Row { id, offering, bytes, sha256, path, url }
        })
        .collect()
}

fn gguf_entries() -> Vec<&'static ModelInfo> {
    models::catalog().iter().filter(|m| m.runtime == Runtime::TranscribeCpp).collect()
}

fn offering_word(o: Offering) -> &'static str {
    match o {
        Offering::Default => "default",
        Offering::Alternative => "alternative",
        Offering::BenchOnly => "bench-only",
    }
}

#[test]
fn script_lists_every_gguf_catalog_entry_as_in_the_catalog() {
    let rows = script_rows();
    for m in gguf_entries() {
        let row = rows
            .iter()
            .find(|r| r.id == m.id)
            .unwrap_or_else(|| panic!("{}: missing from scripts/fetch-model.sh; add a line for it to MODELS", m.id));
        let sha256 = m.sha256.unwrap_or_else(|| panic!("{}: GGUF entries need a sha256 for the script", m.id));
        assert_eq!(row.offering, offering_word(m.offering), "{}: offering", m.id);
        assert_eq!(row.bytes, m.download_bytes.to_string(), "{}: size", m.id);
        assert_eq!(row.sha256, sha256, "{}: sha256", m.id);
        assert_eq!(row.path, m.path, "{}: path", m.id);
        assert_eq!(row.url, m.download_url, "{}: URL", m.id);
    }
}

#[test]
fn script_lists_nothing_outside_the_catalog() {
    let ids: Vec<_> = gguf_entries().iter().map(|m| m.id).collect();
    let rows = script_rows();
    for row in &rows {
        assert!(ids.contains(&row.id), "{}: in scripts/fetch-model.sh but not a GGUF catalog entry", row.id);
    }
    assert_eq!(rows.len(), ids.len(), "scripts/fetch-model.sh lists a model twice");
}

/// The script fetches the one `default` row when it's given no model id.
#[test]
fn script_default_is_the_catalog_default() {
    let defaults: Vec<_> = script_rows().into_iter().filter(|r| r.offering == "default").map(|r| r.id).collect();
    assert_eq!(defaults, [RECOMMENDED_MODEL]);
}
