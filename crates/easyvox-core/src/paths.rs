//! Where EasyVox keeps its data: `~/Library/Application Support/EasyVox` on macOS and
//! `$XDG_DATA_HOME/easyvox` (default `~/.local/share/easyvox`) elsewhere.

use std::path::PathBuf;

/// Root data directory. `EASYVOX_DATA_DIR` overrides it.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("EASYVOX_DATA_DIR") {
        return dir.into();
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/EasyVox")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(".local/share"))
            .join("easyvox")
    }
}

/// Downloaded models (layout: see `ModelInfo::path`). `EASYVOX_MODELS_DIR` overrides it.
pub fn models_dir() -> PathBuf {
    std::env::var_os("EASYVOX_MODELS_DIR").map(PathBuf::from).unwrap_or_else(|| data_dir().join("models"))
}

/// The transcript history database.
pub fn history_db() -> PathBuf {
    data_dir().join("history.sqlite3")
}
