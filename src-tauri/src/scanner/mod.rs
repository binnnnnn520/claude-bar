pub mod claude;
pub mod codex;
pub mod roots;
pub mod time;
pub mod types;

use self::roots::discover_roots;
use self::time::{codex_date_from_path, cutoff_last_30_days, date_key};
use self::types::{ProviderId, ProviderUsage, UsageSnapshot};
use chrono::{DateTime, NaiveDate, Utc};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn scan_usage(window: &str) -> UsageSnapshot {
    let now = Utc::now();
    let cutoff = cutoff_last_30_days(now);
    let roots = discover_roots();

    let codex = scan_codex_roots(&roots.codex_roots, cutoff).finalize();
    let claude = scan_claude_roots(&roots.claude_roots, cutoff).finalize();

    UsageSnapshot {
        window: window.to_owned(),
        scanned_at: now.to_rfc3339(),
        providers: vec![codex, claude],
    }
}

fn jsonl_files(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for root in roots {
        if !root.exists() {
            continue;
        }

        for entry in WalkDir::new(root).follow_links(false).into_iter().flatten() {
            let path = entry.path();
            if path.is_file()
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl"))
            {
                files.push(path.to_path_buf());
            }
        }
    }

    files
}

fn scan_codex_roots(roots: &[PathBuf], cutoff: DateTime<Utc>) -> ProviderUsage {
    let mut usage = ProviderUsage::empty(ProviderId::Codex);

    for path in jsonl_files(roots) {
        usage.files_scanned += 1;
        scan_codex_file(&path, cutoff, &mut usage);
    }

    usage
}

fn scan_claude_roots(roots: &[PathBuf], cutoff: DateTime<Utc>) -> ProviderUsage {
    let mut usage = ProviderUsage::empty(ProviderId::Claude);

    for path in jsonl_files(roots) {
        usage.files_scanned += 1;
        scan_claude_file(&path, cutoff, &mut usage);
    }

    usage
}

fn scan_codex_file(path: &Path, cutoff: DateTime<Utc>, usage: &mut ProviderUsage) {
    let Ok(file) = File::open(path) else {
        usage.errors.push(format!("Cannot read {}", path.display()));
        return;
    };

    let fallback_date = codex_date_from_path(path);
    let found_usage = scan_codex_lines(
        BufReader::new(file).lines().map_while(Result::ok),
        Some(cutoff),
        fallback_date,
        usage,
    );

    if found_usage {
        usage.files_with_usage += 1;
    }
}

fn scan_claude_file(path: &Path, cutoff: DateTime<Utc>, usage: &mut ProviderUsage) {
    let Ok(file) = File::open(path) else {
        usage.errors.push(format!("Cannot read {}", path.display()));
        return;
    };

    let found_usage = scan_claude_lines(
        BufReader::new(file).lines().map_while(Result::ok),
        Some(cutoff),
        usage,
    );

    if found_usage {
        usage.files_with_usage += 1;
    }
}

fn scan_codex_lines<I>(
    lines: I,
    cutoff: Option<DateTime<Utc>>,
    fallback_date: Option<String>,
    usage: &mut ProviderUsage,
) -> bool
where
    I: IntoIterator<Item = String>,
{
    let mut found_usage = false;

    for line in lines {
        let parsed = match codex::parse_codex_line(&line) {
            Ok(Some(parsed)) => parsed,
            Ok(None) => continue,
            Err(_) => {
                usage.parse_warnings += 1;
                continue;
            }
        };

        let codex::ParsedUsage {
            timestamp,
            model,
            bucket,
        } = parsed;

        let fallback_date = fallback_date.clone();

        if let Some(cutoff) = cutoff.as_ref() {
            let timestamp_before_cutoff = timestamp
                .as_ref()
                .is_some_and(|timestamp| timestamp < cutoff);
            let fallback_before_cutoff = timestamp.is_none()
                && fallback_date
                    .as_deref()
                    .is_some_and(|date| fallback_date_before_cutoff(date, cutoff));

            if timestamp_before_cutoff || fallback_before_cutoff {
                continue;
            }
        }

        let date = timestamp
            .map(date_key)
            .or(fallback_date)
            .unwrap_or_else(|| "unknown".to_string());
        usage.add_daily_bucket(date, bucket.clone());
        if let Some(model) = model {
            usage.add_model_bucket(model, bucket);
        }
        found_usage = true;
    }

    found_usage
}

