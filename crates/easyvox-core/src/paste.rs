//! Clipboard + simulated Cmd+V (macOS). The transcript is left on the clipboard on purpose.
//!
//! EasyVox only ever *writes* the clipboard. On macOS 15.4+ reading it programmatically can
//! show a paste-privacy prompt, so there is deliberately no read function here.

use anyhow::bail;

/// Result of a clipboard write, from pasteboard metadata only (reading metadata such as
/// `changeCount` and `types` does not trigger macOS's paste-access alert).
#[derive(Clone, Debug)]
pub struct ClipboardWrite {
    pub change_count_before: isize,
    pub change_count_after: isize,
    pub has_string_type: bool,
    /// `NSPasteboard.accessBehavior` (macOS 15.4+): 0 default, 1 ask, 2 always allow, 3 always deny.
    pub access_behavior: Option<isize>,
}

#[cfg(target_os = "macos")]
mod mac {
    use super::ClipboardWrite;
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
    use objc2_core_graphics::{CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation};
    use objc2_foundation::{NSArray, NSString};

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
        /// A CFStringRef, toll-free bridged to NSString.
        static kAXTrustedCheckOptionPrompt: *const std::ffi::c_void;
    }

    /// Whether macOS lets this app post the ⌘V keystroke. Never prompts. It follows the
    /// switch in System Settings live: no relaunch needed after it's turned on.
    pub fn accessibility_trusted() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    /// Shows macOS's Accessibility prompt ("… would like to control this computer using
    /// accessibility features"), whose button opens System Settings at the right list, and adds
    /// the app to that list, switched off. Returns at once with whether the app is already
    /// allowed; the grant itself comes later, when the user turns the switch on. Only call it
    /// when the user asked: it puts a dialog in front of them.
    pub fn prompt_accessibility() -> bool {
        use objc2_foundation::{NSDictionary, NSNumber, NSString};
        // SAFETY: a CFStringRef constant from ApplicationServices, valid for the whole process.
        let Some(key) = (unsafe { kAXTrustedCheckOptionPrompt.cast::<NSString>().as_ref() }) else {
            return accessibility_trusted();
        };
        let yes = NSNumber::new_bool(true);
        let options = NSDictionary::from_slices(&[key], &[&*yes]);
        // SAFETY: NSDictionary is toll-free bridged to the CFDictionaryRef the call takes.
        unsafe { AXIsProcessTrustedWithOptions(objc2::rc::Retained::as_ptr(&options).cast()) }
    }

    pub fn copy_to_clipboard(text: &str) -> anyhow::Result<ClipboardWrite> {
        let pb = NSPasteboard::generalPasteboard();
        let ty = unsafe { NSPasteboardTypeString };
        let before = pb.changeCount();
        pb.clearContents();
        if !pb.setString_forType(&NSString::from_str(text), ty) {
            anyhow::bail!("NSPasteboard setString:forType: returned NO");
        }
        let after = pb.changeCount();
        let types = NSArray::from_slice(&[ty]);
        let has_string_type = pb.availableTypeFromArray(&types).is_some();
        let access_behavior = Some(pb.accessBehavior().0);
        Ok(ClipboardWrite { change_count_before: before, change_count_after: after, has_string_type, access_behavior })
    }

    /// Posts Cmd+V key down/up to the HID event tap. Key code 9 is the ANSI "V" position,
    /// with the flags forced to Command only. Non-QWERTY layouts may need a layout lookup.
    pub fn post_cmd_v() -> anyhow::Result<()> {
        const KVK_ANSI_V: u16 = 9;
        let src = CGEventSource::new(CGEventSourceStateID::HIDSystemState);
        for down in [true, false] {
            let ev = CGEvent::new_keyboard_event(src.as_deref(), KVK_ANSI_V, down)
                .ok_or_else(|| anyhow::anyhow!("CGEventCreateKeyboardEvent returned NULL"))?;
            CGEvent::set_flags(Some(&ev), CGEventFlags::MaskCommand);
            CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&ev));
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
pub use mac::{accessibility_trusted, copy_to_clipboard, prompt_accessibility};

/// Puts text on the clipboard (and leaves it there), then simulates Cmd+V.
/// Returns a clear error if the process lacks the Accessibility permission; the text is
/// still on the clipboard in that case.
#[cfg(target_os = "macos")]
pub fn paste_text(text: &str) -> anyhow::Result<()> {
    mac::copy_to_clipboard(text)?;
    if !mac::accessibility_trusted() {
        bail!(
            "Accessibility permission is missing, so Cmd+V can't be simulated (AXIsProcessTrusted() = false). \
             The text is on the clipboard. Grant access in System Settings > Privacy & Security > Accessibility."
        );
    }
    mac::post_cmd_v()
}

#[cfg(not(target_os = "macos"))]
pub fn accessibility_trusted() -> bool {
    false
}
#[cfg(not(target_os = "macos"))]
pub fn prompt_accessibility() -> bool {
    false
}
#[cfg(not(target_os = "macos"))]
pub fn copy_to_clipboard(_text: &str) -> anyhow::Result<ClipboardWrite> {
    bail!("clipboard not implemented on this platform yet")
}
#[cfg(not(target_os = "macos"))]
pub fn paste_text(_text: &str) -> anyhow::Result<()> {
    bail!("paste not implemented on this platform yet")
}
