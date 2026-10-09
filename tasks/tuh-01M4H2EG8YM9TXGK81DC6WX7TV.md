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

## History

_No activity yet._