fn scan_claude_lines<I>(
    lines: I,
    cutoff: Option<DateTime<Utc>>,
    usage: &mut ProviderUsage,
) -> bool
where
    I: IntoIterator<Item = String>,
{
    let mut found_usage = false;
    let mut seen = HashSet::new();

    for line in lines {
        let parsed = match claude::parse_claude_line(&line) {
            Ok(Some(parsed)) => parsed,
            Ok(None) => continue,
            Err(_) => {
                usage.parse_warnings += 1;
                continue;
            }
        };

        let claude::ParsedUsage {
            timestamp,
            model,
            dedupe_key,
            bucket,
        } = parsed;

        if let (Some(timestamp), Some(cutoff)) = (timestamp.as_ref(), cutoff.as_ref()) {
            if timestamp < cutoff {
                continue;
            }
        }

        if let Some(key) = dedupe_key {
            if !seen.insert(key) {
                continue;
            }
        }

        let date = timestamp
            .map(date_key)
            .unwrap_or_else(|| "unknown".to_string());
        usage.add_daily_bucket(date, bucket.clone());
        if let Some(model) = model {
            usage.add_model_bucket(model, bucket);
        }
        found_usage = true;
    }

    found_usage
}

fn fallback_date_before_cutoff(fallback_date: &str, cutoff: &DateTime<Utc>) -> bool {
    matches!(
        NaiveDate::parse_from_str(fallback_date, "%Y-%m-%d"),
        Ok(date) if date < cutoff.date_naive()
    )
}

#[cfg(test)]
pub fn scan_codex_lines_for_test(lines: Vec<String>) -> ProviderUsage {
    let mut usage = ProviderUsage::empty(ProviderId::Codex);
    usage.files_scanned = 1;

    if scan_codex_lines(lines, None, None, &mut usage) {
        usage.files_with_usage = 1;
    }

    usage.finalize()
}

#[cfg(test)]
pub fn scan_codex_lines_for_test_with_cutoff_and_fallback_date(
    lines: Vec<String>,
    cutoff: DateTime<Utc>,
    fallback_date: Option<String>,
) -> ProviderUsage {
    let mut usage = ProviderUsage::empty(ProviderId::Codex);
    usage.files_scanned = 1;

    if scan_codex_lines(lines, Some(cutoff), fallback_date, &mut usage) {
        usage.files_with_usage = 1;
    }

    usage.finalize()
}

#[cfg(test)]
pub fn scan_claude_lines_for_test(lines: Vec<String>) -> ProviderUsage {
    let mut usage = ProviderUsage::empty(ProviderId::Claude);
    usage.files_scanned = 1;

    if scan_claude_lines(lines, None, &mut usage) {
        usage.files_with_usage = 1;
    }

    usage.finalize()
}

#[cfg(test)]
pub fn scan_claude_lines_for_test_with_cutoff(
    lines: Vec<String>,
    cutoff: DateTime<Utc>,
) -> ProviderUsage {
    let mut usage = ProviderUsage::empty(ProviderId::Claude);
    usage.files_scanned = 1;

    if scan_claude_lines(lines, Some(cutoff), &mut usage) {
        usage.files_with_usage = 1;
    }

    usage.finalize()
}

#[cfg(test)]
mod tests {
    use super::types::{ProviderId, ProviderUsage, TokenBucket};
    use chrono::{DateTime, Utc};

    fn utc_timestamp(raw: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(raw)
            .expect("timestamp should parse")
            .with_timezone(&Utc)
    }

    #[test]
    fn token_bucket_total_includes_input_cache_and_output() {
        let bucket = TokenBucket {
            input_tokens: 10,
            cached_input_tokens: 3,
            cache_read_tokens: 4,
            cache_creation_tokens: 5,
            output_tokens: 7,
            total_tokens: 0,
        }
        .with_total();

        assert_eq!(bucket.total_tokens, 29);
    }

