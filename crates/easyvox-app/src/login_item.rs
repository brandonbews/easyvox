//! Open at login: the switch in Settings › General and at the end of setup. It's macOS's own
//! login item for the app (`platform::login_item`), not a key in the settings file, since the
//! user can also remove it in System Settings › General › Login Items & Extensions. So the switch
//! shows what macOS says, read whenever a window that shows it opens or comes forward, and again
//! after the switch is used. Off by default: nothing registers it unless the switch is turned on.

use crate::brand::{self, Colors};
use crate::platform::{self, LoginItem};
use crate::state::AppStatus;
use gpui_kit::component::switch::Switch;
use gpui_kit::*;

/// Where System Settings lists it.
const WHERE: &str = "System Settings › General › Login Items & Extensions";

/// How many times the row was drawn: the self-test checks where it appears.
#[cfg(feature = "self-test")]
pub static DRAWN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Reads whether EasyVox opens at login into `AppStatus`, and forgets the switch's last error:
/// what macOS says now is what the switch shows.
pub fn read(cx: &mut App) {
    let now = platform::login_item();
    let status = cx.global::<AppStatus>();
    if status.login_item != now || status.login_item_error.is_some() {
        log::info!("login item: {now:?}");
        let status = cx.global_mut::<AppStatus>();
        status.login_item = now;
        status.login_item_error = None;
        cx.refresh_windows();
    }
}

/// Reads it in `window`'s view now, and whenever the window comes forward (back from System
/// Settings, say). Keep the subscription for as long as the view.
pub fn watch<V: 'static>(window: &mut Window, cx: &mut Context<V>) -> Subscription {
    read(cx);
    cx.observe_window_activation(window, |_, window, cx| {
        if window.is_window_active() {
            read(cx);
        }
    })
}

/// What the switch does: registers EasyVox as a login item (`on`) or unregisters it, unless
/// macOS has it that way already, then reads it back. A refusal is kept for the row to show.
pub fn set(on: bool, cx: &mut App) {
    let before = platform::login_item();
    let result = if (before != LoginItem::Off) == on {
        Ok(())
    } else {
        log::info!("login item: {} (was {before:?})", if on { "registering" } else { "unregistering" });
        platform::set_login_item(on)
    };
    let after = platform::login_item();
    log::info!("login item: now {after:?}");
    let status = cx.global_mut::<AppStatus>();
    status.login_item = after;
    status.login_item_error = refusal(on, result, after);
    cx.refresh_windows();
}

/// Why the switch couldn't do what it was asked, in one line, if it couldn't: macOS refused,
/// with its reason, or said yes and still has it the other way. Nothing when it ended up as
/// asked, whatever macOS answered: a register that waits for approval errs, and the approval
/// line says what to do.
fn refusal(on: bool, result: Result<(), String>, after: LoginItem) -> Option<String> {
    if (after != LoginItem::Off) == on {
        return None;
    }
    let what = if on { "on" } else { "off" };
    match result {
        Err(reason) => {
            let reason = reason.trim();
            let end = if reason.ends_with(['.', '!', '?']) { "" } else { "." };
            Some(format!("Couldn't turn Open at login {what}. {reason}{end}"))
        }
        Ok(()) => {
            let lists = if on { "doesn't list" } else { "still lists" };
            Some(format!("Couldn't turn Open at login {what}. macOS {lists} EasyVox in its login items."))
        }
    }
}

/// What the row says under its title instead of its own line, if anything: the switch's last
/// error, or that macOS waits for the user's approval.
pub fn problem(login_item: LoginItem, error: Option<&str>) -> Option<String> {
    match (error, login_item) {
        (Some(error), _) => Some(error.into()),
        (None, LoginItem::NeedsApproval) => Some(format!("Waiting for your approval in {WHERE}.")),
        (None, LoginItem::On | LoginItem::Off) => None,
    }
}

/// The Open at login row: its title, `line` (or the problem, in its place), and the switch, on
/// while EasyVox is registered, approved or not.
pub fn row(c: &'static Colors, line: Option<&'static str>, cx: &App) -> Div {
    #[cfg(feature = "self-test")]
    DRAWN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let status = cx.global::<AppStatus>();
    let line = match problem(status.login_item, status.login_item_error.as_deref()) {
        Some(problem) => Some(brand::problem(c, problem, brand::TEXT_SMALL).into_any_element()),
        None => line.map(|line| brand::line(c, line)),
    };
    let switch = Switch::new("open-at-login")
        .checked(status.login_item != LoginItem::Off)
        .on_click(|on: &bool, _, cx| set(*on, cx));
    brand::row("Open at login", line, Some(switch.into_any_element()))
}

#[cfg(test)]
mod tests {
    use super::{LoginItem, problem, refusal};

    #[test]
    fn the_row_says_nothing_more_while_macos_does_what_the_switch_shows() {
        assert_eq!(problem(LoginItem::Off, None), None);
        assert_eq!(problem(LoginItem::On, None), None);
    }

    #[test]
    fn waiting_for_approval_says_where_to_give_it() {
        let words = problem(LoginItem::NeedsApproval, None).unwrap_or_default();
        assert_eq!(words, "Waiting for your approval in System Settings › General › Login Items & Extensions.");
    }

    #[test]
    fn a_refusal_shows_until_the_next_read_whatever_macos_says() {
        for login_item in [LoginItem::Off, LoginItem::On, LoginItem::NeedsApproval] {
            assert_eq!(problem(login_item, Some("Couldn't.")), Some("Couldn't.".into()), "{login_item:?}");
        }
    }

    #[test]
    fn a_refusal_says_what_didnt_happen_and_macos_reason() {
        assert_eq!(
            refusal(true, Err("Operation not permitted".into()), LoginItem::Off).as_deref(),
            Some("Couldn't turn Open at login on. Operation not permitted.")
        );
        assert_eq!(
            refusal(false, Err(" Invalid signature. ".into()), LoginItem::On).as_deref(),
            Some("Couldn't turn Open at login off. Invalid signature.")
        );
    }

    #[test]
    fn an_error_that_still_ended_up_as_asked_says_nothing_more() {
        // Registering an item the user hasn't approved errs, and leaves it waiting for approval,
        // which the approval line explains.
        assert_eq!(refusal(true, Err("Launch denied by user.".into()), LoginItem::NeedsApproval), None);
        assert_eq!(refusal(false, Err("Job not found.".into()), LoginItem::Off), None);
    }

    #[test]
    fn a_yes_that_changed_nothing_isnt_silent() {
        assert_eq!(
            refusal(true, Ok(()), LoginItem::Off).as_deref(),
            Some("Couldn't turn Open at login on. macOS doesn't list EasyVox in its login items.")
        );
        for still in [LoginItem::On, LoginItem::NeedsApproval] {
            assert_eq!(
                refusal(false, Ok(()), still).as_deref(),
                Some("Couldn't turn Open at login off. macOS still lists EasyVox in its login items."),
                "{still:?}"
            );
        }
    }

    #[test]
    fn a_yes_that_took_says_nothing_more() {
        // Waiting for approval has its own line (`problem`).
        for after in [LoginItem::On, LoginItem::NeedsApproval] {
            assert_eq!(refusal(true, Ok(()), after), None, "{after:?}");
        }
        assert_eq!(refusal(false, Ok(()), LoginItem::Off), None);
    }
}
