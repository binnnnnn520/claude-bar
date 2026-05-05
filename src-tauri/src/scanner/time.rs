use chrono::{DateTime, Duration, Local, NaiveDate, Utc};
use serde_json::Value;
use std::path::Path;

pub fn cutoff_last_30_days(now: DateTime<Utc>) -> DateTime<Utc> {
    now - Duration::days(30)
}

pub fn parse_timestamp(value: &Value) -> Option<DateTime<Utc>> {
    for key in ["timestamp", "ts", "time", "created_at", "createdAt"] {
        if let Some(raw) = value.get(key).and_then(Value::as_str) {
            if let Ok(ts) = DateTime::parse_from_rfc3339(raw) {
                return Some(ts.with_timezone(&Utc));
            }
        }
    }
    None
}

pub fn date_key(ts: DateTime<Utc>) -> String {
    ts.with_timezone(&Local).format("%Y-%m-%d").to_string()
}

pub fn codex_date_from_path(path: &Path) -> Option<String> {
    let parts: Vec<String> = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_string())
        .collect();

    for window in parts.windows(3) {
        let year = &window[0];
        let month = &window[1];
        let day = &window[2];
        if year.len() == 4
            && month.len() == 2
            && day.len() == 2
            && year.chars().all(|c| c.is_ascii_digit())
            && month.chars().all(|c| c.is_ascii_digit())
            && day.chars().all(|c| c.is_ascii_digit())
        {
            let key = format!("{year}-{month}-{day}");
            if NaiveDate::parse_from_str(&key, "%Y-%m-%d").is_ok() {
                return Some(key);
            }
        }
    }

    None
}