    #[test]
    fn provider_usage_adds_buckets() {
        let mut usage = ProviderUsage::empty(ProviderId::Codex);
        usage.add_bucket(TokenBucket {
            input_tokens: 10,
            cached_input_tokens: 2,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            output_tokens: 5,
            total_tokens: 17,
        });

        assert_eq!(usage.bucket.total_tokens, 17);
        assert_eq!(usage.bucket.input_tokens, 10);
        assert_eq!(usage.bucket.cached_input_tokens, 2);
        assert_eq!(usage.bucket.output_tokens, 5);
    }

    #[test]
    fn scan_lines_aggregates_provider_usage() {
        let lines = vec![
            r#"{"type":"assistant","timestamp":"2026-05-01T12:00:00Z","message":{"id":"msg_1","model":"claude-sonnet","usage":{"input_tokens":10,"output_tokens":5}}}"#.to_string(),
            r#"{"type":"assistant","timestamp":"2026-05-01T12:01:00Z","message":{"id":"msg_2","model":"claude-sonnet","usage":{"input_tokens":20,"output_tokens":7}}}"#.to_string(),
        ];

        let usage = super::scan_claude_lines_for_test(lines);

        assert_eq!(usage.files_scanned, 1);
        assert_eq!(usage.files_with_usage, 1);
        assert_eq!(usage.bucket.input_tokens, 30);
        assert_eq!(usage.bucket.output_tokens, 12);
        assert_eq!(usage.bucket.total_tokens, 42);
        assert_eq!(usage.daily.len(), 1);
        assert_eq!(usage.daily[0].bucket.total_tokens, 42);
        assert_eq!(usage.models[0].model, "claude-sonnet");
        assert_eq!(usage.models[0].bucket.total_tokens, 42);
    }

    #[test]
    fn scan_lines_counts_malformed_json_parse_warnings() {
        let lines = vec![
            r#"{"type":"assistant""#.to_string(),
            r#"{"type":"assistant","timestamp":"2026-05-01T12:00:00Z","message":{"id":"msg_1","model":"claude-sonnet","usage":{"input_tokens":10,"output_tokens":5}}}"#.to_string(),
        ];

        let usage = super::scan_claude_lines_for_test(lines);

        assert_eq!(usage.parse_warnings, 1);
        assert_eq!(usage.files_scanned, 1);
        assert_eq!(usage.files_with_usage, 1);
        assert_eq!(usage.bucket.total_tokens, 15);
    }

    #[test]
    fn scan_codex_lines_aggregates_last_token_usage_without_cumulative_overcount() {
        let lines = vec![
            r#"{"timestamp":"2026-05-01T12:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1000,"cached_input_tokens":500,"output_tokens":100,"total_tokens":1100},"last_token_usage":{"input_tokens":10,"cached_input_tokens":5,"output_tokens":2,"total_tokens":12}},"model":"gpt-5.3-codex"}}"#.to_string(),
            r#"{"timestamp":"2026-05-01T12:01:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":2000,"cached_input_tokens":700,"output_tokens":200,"total_tokens":2200},"last_token_usage":{"input_tokens":20,"cached_input_tokens":7,"output_tokens":3,"total_tokens":23}},"model":"gpt-5.3-codex"}}"#.to_string(),
        ];

        let usage = super::scan_codex_lines_for_test(lines);

        assert_eq!(usage.files_scanned, 1);
        assert_eq!(usage.files_with_usage, 1);
        assert_eq!(usage.bucket.input_tokens, 30);
        assert_eq!(usage.bucket.cached_input_tokens, 12);
        assert_eq!(usage.bucket.output_tokens, 5);
        assert_eq!(usage.bucket.total_tokens, 35);
        assert_eq!(usage.daily.len(), 1);
        assert_eq!(usage.daily[0].bucket.total_tokens, 35);
        assert_eq!(usage.models[0].model, "gpt-5.3-codex");
        assert_eq!(usage.models[0].bucket.total_tokens, 35);
    }

