use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime, Size, WebviewWindow};

const COMPACT_WIDTH: u32 = 360;
const COMPACT_HEIGHT: u32 = 244;
const DASHBOARD_WIDTH: u32 = 920;
const DASHBOARD_HEIGHT: u32 = 720;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelMode {
    Compact,
    Dashboard,
}

impl PanelMode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "compact" => Ok(Self::Compact),
            "dashboard" => Ok(Self::Dashboard),
            other => Err(format!("Unsupported panel mode: {other}")),
        }
    }

    fn size(self) -> (u32, u32) {
        match self {
            Self::Compact => (COMPACT_WIDTH, COMPACT_HEIGHT),
            Self::Dashboard => (DASHBOARD_WIDTH, DASHBOARD_HEIGHT),
        }
    }
}

#[derive(Clone, Copy)]
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
                position,
                ..
            } = event
            {
                toggle_main_window(&handle, position.x as i32, position.y as i32);
            }
        })
        .build(app)?;

    if let Some(window) = app.get_webview_window("main") {
        show_panel_mode(&window, PanelMode::Compact, None)?;
    }

    Ok(())
}

fn toggle_main_window<R: Runtime>(app: &AppHandle<R>, x: i32, y: i32) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    let _ = show_panel_mode(&window, PanelMode::Compact, Some((x, y)));
}

pub fn set_panel_mode<R: Runtime>(window: &WebviewWindow<R>, mode: PanelMode) -> tauri::Result<()> {
    let (width, height) = mode.size();
    window.set_size(Size::Physical(PhysicalSize::new(width, height)))?;
    position_panel(window, width as i32, height as i32, None)
}

fn show_panel_mode<R: Runtime>(
    window: &WebviewWindow<R>,
    mode: PanelMode,
    anchor: Option<(i32, i32)>,
) -> tauri::Result<()> {
    let (width, height) = mode.size();
    window.set_size(Size::Physical(PhysicalSize::new(width, height)))?;
    position_panel(window, width as i32, height as i32, anchor)?;

    window.show()?;
    window.set_focus()?;
    Ok(())
}

fn position_panel<R: Runtime>(
    window: &WebviewWindow<R>,
    window_width: i32,
    window_height: i32,
    anchor: Option<(i32, i32)>,
) -> tauri::Result<()> {
    let bounds = anchor
        .and_then(|(x, y)| monitor_bounds_for_point(window, x, y))
        .or_else(|| monitor_bounds_for_window(window));
    let (target_x, target_y) = popover_position(0, 0, window_width, window_height, bounds);

    window.set_position(PhysicalPosition::new(target_x, target_y))
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

fn popover_position(
    x: i32,
    y: i32,
    window_width: i32,
    window_height: i32,
    bounds: Option<MonitorBounds>,
) -> (i32, i32) {
    let Some(bounds) = bounds else {
        return (
            (x - window_width).max(0),
            (y - window_height).max(0),
        );
    };

    let max_y = (bounds.y + bounds.height - window_height).max(bounds.y);
    let target_x = (bounds.x + bounds.width - window_width).max(bounds.x);
    let target_y = max_y;

    (target_x, target_y)
}

#[cfg(test)]
mod tests {
    use super::{popover_position, MonitorBounds};

    const PRIMARY: MonitorBounds = MonitorBounds {
        x: 0,
        y: 0,
        width: 1280,
        height: 960,
    };

    #[test]
    fn anchors_window_to_monitor_bottom_right() {
        assert_eq!(popover_position(1200, 900, 360, 244, Some(PRIMARY)), (920, 716));
    }

    #[test]
    fn ignores_floating_tray_popup_click_position() {
        assert_eq!(popover_position(100, 400, 360, 244, Some(PRIMARY)), (920, 716));
    }

    #[test]
    fn anchors_to_right_edge_of_current_monitor() {
        let secondary = MonitorBounds {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
        };
        assert_eq!(popover_position(-200, 900, 920, 720, Some(secondary)), (-920, 360));
    }

    #[test]
    fn keeps_legacy_origin_clamp_without_monitor_data() {
        assert_eq!(popover_position(100, 400, 820, 720, None), (0, 0));
    }
}
