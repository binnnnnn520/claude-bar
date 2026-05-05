use crate::scanner;
use crate::scanner::types::UsageSnapshot;
use tauri::AppHandle;

#[tauri::command]
pub async fn scan_usage(window: String) -> Result<UsageSnapshot, String> {
    let normalized = normalize_usage_window(&window)?;

    tauri::async_runtime::spawn_blocking(move || scanner::scan_usage(&normalized))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

fn normalize_usage_window(window: &str) -> Result<String, String> {
    match window {
        "last30d" | "month" | "today" => Ok(window.to_string()),
        other => Err(format!("Unsupported usage window: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_usage_window;

    #[test]
    fn normalize_usage_window_accepts_supported_windows() {
        for window in ["last30d", "month", "today"] {
            assert_eq!(normalize_usage_window(window), Ok(window.to_string()));
        }
    }

    #[test]
    fn normalize_usage_window_rejects_unsupported_windows() {
        assert_eq!(
            normalize_usage_window("week"),
            Err("Unsupported usage window: week".to_string())
        );
    }
}
