//! OS-specific code that GPUI, GPUI Kit, tray-icon and global-hotkey don't cover. Each
//! platform module exposes the same items; only macOS exists today, and a Linux port adds a
//! sibling module here.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;