    #[test]
    fn scan_claude_lines_dedupes_after_timestamp_cutoff() {
        let lines = vec![
            r#"{"type":"assistant","timestamp":"2026-04-01T12:00:00Z","message":{"id":"msg_1","model":"claude-sonnet","usage":{"input_tokens":100,"output_tokens":50}}}"#.to_string(),
            r#"{"type":"assistant","timestamp":"2026-05-01T12:00:00Z","message":{"id":"msg_1","model":"claude-sonnet","usage":{"input_tokens":20,"output_tokens":7}}}"#.to_string(),
        ];

        let usage = super::scan_claude_lines_for_test_with_cutoff(
            lines,
            utc_timestamp("2026-04-15T00:00:00Z"),
        );

        assert_eq!(usage.files_scanned, 1);
        assert_eq!(usage.files_with_usage, 1);
        assert_eq!(usage.bucket.input_tokens, 20);
        assert_eq!(usage.bucket.output_tokens, 7);
        assert_eq!(usage.bucket.total_tokens, 27);
        assert_eq!(usage.models[0].bucket.total_tokens, 27);
    }

    #[test]
    fn scan_codex_lines_skips_old_path_date_fallback_without_timestamp() {
        let lines = vec![
            r#"{"type":"event_msg","payload":{"type":"token_count","input_tokens":10,"output_tokens":5,"model":"gpt-5.3-codex"}}"#.to_string(),
        ];
        let fallback_date = super::time::codex_date_from_path(std::path::Path::new(
            "sessions/2026/04/01/session.jsonl",
        ));

        let usage = super::scan_codex_lines_for_test_with_cutoff_and_fallback_date(
            lines,
            utc_timestamp("2026-04-15T00:00:00Z"),
            fallback_date,
        );

        assert_eq!(usage.files_scanned, 1);
        assert_eq!(usage.files_with_usage, 0);
        assert_eq!(usage.bucket.total_tokens, 0);
        assert!(usage.daily.is_empty());
        assert!(usage.models.is_empty());
    }

    #[test]
    fn claude_parser_extracts_assistant_usage() {
        let line = r#"{"type":"assistant","timestamp":"2026-05-01T12:00:00Z","message":{"id":"msg_1","model":"claude-sonnet","usage":{"input_tokens":100,"cache_creation_input_tokens":20,"cache_read_input_tokens":30,"output_tokens":40}}}"#;
        let parsed = super::claude::parse_claude_line(line)
            .expect("valid json should parse")
            .expect("usage should parse");

        assert_eq!(parsed.bucket.input_tokens, 100);
        assert_eq!(parsed.bucket.cache_creation_tokens, 20);
        assert_eq!(parsed.bucket.cache_read_tokens, 30);
        assert_eq!(parsed.bucket.output_tokens, 40);
        assert_eq!(parsed.bucket.total_tokens, 190);
        assert_eq!(parsed.model.as_deref(), Some("claude-sonnet"));
        assert_eq!(parsed.dedupe_key.as_deref(), Some("msg_1"));
    }

    #[test]
    fn claude_parser_skips_non_assistant_rows() {
        let line = r#"{"type":"user","timestamp":"2026-05-01T12:00:00Z","message":{"content":"hidden"}}"#;
        assert!(super::claude::parse_claude_line(line)
            .expect("valid json should parse")
            .is_none());
    }

    #[test]
    fn claude_parser_reports_malformed_json() {
        let line = r#"{"type":"assistant""#;
        assert!(super::claude::parse_claude_line(line).is_err());
    }

    #[test]
    fn claude_parser_uses_request_id_as_dedupe_fallback() {
        let line = r#"{"type":"assistant","requestId":"req_1","timestamp":"2026-05-01T12:00:00Z","message":{"model":"claude-sonnet","usage":{"input_tokens":10,"output_tokens":5}}}"#;
        let parsed = super::claude::parse_claude_line(line)
            .expect("valid json should parse")
            .expect("usage should parse");

        assert_eq!(parsed.dedupe_key.as_deref(), Some("req_1"));
    }

    #[test]
    fn claude_parser_skips_zero_token_usage() {
        let line = r#"{"type":"assistant","timestamp":"2026-05-01T12:00:00Z","message":{"id":"msg_1","model":"claude-sonnet","usage":{}}}"#;
        assert!(super::claude::parse_claude_line(line)
            .expect("valid json should parse")
            .is_none());
    }

