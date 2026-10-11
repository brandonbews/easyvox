//! macOS: the System Settings deep links, the overlay panel shim, key guard and open and close
//! animations, Reduce motion, and the right-⌥ event tap. Small helpers the self-test needs sit
//! at the bottom, behind the `self-test` feature.

use anyhow::Context as _;
use objc2::Message as _;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSPanel, NSView, NSWindow, NSWindowAnimationBehavior, NSWindowStyleMask, NSWorkspace};
use objc2_foundation::{NSNumber, NSString};
use objc2_quartz_core::{
    CABasicAnimation, CALayer, CAMediaTiming as _, CAMediaTimingFunction, CAMediaTimingFunctionName, CATransaction,
    kCAFillModeForwards, kCAMediaTimingFunctionEaseIn, kCAMediaTimingFunctionEaseOut,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::ffi::c_void;
use std::time::Duration;

// ───────────────────────── System Settings ─────────────────────────

/// Deep links to System Settings → Privacy & Security, at one list.
pub const PRIVACY_MICROPHONE: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";
pub const PRIVACY_ACCESSIBILITY: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
pub const PRIVACY_INPUT_MONITORING: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent";

// ───────────────────────── Overlay panel ─────────────────────────

/// The overlay's native window, set up so it can never take keyboard focus or clicks, and its
/// open and close.
///
/// GPUI's `WindowKind::PopUp` is already most of what the overlay needs: a `GPUIPanel` (an
/// NSPanel subclass) with the non-activating style bit, the pop-up level (101), and
/// `CanJoinAllSpaces | FullScreenAuxiliary`. What it lacks: click-through, a truly borderless
/// style mask (GPUI always adds Titled), no shadow, `hidesOnDeactivate = NO`, and a panel that
/// refuses key status (see [`key_guard`]).
///
/// The open and close are Core Animation on the layer of GPUI's view: a fade and a slide that
/// the window server runs, so the app neither redraws nor wakes up for them. Moving the window
/// instead would make GPUI redraw the pill at every step (it redraws whenever its window
/// moves). Each animation starts from the layer as it's drawn at that moment, so a show during
/// a close turns it around without a jump.
#[derive(Clone)]
pub struct OverlayPanel {
    win: Retained<NSWindow>,
    /// GPUI's view. Its layer is what the open and close animate; its `alphaValue` is the
    /// layer's resting opacity (AppKit owns the layer of a view that only wants one).
    view: Retained<NSView>,
}

/// `NSWindowStyleMaskNonactivatingPanel`, which objc2-app-kit doesn't name.
const NONACTIVATING_PANEL: usize = 1 << 7;

/// The keys of the open and close's animations on the layer. Adding one under the same key
/// replaces the one running.
const FADE: &str = "easyvox-fade";
const SLIDE: &str = "easyvox-slide";

impl OverlayPanel {
    /// Configures the window GPUI created for the overlay. Call it inside `open_window`.
    pub fn attach(window: &gpui_kit::Window, cx: &mut gpui_kit::App) -> anyhow::Result<Self> {
        let view = ns_view_of(window).context("no NSView behind the overlay")?;
        let win = view.window().context("no NSWindow behind the overlay")?;
        win.setIgnoresMouseEvents(true);
        win.setHasShadow(false);
        win.setHidesOnDeactivate(false);
        // GPUI gives a pop-up a utility window's ordering animation; the open and close here
        // are the only ones.
        win.setAnimationBehavior(NSWindowAnimationBehavior::None);
        if let Some(panel) = win.downcast_ref::<NSPanel>() {
            panel.setBecomesKeyOnlyIfNeeded(true);
        }
        // A new style mask makes AppKit resize GPUI's view there and then (and back: the
        // content size ends up the same), and GPUI's resize handler borrows the App, which is
        // borrowed until GPUI's update ends. Done here, it would log "RefCell already borrowed"
        // (gpui window.rs, on_resize). So it waits for the next turn of the run loop, as GPUI
        // defers its own appearance changes. The key guard waits with it: it must come after
        // the style mask, whose title bar removal unregisters KVO observers that a swizzled
        // class no longer matches (AppKit throws). Until then the panel is hidden and unfocused
        // (`show: false`, `focus: false`), so it can't become key.
        let deferred = win.clone();
        let hidden = view.clone();
        cx.spawn(async move |_| {
            deferred.setStyleMask(NSWindowStyleMask(NSWindowStyleMask::Borderless.0 | NONACTIVATING_PANEL));
            if let Err(e) = key_guard::install(&deferred) {
                // The overlay still works; it just isn't protected against becoming key.
                log::error!("overlay key guard not installed: {e}");
            }
            // At rest the layer is transparent, as a close leaves it (see `order_out`).
            without_implicit_animations(|| hidden.setAlphaValue(0.));
        })
        .detach();
        Ok(Self { win, view })
    }

    /// Shows the panel without activating the app or making the panel key: at once, fading in
    /// over `duration` as it slides up `slide` points into place. Part-way through a close, it
    /// turns back from where it is, in the share of `duration` that's left.
    pub fn show(&self, duration: Duration, slide: f64) {
        if let Some(layer) = self.view.layer() {
            let (opacity, offset) = if self.win.isVisible() { drawn(&layer) } else { (0., self.down() * slide) };
            let secs = duration.as_secs_f64() * (1. - opacity).clamp(0., 1.);
            // SAFETY: AppKit's constant timing function names, valid for the life of the process.
            let ease_out = unsafe { kCAMediaTimingFunctionEaseOut };
            without_implicit_animations(|| {
                self.view.setAlphaValue(1.);
                animate(&layer, FADE, "opacity", (opacity, 1.), secs, ease_out, false);
                animate(&layer, SLIDE, "transform.translation.y", (offset, 0.), secs, ease_out, false);
            });
        }
        self.win.orderFrontRegardless();
    }

    /// Starts the close: fades the panel out over `duration` as it slides `slide` points down,
    /// from wherever it is. It stays ordered in, held transparent, until [`order_out`]. Returns
    /// how long the fade takes: the share of `duration` that the opacity left calls for.
    ///
    /// [`order_out`]: Self::order_out
    pub fn fade_out(&self, duration: Duration, slide: f64) -> Duration {
        let Some(layer) = self.view.layer() else { return Duration::ZERO };
        let (opacity, offset) = drawn(&layer);
        let secs = duration.as_secs_f64() * opacity.clamp(0., 1.);
        if secs <= 0. {
            return Duration::ZERO;
        }
        // SAFETY: as in `show`.
        let ease_in = unsafe { kCAMediaTimingFunctionEaseIn };
        without_implicit_animations(|| {
            animate(&layer, FADE, "opacity", (opacity, 0.), secs, ease_in, true);
            animate(&layer, SLIDE, "transform.translation.y", (offset, self.down() * slide), secs, ease_in, true);
        });
        Duration::from_secs_f64(secs)
    }

    /// Takes the panel off screen, and leaves its layer transparent with nothing animating, so
    /// the next show fades in from nothing.
    pub fn order_out(&self) {
        self.win.orderOut(None);
        if let Some(layer) = self.view.layer() {
            without_implicit_animations(|| {
                self.view.setAlphaValue(0.);
                layer.removeAnimationForKey(&NSString::from_str(FADE));
                layer.removeAnimationForKey(&NSString::from_str(SLIDE));
            });
        }
    }

    /// Whether the panel is on screen (ordered in), closing included.
    pub fn is_visible(&self) -> bool {
        self.win.isVisible()
    }

    /// Down along the layer's y axis: AppKit's y goes up, unless the view GPUI's sits in is
    /// flipped.
    fn down(&self) -> f64 {
        // SAFETY: a plain getter, on the main thread like every overlay call.
        let flipped = unsafe { self.view.superview() }.is_some_and(|s| s.isFlipped());
        if flipped { 1. } else { -1. }
    }

    #[cfg(feature = "self-test")]
    pub fn state(&self) -> PanelState {
        let style = self.win.styleMask().0;
        PanelState {
            class: self.win.class().name().to_string_lossy().into_owned(),
            visible: self.win.isVisible(),
            key: self.win.isKeyWindow(),
            can_become_key: self.win.canBecomeKeyWindow(),
            ignores_mouse: self.win.ignoresMouseEvents(),
            borderless: style & !NONACTIVATING_PANEL == NSWindowStyleMask::Borderless.0,
            nonactivating: style & NONACTIVATING_PANEL != 0,
        }
    }

    /// The open and close as Core Animation has them now.
    #[cfg(feature = "self-test")]
    pub fn motion_state(&self) -> MotionState {
        let Some(layer) = self.view.layer() else { return MotionState::default() };
        let (opacity, offset) = drawn(&layer);
        let animation = |key: &str| {
            // SAFETY: only read for whether it exists, and its duration.
            unsafe { layer.animationForKey(&NSString::from_str(key)) }.map(|a| a.duration())
        };
        MotionState {
            opacity,
            offset_down: offset * self.down(),
            resting_opacity: self.view.alphaValue(),
            fade_secs: animation(FADE),
            slide_secs: animation(SLIDE),
        }
    }
}

/// The layer's opacity and slide (its y translation) as drawn now, part-way through an
/// animation or not.
fn drawn(layer: &CALayer) -> (f64, f64) {
    // SAFETY: `presentationLayer` returns a copy of this layer, of its own class; it's `nil`
    // until the layer has been committed once, and then the layer itself is what's drawn.
    let shown = unsafe { layer.presentationLayer() }.unwrap_or_else(|| layer.retain());
    (f64::from(shown.opacity()), shown.transform().m42)
}

/// Runs `f` in a Core Animation transaction with implicit animations off, so a change made there
/// only moves if an explicit animation moves it.
fn without_implicit_animations(f: impl FnOnce()) {
    CATransaction::begin();
    CATransaction::setDisableActions(true);
    f();
    CATransaction::commit();
}

/// Animates the layer's `path` from `from` to `to` over `secs`, under `key`, replacing what ran
/// there. `hold` keeps the end value until the animation is removed; otherwise the layer's own
/// value shows once it ends. Nothing to animate removes the key's animation.
fn animate(
    layer: &CALayer,
    key: &str,
    path: &str,
    (from, to): (f64, f64),
    secs: f64,
    timing: &CAMediaTimingFunctionName,
    hold: bool,
) {
    let key = NSString::from_str(key);
    if secs <= 0. || (from - to).abs() < 1e-3 {
        layer.removeAnimationForKey(&key);
        return;
    }
    let animation = CABasicAnimation::animationWithKeyPath(Some(&NSString::from_str(path)));
    let (from, to) = (NSNumber::new_f64(from), NSNumber::new_f64(to));
    let (from, to): (&AnyObject, &AnyObject) = (&from, &to);
    // SAFETY: an opacity and a translation both animate between NSNumbers.
    unsafe {
        animation.setFromValue(Some(from));
        animation.setToValue(Some(to));
    }
    animation.setDuration(secs);
    animation.setTimingFunction(Some(&CAMediaTimingFunction::functionWithName(timing)));
    if hold {
        // SAFETY: Core Animation's constant fill mode name.
        animation.setFillMode(unsafe { kCAFillModeForwards });
        animation.setRemovedOnCompletion(false);
    }
    layer.addAnimation_forKey(&animation, Some(&key));
}

/// The NSView GPUI draws a window into, through raw-window-handle.
fn ns_view_of(window: &gpui_kit::Window) -> Option<Retained<NSView>> {
    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else { return None };
    // SAFETY: GPUI hands out its own live content view, valid while `window` is borrowed.
    let view: &NSView = unsafe { appkit.ns_view.cast::<NSView>().as_ref() };
    Some(view.retain())
}

// ───────────────────────── Reduce motion ─────────────────────────

/// Whether macOS's Reduce motion is on (System Settings → Accessibility → Display). The overlay
/// reads it at each open, so a change applies from the next one.
pub fn reduce_motion() -> bool {
    #[cfg(feature = "self-test")]
    if let Some(forced) = self_test_reduce_motion::forced() {
        return forced;
    }
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

/// `GPUIPanel` answers YES to `canBecomeKeyWindow`. A non-activating panel that becomes key
/// takes keyboard focus without activating the app, so the ⌘V paste would land in the overlay
/// instead of the app the user is typing in. The guard isa-swizzles this one panel to a runtime
/// subclass whose `canBecomeKeyWindow` / `canBecomeMainWindow` answer NO. GPUI checks the class
/// with `isKindOfClass:`, so the subclass keeps working as a `GPUIPanel`.
mod key_guard {
    use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
    use objc2::sel;
    use objc2_app_kit::NSWindow;
    use std::sync::OnceLock;

    static GUARDED: OnceLock<&'static AnyClass> = OnceLock::new();

    unsafe extern "C" {
        fn object_setClass(obj: *mut AnyObject, cls: *const AnyClass) -> *const AnyClass;
    }

    extern "C-unwind" fn no(_this: &AnyObject, _cmd: Sel) -> Bool {
        Bool::NO
    }

    pub fn install(win: &NSWindow) -> Result<(), String> {
        let current: &'static AnyClass = win.class();
        if GUARDED.get().is_some_and(|g| std::ptr::eq(*g, current)) {
            return Ok(());
        }
        // Swizzling over anything else (a KVO subclass, a changed GPUI) could break it.
        if current.name() != c"GPUIPanel" {
            return Err(format!("expected GPUIPanel, found {:?}; not swizzling", current.name()));
        }
        let guarded = match GUARDED.get() {
            Some(g) => *g,
            None => {
                let mut builder = ClassBuilder::new(c"EasyVoxOverlayPanel", current)
                    .ok_or("the class name EasyVoxOverlayPanel is taken")?;
                // SAFETY: both methods match the `- (BOOL)sel` signature they override.
                unsafe {
                    builder.add_method(sel!(canBecomeKeyWindow), no as extern "C-unwind" fn(_, _) -> _);
                    builder.add_method(sel!(canBecomeMainWindow), no as extern "C-unwind" fn(_, _) -> _);
                }
                *GUARDED.get_or_init(|| builder.register())
            }
        };
        // SAFETY: the subclass adds methods only (no ivars), so the object layout is unchanged.
        unsafe { object_setClass(win as *const NSWindow as *mut AnyObject, guarded) };
        Ok(())
    }
}

// ───────────────────────── Hold right ⌥ (CGEventTap) ─────────────────────────

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightListenEventAccess() -> bool;
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: extern "C" fn(*mut c_void, u32, *mut c_void, *mut c_void) -> *mut c_void,
        user_info: *mut c_void,
    ) -> *mut c_void;
    fn CGEventTapEnable(tap: *mut c_void, enable: bool);
    fn CGEventGetFlags(event: *mut c_void) -> u64;
    fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFMachPortCreateRunLoopSource(alloc: *const c_void, port: *mut c_void, order: isize) -> *mut c_void;
    fn CFRunLoopGetMain() -> *mut c_void;
    fn CFRunLoopAddSource(rl: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRelease(cf: *const c_void);
    static kCFRunLoopCommonModes: *const c_void;
}

/// Input Monitoring, which the right-⌥ tap needs. Never prompts.
pub fn input_monitoring_granted() -> bool {
    // SAFETY: no arguments, no side effects.
    unsafe { CGPreflightListenEventAccess() }
}

/// Detects holding the right ⌥ key on its own, with a listen-only session event tap.
pub mod modifier_tap {
    use super::*;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

    static SINK: OnceLock<Box<dyn Fn(bool) + Send + Sync>> = OnceLock::new();
    static TAP: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
    static RIGHT_OPTION_DOWN: AtomicBool = AtomicBool::new(false);

    const KVK_RIGHT_OPTION: i64 = 61;
    const FIELD_KEYCODE: u32 = 9;
    const EVENT_FLAGS_CHANGED: u32 = 12;
    const EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
    const EVENT_TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;
    const FLAG_ALTERNATE: u64 = 1 << 19;

    extern "C" fn callback(_proxy: *mut c_void, ty: u32, event: *mut c_void, _info: *mut c_void) -> *mut c_void {
        match ty {
            // macOS switches a tap off if it's slow or on certain user input; switch it back on.
            EVENT_TAP_DISABLED_BY_TIMEOUT | EVENT_TAP_DISABLED_BY_USER_INPUT => {
                let tap = TAP.load(Ordering::Acquire);
                if !tap.is_null() {
                    // SAFETY: `tap` is the live mach port created in `install` and never freed.
                    unsafe { CGEventTapEnable(tap, true) };
                }
            }
            EVENT_FLAGS_CHANGED => {
                // SAFETY: `event` is the CGEvent this callback was handed.
                let code = unsafe { CGEventGetIntegerValueField(event, FIELD_KEYCODE) };
                if code == KVK_RIGHT_OPTION {
                    let down = unsafe { CGEventGetFlags(event) } & FLAG_ALTERNATE != 0;
                    if RIGHT_OPTION_DOWN.swap(down, Ordering::AcqRel) != down
                        && let Some(sink) = SINK.get()
                    {
                        sink(down);
                    }
                }
            }
            _ => {}
        }
        event
    }

    /// Installs the tap on the main run loop; `on_change(true)` on press, `false` on release.
    /// Installing twice is a no-op.
    pub fn install(on_change: impl Fn(bool) + Send + Sync + 'static) -> anyhow::Result<()> {
        if !TAP.load(Ordering::Acquire).is_null() {
            return Ok(());
        }
        // Check the permission first: CGEventTapCreate returns a working-looking tap even
        // without Input Monitoring; it just never sees other apps' keys.
        anyhow::ensure!(
            input_monitoring_granted(),
            "Input Monitoring is off. Allow EasyVox in System Settings → Privacy & Security → \
             Input Monitoring, then relaunch EasyVox."
        );
        let _ = SINK.set(Box::new(on_change));
        const SESSION_TAP: u32 = 1;
        const HEAD_INSERT: u32 = 0;
        const LISTEN_ONLY: u32 = 1;
        // SAFETY: plain CoreGraphics/CoreFoundation calls; the port lives for the whole process.
        unsafe {
            let port = CGEventTapCreate(
                SESSION_TAP,
                HEAD_INSERT,
                LISTEN_ONLY,
                1 << EVENT_FLAGS_CHANGED,
                callback,
                std::ptr::null_mut(),
            );
            anyhow::ensure!(!port.is_null(), "CGEventTapCreate returned NULL");
            let source = CFMachPortCreateRunLoopSource(std::ptr::null(), port, 0);
            anyhow::ensure!(!source.is_null(), "CFMachPortCreateRunLoopSource returned NULL");
            CFRunLoopAddSource(CFRunLoopGetMain(), source, kCFRunLoopCommonModes);
            CFRelease(source); // the run loop keeps its own reference
            TAP.store(port, Ordering::Release);
        }
        Ok(())
    }
}

// ───────────────────────── Self-test helpers ─────────────────────────

/// Lets the self-test give [`reduce_motion`] an answer instead of the system's.
#[cfg(feature = "self-test")]
mod self_test_reduce_motion {
    use std::sync::atomic::{AtomicU8, Ordering};

    /// 0: the system's answer; 1: off; 2: on.
    static FORCED: AtomicU8 = AtomicU8::new(0);

    pub fn forced() -> Option<bool> {
        match FORCED.load(Ordering::Relaxed) {
            0 => None,
            v => Some(v == 2),
        }
    }

    pub fn force(on: Option<bool>) {
        FORCED.store(on.map_or(0, |on| 1 + u8::from(on)), Ordering::Relaxed);
    }
}

/// Makes [`reduce_motion`] answer `Some(on)`, as if the system's setting were that, or follow the
/// system again (`None`). The system's setting is never touched.
#[cfg(feature = "self-test")]
pub fn force_reduce_motion(on: Option<bool>) {
    self_test_reduce_motion::force(on);
}

/// The overlay's open and close as Core Animation has them: what's drawn now (part-way through
/// an animation or not), the layer's resting opacity, and the animations on it.
#[cfg(feature = "self-test")]
#[derive(Debug, Default)]
pub struct MotionState {
    pub opacity: f64,
    /// How far below its place the pill is drawn, in points.
    pub offset_down: f64,
    pub resting_opacity: f64,
    /// The durations of the fade and the slide on the layer, if there are any.
    pub fade_secs: Option<f64>,
    pub slide_secs: Option<f64>,
}

/// What AppKit says about the overlay panel.
#[cfg(feature = "self-test")]
#[derive(Debug)]
pub struct PanelState {
    pub class: String,
    pub visible: bool,
    pub key: bool,
    pub can_become_key: bool,
    pub ignores_mouse: bool,
    /// No title bar, frame or controls: the style mask is `Borderless`, give or take the
    /// non-activating bit.
    pub borderless: bool,
    pub nonactivating: bool,
}

/// Gives EasyVox its own appearance (`Some(true)` dark, `Some(false)` light), as if the
/// system's had changed, or `None` to follow the system again. AppKit tells every window, so the
/// windows recolour through the same path as a real change. It never touches the system's
/// setting, and it lasts until the app quits. Main thread only.
#[cfg(feature = "self-test")]
pub fn set_app_appearance(dark: Option<bool>) -> Result<(), String> {
    use objc2_app_kit::{NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication};
    let mtm = objc2::MainThreadMarker::new().ok_or("not on the main thread")?;
    let appearance = dark.and_then(|dark| {
        // SAFETY: AppKit's constant appearance names, valid for the life of the process.
        let name = unsafe { if dark { NSAppearanceNameDarkAqua } else { NSAppearanceNameAqua } };
        NSAppearance::appearanceNamed(name)
    });
    NSApplication::sharedApplication(mtm).setAppearance(appearance.as_deref());
    Ok(())
}

/// Whether the menu bar icon's status item is visible.
#[cfg(feature = "self-test")]
pub fn status_item_visible(icon: &tray_icon::TrayIcon) -> Option<bool> {
    icon.ns_status_item().map(|item| item.isVisible())
}

/// The general pasteboard's change count. Metadata only: it never reads the clipboard, so it
/// can't trigger macOS's paste-privacy prompt.
#[cfg(feature = "self-test")]
pub fn clipboard_change_count() -> isize {
    objc2_app_kit::NSPasteboard::generalPasteboard().changeCount()
}

/// Fires a tray menu item's AppKit action, as a click would, so the real muda → GPUI path runs.
#[cfg(feature = "self-test")]
pub fn perform_menu_item(menu: &tray_icon::menu::Menu, title: &str) -> Result<(), String> {
    use objc2_app_kit::NSMenu;
    use objc2_foundation::NSString;
    use tray_icon::menu::ContextMenu as _;
    // SAFETY: muda's `ns_menu` is the NSMenu it owns, alive as long as `menu`.
    let ns_menu: &NSMenu = unsafe { &*(menu.ns_menu() as *const NSMenu) };
    let ix = ns_menu.indexOfItemWithTitle(&NSString::from_str(title));
    if ix < 0 {
        return Err(format!("no menu item titled {title:?}"));
    }
    ns_menu.performActionForItemAtIndex(ix);
    Ok(())
}

/// Posts a Carbon hot-key press or release for our registered hot key to the main event queue,
/// the way the OS delivers a real one. It runs global-hotkey's handler → our channel → the
/// controller, but not the OS's own key matching. Returns the Carbon status (0 = ok).
#[cfg(feature = "self-test")]
pub fn post_hotkey_event(hotkey_id: u32, pressed: bool) -> i32 {
    #[repr(C)]
    struct EventHotKeyId {
        signature: u32,
        id: u32,
    }

    #[link(name = "Carbon", kind = "framework")]
    unsafe extern "C" {
        fn CreateEvent(
            alloc: *const c_void,
            class: u32,
            kind: u32,
            when: f64,
            flags: u32,
            out: *mut *mut c_void,
        ) -> i32;
        fn SetEventParameter(ev: *mut c_void, name: u32, ty: u32, size: usize, data: *const c_void) -> i32;
        fn GetApplicationEventTarget() -> *mut c_void;
        fn GetMainEventQueue() -> *mut c_void;
        fn PostEventToQueue(queue: *mut c_void, ev: *mut c_void, priority: i16) -> i32;
        fn ReleaseEvent(ev: *mut c_void);
    }

    const fn fourcc(s: &[u8; 4]) -> u32 {
        u32::from_be_bytes(*s)
    }
    const KIND_PRESSED: u32 = 5;
    const KIND_RELEASED: u32 = 6;

    // SAFETY: standard Carbon event calls; the event is released after posting.
    unsafe {
        let mut ev = std::ptr::null_mut();
        let kind = if pressed { KIND_PRESSED } else { KIND_RELEASED };
        let status = CreateEvent(std::ptr::null(), fourcc(b"keyb"), kind, 0.0, 0, &mut ev);
        if status != 0 {
            return status;
        }
        // global-hotkey registers its hot keys with the signature 'htrs'.
        let hk = EventHotKeyId { signature: fourcc(b"htrs"), id: hotkey_id };
        let size = std::mem::size_of::<EventHotKeyId>();
        SetEventParameter(ev, fourcc(b"----"), fourcc(b"hkid"), size, &hk as *const _ as *const c_void);
        let target = GetApplicationEventTarget();
        let size = std::mem::size_of::<*mut c_void>();
        SetEventParameter(ev, fourcc(b"ptrg"), fourcc(b"etrg"), size, &target as *const _ as *const c_void);
        let status = PostEventToQueue(GetMainEventQueue(), ev, 1);
        ReleaseEvent(ev);
        status
    }
}
