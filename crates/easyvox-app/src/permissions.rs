//! The macOS permissions as the system answers now, and the watch the setup and Settings windows
//! keep on them.
//!
//! A permission granted or turned off in System Settings reaches the app through nothing but a
//! fresh read, so a window that shows one reads it again: when the window opens, whenever it
//! comes forward (back from System Settings), and every [`POLL`] while one it shows is missing.
//! The timer stops once they're all allowed, and goes with the window: nothing runs while
//! neither window is open. When a read finds a change, the window redraws and the controller
//! hears of it, so the menu bar item catches up and right ⌥ starts working once Input
//! Monitoring arrives (`Controller::recheck_permissions`).

use crate::controller;
use crate::platform::{self, InputMonitoring};
use easyvox_core::MicPermission;
use gpui_kit::*;
use std::fmt::Debug;
use std::time::Duration;

/// How often a window reads the permissions again while one it shows is missing.
pub const POLL: Duration = Duration::from_secs(1);

/// What the system says about the two permissions onboarding asks for, which the menu bar item
/// shows too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Permissions {
    pub mic: MicPermission,
    pub accessibility: bool,
}

impl Permissions {
    /// Never prompts.
    pub fn read() -> Self {
        Self { mic: easyvox_core::mic_permission_status(), accessibility: easyvox_core::accessibility_trusted() }
    }

    pub fn all_allowed(self) -> bool {
        self.mic == MicPermission::Authorized && self.accessibility
    }
}

/// All three permissions EasyVox uses, as Settings › Permissions shows them: onboarding's two,
/// and Input Monitoring, only for holding right ⌥.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllPermissions {
    pub mic: MicPermission,
    pub accessibility: bool,
    pub input_monitoring: InputMonitoring,
}

impl AllPermissions {
    /// Never prompts.
    pub fn read() -> Self {
        let Permissions { mic, accessibility } = Permissions::read();
        Self { mic, accessibility, input_monitoring: platform::input_monitoring() }
    }

    pub fn all_allowed(self) -> bool {
        Permissions { mic: self.mic, accessibility: self.accessibility }.all_allowed()
            && self.input_monitoring == InputMonitoring::Allowed
    }
}

/// A window's watch on the permissions it shows (`P`, read by `read`): owned by its view, and
/// dropped with it. `missing` says whether one is still missing, so it keeps reading.
pub struct Watch<P> {
    seen: P,
    read: fn() -> P,
    missing: fn(P) -> bool,
    /// Reads them again every [`POLL`] while one is missing.
    poll: Option<Task<()>>,
    /// Self-test only: permissions to show instead of the system's, which it never changes.
    #[cfg(feature = "self-test")]
    pinned: Option<P>,
    /// Self-test only: how many times the system was asked.
    #[cfg(feature = "self-test")]
    reads: usize,
    _activation: Subscription,
}

impl<P: Copy + PartialEq + Debug + 'static> Watch<P> {
    /// Reads the permissions, and keeps reading them for `window` while one is missing. `field`
    /// finds this watch in the window's view.
    pub fn new<V: 'static>(
        window: &mut Window,
        cx: &mut Context<V>,
        field: fn(&mut V) -> &mut Self,
        read: fn() -> P,
        missing: fn(P) -> bool,
    ) -> Self {
        // Back from System Settings: read them now, and watch again if one is missing.
        let activation = cx.observe_window_activation(window, move |view, window, cx| {
            if window.is_window_active() {
                Self::restart(view, field, cx);
            }
        });
        let seen = read();
        let poll = missing(seen).then(|| Self::poll(field, cx));
        Self {
            seen,
            read,
            missing,
            poll,
            #[cfg(feature = "self-test")]
            pinned: None,
            #[cfg(feature = "self-test")]
            reads: 1,
            _activation: activation,
        }
    }

    /// The permissions as last read.
    pub fn get(&self) -> P {
        #[cfg(feature = "self-test")]
        if let Some(pinned) = self.pinned {
            return pinned;
        }
        self.seen
    }

    /// Reads them now, then every [`POLL`] while one is missing. Replaces any earlier timer.
    pub fn restart<V: 'static>(view: &mut V, field: fn(&mut V) -> &mut Self, cx: &mut Context<V>) {
        let missing = field(view).check(cx);
        let poll = missing.then(|| Self::poll(field, cx));
        field(view).poll = poll;
    }

    fn poll<V: 'static>(field: fn(&mut V) -> &mut Self, cx: &mut Context<V>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                match this.update(cx, |view, cx| field(view).check(cx)) {
                    Ok(true) => {}
                    _ => break, // all allowed, or the window is gone
                }
            }
        })
    }

    /// Reads them; on a change, redraws the window and tells the controller. True while one is
    /// still missing.
    fn check<V: 'static>(&mut self, cx: &mut Context<V>) -> bool {
        let now = (self.read)();
        #[cfg(feature = "self-test")]
        {
            self.reads += 1;
        }
        if now != self.seen {
            log::info!("permissions: now {now:?}");
            self.seen = now;
            cx.notify();
            // After this update: the controller may be the one updating the window now.
            cx.defer(|cx| {
                controller::with(cx, |c, cx| c.recheck_permissions(cx));
            });
        }
        (self.missing)(self.get())
    }

    /// Shows `pinned` instead of the system's answers (`None`: the system's again), and watches
    /// as if they were the system's.
    #[cfg(feature = "self-test")]
    pub fn pin<V: 'static>(view: &mut V, field: fn(&mut V) -> &mut Self, pinned: Option<P>, cx: &mut Context<V>) {
        field(view).pinned = pinned;
        Self::restart(view, field, cx);
        cx.notify();
    }

    /// How many times this watch asked the system.
    #[cfg(feature = "self-test")]
    pub fn reads(&self) -> usize {
        self.reads
    }
}
