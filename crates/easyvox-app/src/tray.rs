//! The menu bar icon and its menu, through the `tray-icon` crate (Tauri team, used without
//! Tauri). Menu callbacks run on the main thread from AppKit; `main.rs` forwards them into GPUI
//! through the app's event channel.

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

/// Debug builds only: replays a test clip as if it were spoken.
#[cfg(debug_assertions)]
pub const MENU_TEST_DICTATION: &str = "test-dictation";
pub const MENU_SETTINGS: &str = "settings";
pub const MENU_HISTORY: &str = "history";
pub const MENU_QUIT: &str = "quit";
pub const SETTINGS_TITLE: &str = "Settings…";
pub const HISTORY_TITLE: &str = "History…";
pub const QUIT_TITLE: &str = "Quit EasyVox";

/// Dropping it removes the menu bar icon. The fields are read only by the self-test.
#[cfg_attr(not(feature = "self-test"), allow(dead_code))]
pub struct Tray {
    pub icon: TrayIcon,
    pub menu: Menu,
}

pub fn create(on_menu: impl Fn(String) + Send + Sync + 'static) -> anyhow::Result<Tray> {
    let menu = Menu::new();
    #[cfg(debug_assertions)]
    menu.append_items(&[
        &MenuItem::with_id(MENU_TEST_DICTATION, "Test dictation (WAV replay)", true, None),
        &PredefinedMenuItem::separator(),
    ])?;
    menu.append_items(&[
        &MenuItem::with_id(MENU_SETTINGS, SETTINGS_TITLE, true, None),
        &MenuItem::with_id(MENU_HISTORY, HISTORY_TITLE, true, None),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id(MENU_QUIT, QUIT_TITLE, true, None),
    ])?;
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| on_menu(e.id.0)));

    let icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu.clone()))
        .with_icon_templated(mic_icon())
        .with_tooltip("EasyVox")
        .build()?;
    Ok(Tray { icon, menu })
}

/// A 36×36 (18 pt @2x) template microphone, drawn in code so there's no asset to ship. As a
/// template image, macOS tints it for light and dark menu bars.
fn mic_icon() -> Icon {
    const N: usize = 36;
    let mut rgba = vec![0u8; N * N * 4];
    for y in 0..N {
        for x in 0..N {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let dx = (fx - 18.0).abs();
            // Capsule: x in [12.5, 23.5], y in [3, 22], radius 5.5.
            let cy = fy.clamp(8.5, 16.5);
            let in_capsule = (dx.powi(2) + (fy - cy).powi(2)).sqrt() <= 5.5;
            // A U-shaped arc around the capsule's lower half, with straight sides above it.
            let d = (dx.powi(2) + (fy - 16.0).powi(2)).sqrt();
            let in_arc = fy >= 16.0 && (8.5..=10.5).contains(&d);
            let in_arc_sides = (12.0..16.0).contains(&fy) && (8.5..=9.5).contains(&dx);
            let in_stem = (17.0..=19.0).contains(&fx) && (26.0..=31.0).contains(&fy);
            let in_base = (12.0..=24.0).contains(&fx) && (30.5..=32.5).contains(&fy);
            if in_capsule || in_arc || in_arc_sides || in_stem || in_base {
                let i = (y * N + x) * 4;
                rgba[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
    }
    Icon::from_rgba(rgba, N as u32, N as u32).expect("a 36×36 RGBA buffer is a valid icon")
}
