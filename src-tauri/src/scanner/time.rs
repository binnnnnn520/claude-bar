use chrono::{
    DateTime, Datelike, Duration, FixedOffset, Local, LocalResult, NaiveDate, Offset, TimeZone, Utc,
};
use serde_json::Value;
use std::path::Path;

pub fn cutoff_last_30_days(now: DateTime<Utc>) -> DateTime<Utc> {
    now - Duration::days(30)
}

pub fn usage_window_cutoff(window: &str, now: DateTime<Local>) -> Option<DateTime<Utc>> {
    match window {
        "last30d" => Some(cutoff_last_30_days(now.with_timezone(&Utc))),
        "month" => {
            let first_day = NaiveDate::from_ymd_opt(now.year(), now.month(), 1)?;
            Some(local_date_start_to_utc(first_day, now.offset().fix()))
        }
        "today" => Some(local_date_start_to_utc(
            now.date_naive(),
            now.offset().fix(),
        )),
        _ => None,
    }
}

fn local_date_start_to_utc(date: NaiveDate, fallback_offset: FixedOffset) -> DateTime<Utc> {
    let midnight = date
        .and_hms_opt(0, 0, 0)
        .expect("midnight should be valid for a valid date");

    match Local.from_local_datetime(&midnight) {
        LocalResult::Single(timestamp) | LocalResult::Ambiguous(timestamp, _) => {
            timestamp.with_timezone(&Utc)
        }
        LocalResult::None => fallback_offset
            .from_local_datetime(&midnight)
            .single()
            .expect("fixed offset local midnight should be unambiguous")
            .with_timezone(&Utc),
    }
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

#[cfg(test)]
mod tests {
    use super::usage_window_cutoff;
    use chrono::{Datelike, Duration, Local, TimeZone, Timelike, Utc};

    fn local_time(year: i32, month: u32, day: u32, hour: u32) -> chrono::DateTime<Local> {
        Local
            .with_ymd_and_hms(year, month, day, hour, 30, 0)
            .single()
            .expect("test local time should exist")
    }

    #[test]
    fn usage_window_cutoff_uses_last_30_days_relative_to_now() {
        let now = local_time(2026, 5, 15, 12);
        let expected = now.with_timezone(&Utc) - Duration::days(30);

        assert_eq!(usage_window_cutoff("last30d", now), Some(expected));
    }

    #[test]
    fn usage_window_cutoff_uses_start_of_current_local_month() {
        let now = local_time(2026, 5, 15, 12);
        let cutoff = usage_window_cutoff("month", now)
            .expect("month should produce a cutoff")
            .with_timezone(&Local);

        assert_eq!(cutoff.year(), 2026);
        assert_eq!(cutoff.month(), 5);
        assert_eq!(cutoff.day(), 1);
        assert_eq!(cutoff.hour(), 0);
        assert_eq!(cutoff.minute(), 0);
        assert_eq!(cutoff.second(), 0);
    }

    #[test]
    fn usage_window_cutoff_uses_start_of_current_local_day() {
        let now = local_time(2026, 5, 15, 12);
        let cutoff = usage_window_cutoff("today", now)
            .expect("today should produce a cutoff")
            .with_timezone(&Local);

        assert_eq!(cutoff.year(), 2026);
        assert_eq!(cutoff.month(), 5);
        assert_eq!(cutoff.day(), 15);
        assert_eq!(cutoff.hour(), 0);
        assert_eq!(cutoff.minute(), 0);
        assert_eq!(cutoff.second(), 0);
    }
}
