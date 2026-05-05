use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, PhysicalPosition, Runtime};

const WINDOW_WIDTH: i32 = 386;
const WINDOW_HEIGHT: i32 = 520;
const WINDOW_X_OFFSET: i32 = 28;
const WINDOW_Y_OFFSET: i32 = 18;

pub fn setup<R: Runtime>(app: &mut App<R>) -> tauri::Result<()> {
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&quit])?;
    let quit_id = quit.id().clone();
    let handle = app.handle().clone();

    TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Token Ledger")
        .on_menu_event(move |app, event| {
            if event.id == quit_id {
                app.exit(0);
            }
        })
        .on_tray_icon_event(move |_tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            {
                toggle_main_window(&handle, position.x as i32, position.y as i32);
            }
        })
        .build(app)?;

    Ok(())
}

fn toggle_main_window<R: Runtime>(app: &AppHandle<R>, x: i32, y: i32) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }

    let (target_x, target_y) = popover_position(x, y);

    let _ = window.set_position(PhysicalPosition::new(target_x, target_y));
    let _ = window.show();
    let _ = window.set_focus();
}

fn popover_position(x: i32, y: i32) -> (i32, i32) {
    (
        (x - WINDOW_WIDTH + WINDOW_X_OFFSET).max(0),
        (y - WINDOW_HEIGHT - WINDOW_Y_OFFSET).max(0),
    )
}

#[cfg(test)]
mod tests {
    use super::popover_position;

    #[test]
    fn positions_window_above_and_left_of_tray_click() {
        assert_eq!(popover_position(1200, 900), (842, 362));
    }

    #[test]
    fn clamps_window_position_to_screen_origin() {
        assert_eq!(popover_position(100, 400), (0, 0));
    }
}
