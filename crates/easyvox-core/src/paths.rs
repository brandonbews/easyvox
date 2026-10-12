//! Where EasyVox keeps its data: `~/Library/Application Support/EasyVox` on macOS and
//! `$XDG_DATA_HOME/easyvox` (default `~/.local/share/easyvox`) elsewhere. The settings file is
//! `~/.config/easyvox/config.toml` everywhere, macOS included, so dotfile managers and scripts
//! find it where they expect it on any Unix.

use std::ffi::OsString;
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

/// The settings file: `$EASYVOX_CONFIG` if set, else `$XDG_CONFIG_HOME/easyvox/config.toml`,
/// else `~/.config/easyvox/config.toml`. On macOS too: never `~/Library/Preferences`.
pub fn config_file() -> PathBuf {
    let var = |name| std::env::var_os(name);
    config_file_from(var("EASYVOX_CONFIG"), var("XDG_CONFIG_HOME"), var("HOME"))
}

/// [`config_file`] from the values of `EASYVOX_CONFIG`, `XDG_CONFIG_HOME` and `HOME`. Empty
/// values count as unset, and a relative `XDG_CONFIG_HOME` is ignored, as the XDG spec says.
pub fn config_file_from(
    easyvox_config: Option<OsString>,
    xdg_config_home: Option<OsString>,
    home: Option<OsString>,
) -> PathBuf {
    let set = |v: Option<OsString>| v.filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(file) = set(easyvox_config) {
        return file;
    }
    set(xdg_config_home)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| set(home).unwrap_or_default().join(".config"))
        .join("easyvox/config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(easyvox_config: Option<&str>, xdg: Option<&str>, home: Option<&str>) -> PathBuf {
        config_file_from(easyvox_config.map(Into::into), xdg.map(Into::into), home.map(Into::into))
    }

    #[test]
    fn config_file_is_under_dot_config_by_default() {
        let home = Some("/Users/me");
        assert_eq!(resolve(None, None, home), PathBuf::from("/Users/me/.config/easyvox/config.toml"));
        // Empty or relative XDG_CONFIG_HOME is ignored.
        assert_eq!(resolve(None, Some(""), home), PathBuf::from("/Users/me/.config/easyvox/config.toml"));
        assert_eq!(resolve(None, Some("cfg"), home), PathBuf::from("/Users/me/.config/easyvox/config.toml"));
    }

    #[test]
    fn config_file_honours_xdg_config_home() {
        let path = resolve(None, Some("/tmp/xdg"), Some("/Users/me"));
        assert_eq!(path, PathBuf::from("/tmp/xdg/easyvox/config.toml"));
    }

    #[test]
    fn easyvox_config_overrides_everything() {
        let path = resolve(Some("/tmp/test.toml"), Some("/tmp/xdg"), Some("/Users/me"));
        assert_eq!(path, PathBuf::from("/tmp/test.toml"));
        assert_eq!(resolve(Some(""), None, Some("/Users/me")), PathBuf::from("/Users/me/.config/easyvox/config.toml"));
    }
}