    #[test]
    fn codex_parser_extracts_payload_token_count() {
        let line = r#"{"timestamp":"2026-05-01T12:00:00Z","type":"event_msg","payload":{"type":"token_count","input_tokens":100,"cached_input_tokens":25,"output_tokens":40,"model":"gpt-5.3-codex"}}"#;
        let parsed = super::codex::parse_codex_line(line)
            .expect("valid json should parse")
            .expect("usage should parse");

        assert_eq!(parsed.bucket.input_tokens, 100);
        assert_eq!(parsed.bucket.cached_input_tokens, 25);
        assert_eq!(parsed.bucket.output_tokens, 40);
        assert_eq!(parsed.bucket.total_tokens, 165);
        assert_eq!(parsed.model.as_deref(), Some("gpt-5.3-codex"));
    }

    #[test]
    fn codex_parser_extracts_realistic_info_token_count() {
        let line = r#"{"timestamp":"2026-05-05T01:43:10.790Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":11054,"cached_input_tokens":6528,"output_tokens":78,"reasoning_output_tokens":60,"total_tokens":11132},"last_token_usage":{"input_tokens":11054,"cached_input_tokens":6528,"output_tokens":78,"reasoning_output_tokens":60,"total_tokens":11132},"model_context_window":258400},"rate_limits":{"limit_id":"codex"}}}"#;
        let parsed = super::codex::parse_codex_line(line)
            .expect("valid json should parse")
            .expect("usage should parse");

        assert_eq!(parsed.bucket.input_tokens, 11054);
        assert_eq!(parsed.bucket.cached_input_tokens, 6528);
        assert_eq!(parsed.bucket.output_tokens, 78);
        assert_eq!(parsed.bucket.total_tokens, 11132);
    }

    #[test]
    fn codex_parser_prefers_last_token_usage_for_row_aggregation() {
        let line = r#"{"timestamp":"2026-05-05T01:43:10.790Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1000,"cached_input_tokens":500,"output_tokens":100,"total_tokens":1100},"last_token_usage":{"input_tokens":10,"cached_input_tokens":5,"output_tokens":2,"total_tokens":12}}}}"#;
        let parsed = super::codex::parse_codex_line(line)
            .expect("valid json should parse")
            .expect("usage should parse");

        assert_eq!(parsed.bucket.input_tokens, 10);
        assert_eq!(parsed.bucket.cached_input_tokens, 5);
        assert_eq!(parsed.bucket.output_tokens, 2);
        assert_eq!(parsed.bucket.total_tokens, 12);
    }

    #[test]
    fn codex_parser_falls_back_to_total_token_usage() {
        let line = r#"{"timestamp":"2026-05-05T01:43:10.790Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":20,"cached_input_tokens":8,"output_tokens":4,"total_tokens":24}}}}"#;
        let parsed = super::codex::parse_codex_line(line)
            .expect("valid json should parse")
            .expect("usage should parse");

        assert_eq!(parsed.bucket.input_tokens, 20);
        assert_eq!(parsed.bucket.cached_input_tokens, 8);
        assert_eq!(parsed.bucket.output_tokens, 4);
        assert_eq!(parsed.bucket.total_tokens, 24);
    }

    #[test]
    fn codex_parser_skips_token_count_with_null_info_and_rate_limits_only() {
        let line = r#"{"timestamp":"2026-05-05T01:43:10.790Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{"limit_id":"codex"}}}"#;
        assert!(super::codex::parse_codex_line(line)
            .expect("valid json should parse")
            .is_none());
    }

    #[test]
    fn codex_parser_skips_non_usage_rows() {
        let line = r#"{"timestamp":"2026-05-01T12:00:00Z","type":"response_item","payload":{"type":"message","content":"hidden"}}"#;
        assert!(super::codex::parse_codex_line(line)
            .expect("valid json should parse")
            .is_none());
    }

    #[test]
    fn codex_parser_reports_malformed_json() {
        let line = r#"{"timestamp":"2026-05-01T12:00:00Z","type":"event_msg""#;
        assert!(super::codex::parse_codex_line(line).is_err());
    }
}
