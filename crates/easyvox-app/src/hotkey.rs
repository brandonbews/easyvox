//! The global push-to-talk shortcut, through the `global-hotkey` crate (Carbon
//! `RegisterEventHotKey` on macOS: press and release events, no permission needed).

use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use gpui_kit::Keystroke;
use std::str::FromStr;

/// ⌥Space, in GPUI keystroke syntax.
pub const DEFAULT_SHORTCUT: &str = "alt-space";

pub struct HotkeyService {
    manager: GlobalHotKeyManager,
    current: Option<(String, HotKey)>,
    suspended: bool,
}

impl HotkeyService {
    /// Must run on the main thread (installs a Carbon handler on the app event target).
    /// `on_event(id, pressed)` runs on the main thread for every press and release.
    pub fn new(on_event: impl Fn(u32, bool) + Send + Sync + 'static) -> anyhow::Result<Self> {
        let manager = GlobalHotKeyManager::new()?;
        GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
            on_event(e.id, e.state == HotKeyState::Pressed)
        }));
        Ok(Self { manager, current: None, suspended: false })
    }

    pub fn current_id(&self) -> Option<u32> {
        self.current.as_ref().map(|(_, h)| h.id())
    }

    /// Registers `keystroke` in place of the current shortcut. If the OS refuses it, the old
    /// shortcut is registered again.
    pub fn bind(&mut self, keystroke: &str) -> anyhow::Result<()> {
        let new = to_hotkey(keystroke)?;
        let old = self.current.take();
        if let Some((_, h)) = &old
            && !self.suspended
        {
            let _ = self.manager.unregister(*h);
        }
        match self.manager.register(new) {
            Ok(()) => {
                self.current = Some((keystroke.to_string(), new));
                self.suspended = false;
                Ok(())
            }
            Err(e) => {
                if let Some((s, h)) = old {
                    let _ = self.manager.register(h);
                    self.current = Some((s, h));
                    self.suspended = false;
                }
                Err(anyhow::anyhow!("couldn't register {keystroke}: {e}"))
            }
        }
    }

    /// Releases the OS hot key while the Settings recorder listens; otherwise Carbon swallows
    /// the very combination the user is trying to record.
    pub fn suspend(&mut self) {
        if let Some((_, h)) = &self.current
            && !self.suspended
        {
            let _ = self.manager.unregister(*h);
            self.suspended = true;
        }
    }

    pub fn resume(&mut self) {
        if let Some((_, h)) = &self.current
            && self.suspended
        {
            let _ = self.manager.register(*h);
            self.suspended = false;
        }
    }
}

/// GPUI keystroke (`alt-space`, `ctrl-shift-f5`) → global-hotkey `HotKey`.
pub fn to_hotkey(keystroke: &str) -> anyhow::Result<HotKey> {
    let ks = Keystroke::parse(keystroke).map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let m = &ks.modifiers;
    let mut parts = vec![];
    for (on, name) in [(m.control, "ctrl"), (m.alt, "alt"), (m.shift, "shift"), (m.platform, "cmd")] {
        if on {
            parts.push(name);
        }
    }
    let key = ks.key.to_uppercase();
    parts.push(&key);
    HotKey::from_str(&parts.join("+")).map_err(|e| anyhow::anyhow!("{e}"))
}

/// What the Settings recorder accepts: a key plus at least one modifier (bare F-keys are fine).
/// Returns the GPUI keystroke string.
pub fn accept_recorded(ks: &Keystroke) -> Result<String, String> {
    let m = &ks.modifiers;
    let is_fkey = ks.key.len() >= 2 && ks.key.starts_with('f') && ks.key[1..].parse::<u8>().is_ok();
    if !(m.control || m.alt || m.shift || m.platform) && !is_fkey {
        return Err("Add a modifier (⌃ ⌥ ⇧ ⌘)".into());
    }
    let mut s = String::new();
    for (on, name) in [(m.control, "ctrl-"), (m.alt, "alt-"), (m.shift, "shift-"), (m.platform, "cmd-")] {
        if on {
            s.push_str(name);
        }
    }
    s.push_str(&ks.key);
    to_hotkey(&s).map_err(|e| e.to_string())?;
    Ok(s)
}

/// A shortcut as its keys read on a Mac (`alt-space` → `⌥Space`), for words like "Hold ⌥Space
/// to talk". One that doesn't parse reads as written.
pub fn label(keystroke: &str) -> String {
    Keystroke::parse(keystroke).map_or_else(|_| keystroke.to_string(), |ks| gpui_kit::component::kbd::Kbd::format(&ks))
}

#[cfg(test)]
mod tests {
    use super::*;
    use global_hotkey::hotkey::{Code, Modifiers};

    #[test]
    fn shortcuts_read_as_their_keys() {
        assert_eq!(label(DEFAULT_SHORTCUT), "⌥Space");
        assert_eq!(label("ctrl-shift-k"), "⌃⇧K");
    }

    #[test]
    fn default_shortcut_is_option_space() {
        assert_eq!(to_hotkey(DEFAULT_SHORTCUT).unwrap(), HotKey::new(Some(Modifiers::ALT), Code::Space));
    }

    #[test]
    fn recorder_needs_a_modifier_except_for_function_keys() {
        let ks = |s: &str| Keystroke::parse(s).unwrap();
        assert_eq!(accept_recorded(&ks("ctrl-shift-k")).unwrap(), "ctrl-shift-k");
        assert_eq!(accept_recorded(&ks("f5")).unwrap(), "f5");
        assert!(accept_recorded(&ks("k")).is_err());
    }
}
