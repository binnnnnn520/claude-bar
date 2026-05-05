use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime, Size, WebviewWindow, WindowEvent};

const PANEL_WIDTH: u32 = 820;
const PANEL_HEIGHT: u32 = 680;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SavedWindowPosition {
    x: i32,
    y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MonitorBounds {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

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
                ..
            } = event
            {
                toggle_main_window(&handle);
            }
        })
        .build(app)?;

    if let Some(window) = app.get_webview_window("main") {
        let path = saved_position_path(app.handle());
        let saved = path.as_deref().and_then(load_saved_position);
        show_panel(&window, saved)?;
        remember_window_position(&window, path);
    }

    Ok(())
}

fn toggle_main_window<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }

    let path = saved_position_path(app);
    let saved = path.as_deref().and_then(load_saved_position);
    let _ = show_panel(&window, saved);
}

fn show_panel<R: Runtime>(
    window: &WebviewWindow<R>,
    saved: Option<SavedWindowPosition>,
) -> tauri::Result<()> {
    window.set_size(Size::Physical(PhysicalSize::new(PANEL_WIDTH, PANEL_HEIGHT)))?;
    position_panel(window, PANEL_WIDTH as i32, PANEL_HEIGHT as i32, saved)?;
    window.show()?;
    window.set_focus()?;
    Ok(())
}

fn position_panel<R: Runtime>(
    window: &WebviewWindow<R>,
    window_width: i32,
    window_height: i32,
    saved: Option<SavedWindowPosition>,
) -> tauri::Result<()> {
    let bounds = saved
        .and_then(|position| monitor_bounds_for_point(window, position.x, position.y).map(|bounds| (position, bounds)))
        .map(|(position, bounds)| (Some(position), Some(bounds)))
        .unwrap_or_else(|| (saved, monitor_bounds_for_window(window)));
    let (target_x, target_y) = panel_position(bounds.0, window_width, window_height, bounds.1);

    window.set_position(PhysicalPosition::new(target_x, target_y))
}

fn remember_window_position<R: Runtime>(window: &WebviewWindow<R>, path: Option<PathBuf>) {
    let Some(path) = path else {
        return;
    };

    window.on_window_event(move |event| {
        if let WindowEvent::Moved(position) = event {
            let _ = save_window_position(
                &path,
                SavedWindowPosition {
                    x: position.x,
                    y: position.y,
                },
            );
        }
    });
}

fn monitor_bounds_for_point<R: Runtime>(window: &WebviewWindow<R>, x: i32, y: i32) -> Option<MonitorBounds> {
    let monitors = window.available_monitors().ok()?;
    let monitor = monitors
        .iter()
        .find(|item| {
            let position = item.position();
            let size = item.size();
            let right = position.x + size.width as i32;
            let bottom = position.y + size.height as i32;
            x >= position.x && x <= right && y >= position.y && y <= bottom
        })
        .or_else(|| monitors.first())?;
    let position = monitor.position();
    let size = monitor.size();

    Some(MonitorBounds {
        x: position.x,
        y: position.y,
        width: size.width as i32,
        height: size.height as i32,
    })
}

fn monitor_bounds_for_window<R: Runtime>(window: &WebviewWindow<R>) -> Option<MonitorBounds> {
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())?;
    let position = monitor.position();
    let size = monitor.size();

    Some(MonitorBounds {
        x: position.x,
        y: position.y,
        width: size.width as i32,
        height: size.height as i32,
    })
}

fn panel_position(
    saved: Option<SavedWindowPosition>,
    window_width: i32,
    window_height: i32,
    bounds: Option<MonitorBounds>,
) -> (i32, i32) {
    let Some(bounds) = bounds else {
        let Some(saved) = saved else {
            return (0, 0);
        };
        return (saved.x.max(0), saved.y.max(0));
    };

    let max_x = (bounds.x + bounds.width - window_width).max(bounds.x);
    let max_y = (bounds.y + bounds.height - window_height).max(bounds.y);
    let Some(saved) = saved else {
        return (
            bounds.x + ((bounds.width - window_width).max(0) / 2),
            bounds.y + ((bounds.height - window_height).max(0) / 2),
        );
    };

    (saved.x.clamp(bounds.x, max_x), saved.y.clamp(bounds.y, max_y))
}

fn saved_position_path<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|path| path.join("window-position.json"))
}

fn load_saved_position(path: &Path) -> Option<SavedWindowPosition> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_window_position(path: &Path, position: SavedWindowPosition) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string(&position).unwrap_or_else(|_| "{}".to_string());
    fs::write(path, content)
}

#[cfg(test)]
mod tests {
    use super::{panel_position, SavedWindowPosition, MonitorBounds};

    const PRIMARY: MonitorBounds = MonitorBounds {
        x: 0,
        y: 0,
        width: 1280,
        height: 960,
    };

    #[test]
    fn restores_user_dragged_position_inside_monitor() {
        let saved = SavedWindowPosition { x: 318, y: 142 };
        assert_eq!(panel_position(Some(saved), 760, 640, Some(PRIMARY)), (318, 142));
    }

    #[test]
    fn clamps_saved_position_to_visible_monitor_area() {
        let saved = SavedWindowPosition { x: 1100, y: 900 };
        assert_eq!(panel_position(Some(saved), 760, 640, Some(PRIMARY)), (520, 320));
    }

    #[test]
    fn centers_when_no_saved_position_exists() {
        assert_eq!(panel_position(None, 760, 640, Some(PRIMARY)), (260, 160));
    }

    #[test]
    fn restores_position_on_current_monitor_with_negative_origin() {
        let secondary = MonitorBounds {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let saved = SavedWindowPosition { x: -1500, y: 240 };
        assert_eq!(panel_position(Some(saved), 760, 640, Some(secondary)), (-1500, 240));
    }

    #[test]
    fn keeps_legacy_origin_clamp_without_monitor_data() {
        let saved = SavedWindowPosition { x: -20, y: 50 };
        assert_eq!(panel_position(Some(saved), 760, 640, None), (0, 50));
    }
}
