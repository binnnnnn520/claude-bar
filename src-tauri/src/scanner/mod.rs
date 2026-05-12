pub mod claude;
pub mod codex;
pub mod roots;
pub mod time;
pub mod types;

use self::roots::discover_roots;
use self::time::{codex_date_from_path, cutoff_last_30_days, date_key, usage_window_cutoff};
use self::types::{ProviderId, ProviderUsage, TokenBucket, UsageSnapshot};
use chrono::{DateTime, Local, NaiveDate, Utc};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn scan_usage(window: &str) -> UsageSnapshot {
    let now = Local::now();
    let scanned_at = now.with_timezone(&Utc);
    let cutoff =
        usage_window_cutoff(window, now).unwrap_or_else(|| cutoff_last_30_days(scanned_at.clone()));
    let roots = discover_roots();

    let codex = scan_codex_roots(&roots.codex_roots, cutoff).finalize();
    let claude = scan_claude_roots(&roots.claude_roots, cutoff).finalize();

    UsageSnapshot {
        window: window.to_owned(),
        scanned_at: scanned_at.to_rfc3339(),
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

struct CodexFileEntry {
    path: PathBuf,
    metadata: Option<codex::SessionMetadata>,
}

fn scan_codex_roots(roots: &[PathBuf], cutoff: DateTime<Utc>) -> ProviderUsage {
    let mut usage = ProviderUsage::empty(ProviderId::Codex);
    let files: Vec<CodexFileEntry> = jsonl_files(roots)
        .into_iter()
        .map(|path| CodexFileEntry {
            metadata: read_codex_session_metadata(&path),
            path,
        })
        .collect();
    let mut session_paths: HashMap<String, PathBuf> = HashMap::new();
    for entry in &files {
        if let Some(id) = entry
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.id.as_ref())
        {
            session_paths
                .entry(id.clone())
                .or_insert_with(|| entry.path.clone());
        }
    }
    let mut seen_session_ids = HashSet::new();

    for entry in files {
        usage.files_scanned += 1;

        if let Some(id) = entry
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.id.as_ref())
        {
            if !seen_session_ids.insert(id.clone()) {
                continue;
            }
        }

        let inherited_totals =
            inherited_codex_totals(entry.metadata.as_ref(), &session_paths);
        scan_codex_file(&entry.path, cutoff, inherited_totals, &mut usage);
    }

    usage
}

fn read_codex_session_metadata(path: &Path) -> Option<codex::SessionMetadata> {
    let file = File::open(path).ok()?;
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if let Ok(Some(metadata)) = codex::parse_codex_session_metadata(&line) {
            return Some(metadata);
        }
    }

    None
}

fn inherited_codex_totals(
    metadata: Option<&codex::SessionMetadata>,
    session_paths: &HashMap<String, PathBuf>,
) -> Option<TokenBucket> {
    let metadata = metadata?;
    let forked_from_id = metadata.forked_from_id.as_ref()?;
    let forked_at = metadata.timestamp?;
    let parent_path = session_paths.get(forked_from_id)?;

    codex_totals_at_or_before(parent_path, forked_at)
}

fn codex_totals_at_or_before(path: &Path, forked_at: DateTime<Utc>) -> Option<TokenBucket> {
    let file = File::open(path).ok()?;
    let mut totals = TokenBucket::default();
    let mut found_totals = None;

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let parsed = match codex::parse_codex_line(&line) {
            Ok(Some(parsed)) => parsed,
            _ => continue,
        };

        if !parsed
            .timestamp
            .as_ref()
            .is_some_and(|timestamp| *timestamp <= forked_at)
        {
            continue;
        }

        if let Some(cumulative_bucket) = parsed.cumulative_bucket {
            totals = codex_totals_bucket(cumulative_bucket);
        } else {
            totals = codex_add_totals(&totals, &codex_totals_bucket(parsed.bucket));
        }

        found_totals = Some(totals.clone());
    }

    found_totals
}

fn scan_claude_roots(roots: &[PathBuf], cutoff: DateTime<Utc>) -> ProviderUsage {
    let mut usage = ProviderUsage::empty(ProviderId::Claude);

    for path in jsonl_files(roots) {
        usage.files_scanned += 1;
        scan_claude_file(&path, cutoff, &mut usage);
    }

    usage
}

fn scan_codex_file(
    path: &Path,
    cutoff: DateTime<Utc>,
    inherited_totals: Option<TokenBucket>,
    usage: &mut ProviderUsage,
) {
    let Ok(file) = File::open(path) else {
        usage.errors.push(format!("Cannot read {}", path.display()));
        return;
    };

    let fallback_date = codex_date_from_path(path);
    let found_usage = scan_codex_lines(
        BufReader::new(file).lines().map_while(Result::ok),
        Some(cutoff),
        fallback_date,
        inherited_totals,
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
    inherited_totals: Option<TokenBucket>,
    usage: &mut ProviderUsage,
) -> bool
where
    I: IntoIterator<Item = String>,
{
    let mut found_usage = false;
    let inherited_totals = inherited_totals.map(codex_totals_bucket);
    let mut previous_totals: Option<TokenBucket> = None;
    let mut remaining_inherited_totals = inherited_totals.clone();
    let mut current_model = Some("gpt-5".to_string());

    for line in lines {
        match codex::parse_codex_turn_context_model(&line) {
            Ok(Some(model)) => {
                current_model = Some(model);
                continue;
            }
            Ok(None) | Err(_) => {}
        }

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
            cumulative_bucket,
            source_kind,
        } = parsed;
        let _ = source_kind;
        let model = model.or_else(|| current_model.clone());

        let fallback_date = fallback_date.clone();

        let before_cutoff = if let Some(cutoff) = cutoff.as_ref() {
            let timestamp_before_cutoff = timestamp
                .as_ref()
                .is_some_and(|timestamp| timestamp < cutoff);
            let fallback_before_cutoff = timestamp.is_none()
                && fallback_date
                    .as_deref()
                    .is_some_and(|date| fallback_date_before_cutoff(date, cutoff));

            timestamp_before_cutoff || fallback_before_cutoff
        } else {
            false
        };

        let date = timestamp
            .map(date_key)
            .or(fallback_date)
            .unwrap_or_else(|| "unknown".to_string());

        let row = CodexUsageRow {
            date,
            model,
            bucket,
        };

        if let Some(cumulative_bucket) = cumulative_bucket {
            let raw_totals = codex_totals_bucket(cumulative_bucket);
            let adjusted_totals = inherited_totals
                .as_ref()
                .map(|inherited| codex_totals_delta(&raw_totals, inherited))
                .unwrap_or(raw_totals);

            if before_cutoff {
                previous_totals = Some(adjusted_totals);
                remaining_inherited_totals = None;
                continue;
            }

            let totals_delta = previous_totals
                .as_ref()
                .map(|previous| codex_totals_delta(&adjusted_totals, previous))
                .unwrap_or_else(|| adjusted_totals.clone());
            previous_totals = Some(adjusted_totals);
            remaining_inherited_totals = None;

            let delta_row = CodexUsageRow {
                date: row.date,
                model: row.model,
                bucket: codex_reporting_delta(totals_delta),
            };

            if delta_row.bucket.total_tokens == 0 {
                continue;
            }

            add_codex_usage_row(usage, delta_row);
            found_usage = true;
            continue;
        }

        let mut totals_delta = codex_totals_bucket(row.bucket);

        if let Some(remaining) = remaining_inherited_totals.as_mut() {
            totals_delta = codex_subtract_remaining_inherited(totals_delta, remaining);
            if codex_totals_empty(remaining) {
                remaining_inherited_totals = None;
            }
        }

        previous_totals = Some(
            previous_totals
                .as_ref()
                .map(|previous| codex_add_totals(previous, &totals_delta))
                .unwrap_or_else(|| totals_delta.clone()),
        );

        if before_cutoff {
            continue;
        }

        let delta_row = CodexUsageRow {
            date: row.date,
            model: row.model,
            bucket: codex_reporting_delta(totals_delta),
        };

        if delta_row.bucket.total_tokens == 0 {
            continue;
        }

        add_codex_usage_row(usage, delta_row);
        found_usage = true;
    }

    found_usage
}

fn codex_totals_bucket(mut bucket: TokenBucket) -> TokenBucket {
    bucket.cache_read_tokens = 0;
    bucket.cache_creation_tokens = 0;
    bucket.total_tokens = bucket.input_tokens + bucket.output_tokens;
    bucket
}

fn codex_reporting_delta(bucket: TokenBucket) -> TokenBucket {
    let mut bucket = codex_totals_bucket(bucket);
    bucket.cached_input_tokens = bucket.cached_input_tokens.min(bucket.input_tokens);
    bucket
}

fn codex_totals_delta(after: &TokenBucket, before: &TokenBucket) -> TokenBucket {
    codex_totals_bucket(TokenBucket {
        input_tokens: after.input_tokens.saturating_sub(before.input_tokens),
        cached_input_tokens: after
            .cached_input_tokens
            .saturating_sub(before.cached_input_tokens),
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
        output_tokens: after.output_tokens.saturating_sub(before.output_tokens),
        total_tokens: 0,
    })
}

fn codex_add_totals(current: &TokenBucket, delta: &TokenBucket) -> TokenBucket {
    codex_totals_bucket(TokenBucket {
        input_tokens: current.input_tokens + delta.input_tokens,
        cached_input_tokens: current.cached_input_tokens + delta.cached_input_tokens,
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
        output_tokens: current.output_tokens + delta.output_tokens,
        total_tokens: 0,
    })
}

fn codex_subtract_remaining_inherited(
    mut delta: TokenBucket,
    remaining: &mut TokenBucket,
) -> TokenBucket {
    let input = delta.input_tokens.min(remaining.input_tokens);
    delta.input_tokens -= input;
    remaining.input_tokens -= input;

    let cached = delta.cached_input_tokens.min(remaining.cached_input_tokens);
    delta.cached_input_tokens -= cached;
    remaining.cached_input_tokens -= cached;

    let output = delta.output_tokens.min(remaining.output_tokens);
    delta.output_tokens -= output;
    remaining.output_tokens -= output;

    *remaining = codex_totals_bucket(remaining.clone());
    codex_totals_bucket(delta)
}

fn codex_totals_empty(bucket: &TokenBucket) -> bool {
    bucket.input_tokens == 0 && bucket.cached_input_tokens == 0 && bucket.output_tokens == 0
}

struct CodexUsageRow {
    date: String,
    model: Option<String>,
    bucket: TokenBucket,
}

fn add_codex_usage_row(usage: &mut ProviderUsage, row: CodexUsageRow) {
    usage.add_daily_bucket(row.date, row.bucket.clone());
    if let Some(model) = row.model {
        usage.add_model_bucket(model, row.bucket);
    }
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
        Ok(date) if date < cutoff.with_timezone(&Local).date_naive()
    )
}

#[cfg(test)]
pub fn scan_codex_lines_for_test(lines: Vec<String>) -> ProviderUsage {
    let mut usage = ProviderUsage::empty(ProviderId::Codex);
    usage.files_scanned = 1;

    if scan_codex_lines(lines, None, None, None, &mut usage) {
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

    if scan_codex_lines(lines, Some(cutoff), fallback_date, None, &mut usage) {
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
    fn token_bucket_saturating_subtracts_each_field() {
        let after = TokenBucket {
            input_tokens: 10,
            cached_input_tokens: 20,
            cache_read_tokens: 30,
            cache_creation_tokens: 40,
            output_tokens: 50,
            total_tokens: 60,
        };
        let before = TokenBucket {
            input_tokens: 3,
            cached_input_tokens: 21,
            cache_read_tokens: 10,
            cache_creation_tokens: 41,
            output_tokens: 5,
            total_tokens: 100,
        };

        let bucket = after.saturating_sub(&before);

        assert_eq!(bucket.input_tokens, 7);
        assert_eq!(bucket.cached_input_tokens, 0);
        assert_eq!(bucket.cache_read_tokens, 20);
        assert_eq!(bucket.cache_creation_tokens, 0);
        assert_eq!(bucket.output_tokens, 45);
        assert_eq!(bucket.total_tokens, 0);
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
            r#"{"timestamp":"2026-05-01T12:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":10,"cached_input_tokens":5,"output_tokens":2,"total_tokens":12},"last_token_usage":{"input_tokens":10,"cached_input_tokens":5,"output_tokens":2,"total_tokens":12}},"model":"gpt-5.3-codex"}}"#.to_string(),
            r#"{"timestamp":"2026-05-01T12:01:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":30,"cached_input_tokens":12,"output_tokens":5,"total_tokens":35},"last_token_usage":{"input_tokens":20,"cached_input_tokens":7,"output_tokens":3,"total_tokens":23}},"model":"gpt-5.3-codex"}}"#.to_string(),
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
    fn scan_codex_lines_skips_repeated_last_token_usage_rows() {
        let lines = vec![
            r#"{"timestamp":"2026-05-01T12:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":10,"cached_input_tokens":5,"output_tokens":2,"total_tokens":12},"last_token_usage":{"input_tokens":10,"cached_input_tokens":5,"output_tokens":2,"total_tokens":12}},"model":"gpt-5.3-codex"}}"#.to_string(),
            r#"{"timestamp":"2026-05-01T12:00:01Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":10,"cached_input_tokens":5,"output_tokens":2,"total_tokens":12},"last_token_usage":{"input_tokens":10,"cached_input_tokens":5,"output_tokens":2,"total_tokens":12}},"model":"gpt-5.3-codex"}}"#.to_string(),
            r#"{"timestamp":"2026-05-01T12:01:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":30,"cached_input_tokens":12,"output_tokens":5,"total_tokens":35},"last_token_usage":{"input_tokens":20,"cached_input_tokens":7,"output_tokens":3,"total_tokens":23}},"model":"gpt-5.3-codex"}}"#.to_string(),
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
        assert_eq!(usage.models[0].bucket.total_tokens, 35);
    }

    #[test]
    fn scan_codex_lines_clamps_cached_input_to_input_delta() {
        let lines = vec![
            r#"{"timestamp":"2026-05-01T12:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":150,"output_tokens":10,"total_tokens":110},"last_token_usage":{"input_tokens":100,"cached_input_tokens":150,"output_tokens":10,"total_tokens":110}},"model":"gpt-5.3-codex"}}"#.to_string(),
        ];

        let usage = super::scan_codex_lines_for_test(lines);

        assert_eq!(usage.bucket.input_tokens, 100);
        assert_eq!(usage.bucket.cached_input_tokens, 100);
        assert_eq!(usage.bucket.output_tokens, 10);
        assert_eq!(usage.bucket.total_tokens, 110);
    }

    #[test]
    fn scan_codex_lines_uses_turn_context_model_for_token_rows() {
        let lines = vec![
            r#"{"timestamp":"2026-05-01T12:00:00Z","type":"turn_context","payload":{"model":"gpt-5.4","cwd":"D:\\project"}}"#.to_string(),
            r#"{"timestamp":"2026-05-01T12:01:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":20,"output_tokens":10,"total_tokens":110},"last_token_usage":{"input_tokens":100,"cached_input_tokens":20,"output_tokens":10,"total_tokens":110}}}}"#.to_string(),
        ];

        let usage = super::scan_codex_lines_for_test(lines);

        assert_eq!(usage.models.len(), 1);
        assert_eq!(usage.models[0].model, "gpt-5.4");
        assert_eq!(usage.models[0].bucket.total_tokens, 110);
    }

    #[test]
    fn scan_codex_lines_uses_one_cumulative_total_when_no_delta_rows() {
        let lines = vec![
            r#"{"timestamp":"2026-05-01T12:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":900,"cached_input_tokens":300,"output_tokens":100,"total_tokens":1000}},"model":"gpt-5.3-codex"}}"#.to_string(),
            r#"{"timestamp":"2026-05-01T12:01:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1050,"cached_input_tokens":350,"output_tokens":150,"total_tokens":1200}},"model":"gpt-5.3-codex"}}"#.to_string(),
        ];

        let usage = super::scan_codex_lines_for_test(lines);

        assert_eq!(usage.files_scanned, 1);
        assert_eq!(usage.files_with_usage, 1);
        assert_eq!(usage.bucket.input_tokens, 1050);
        assert_eq!(usage.bucket.cached_input_tokens, 350);
        assert_eq!(usage.bucket.output_tokens, 150);
        assert_eq!(usage.bucket.total_tokens, 1200);
        assert_eq!(usage.daily[0].bucket.total_tokens, 1200);
        assert_eq!(usage.models[0].bucket.total_tokens, 1200);
    }

    #[test]
    fn scan_codex_lines_subtracts_cumulative_baseline_before_cutoff() {
        let lines = vec![
            r#"{"timestamp":"2026-04-01T12:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":900,"cached_input_tokens":300,"output_tokens":100,"total_tokens":1000}},"model":"before-model"}}"#.to_string(),
            r#"{"timestamp":"2026-05-01T12:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1050,"cached_input_tokens":350,"output_tokens":150,"total_tokens":1200}},"model":"after-model"}}"#.to_string(),
        ];

        let usage = super::scan_codex_lines_for_test_with_cutoff_and_fallback_date(
            lines,
            utc_timestamp("2026-04-15T00:00:00Z"),
            None,
        );

        assert_eq!(usage.files_scanned, 1);
        assert_eq!(usage.files_with_usage, 1);
        assert_eq!(usage.bucket.input_tokens, 150);
        assert_eq!(usage.bucket.cached_input_tokens, 50);
        assert_eq!(usage.bucket.output_tokens, 50);
        assert_eq!(usage.bucket.total_tokens, 200);
        assert_eq!(usage.daily[0].date, "2026-05-01");
        assert_eq!(usage.daily[0].bucket.total_tokens, 200);
        assert_eq!(usage.models[0].model, "after-model");
        assert_eq!(usage.models[0].bucket.total_tokens, 200);
    }

    #[test]
    fn scan_codex_roots_dedupes_duplicate_session_ids() {
        let root = test_codex_root("dedupe-session");
        let first = root.join("first.jsonl");
        let second = root.join("second.jsonl");

        std::fs::write(
            &first,
            [
                r#"{"timestamp":"2026-05-01T12:00:00Z","type":"session_meta","payload":{"id":"session-1","timestamp":"2026-05-01T12:00:00Z"}}"#,
                r#"{"timestamp":"2026-05-01T12:01:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":10,"cached_input_tokens":3,"output_tokens":2,"total_tokens":12},"last_token_usage":{"input_tokens":10,"cached_input_tokens":3,"output_tokens":2,"total_tokens":12}}}}"#,
            ]
            .join("\n"),
        )
        .expect("first session should write");
        std::fs::write(
            &second,
            [
                r#"{"timestamp":"2026-05-01T12:00:00Z","type":"session_meta","payload":{"id":"session-1","timestamp":"2026-05-01T12:00:00Z"}}"#,
                r#"{"timestamp":"2026-05-01T12:01:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":30,"output_tokens":20,"total_tokens":120},"last_token_usage":{"input_tokens":100,"cached_input_tokens":30,"output_tokens":20,"total_tokens":120}}}}"#,
            ]
            .join("\n"),
        )
        .expect("second session should write");

        let usage = super::scan_codex_roots(&[root.clone()], utc_timestamp("2026-05-01T00:00:00Z")).finalize();

        assert_eq!(usage.files_scanned, 2);
        assert_eq!(usage.files_with_usage, 1);
        assert_eq!(usage.bucket.input_tokens, 10);
        assert_eq!(usage.bucket.output_tokens, 2);
        assert_eq!(usage.bucket.total_tokens, 12);

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn scan_codex_roots_subtracts_parent_totals_for_forked_sessions() {
        let root = test_codex_root("fork-inherited");
        let parent = root.join("parent.jsonl");
        let child = root.join("child.jsonl");

        std::fs::write(
            &parent,
            [
                r#"{"timestamp":"2026-05-01T12:00:00Z","type":"session_meta","payload":{"id":"parent-session","timestamp":"2026-05-01T12:00:00Z"}}"#,
                r#"{"timestamp":"2026-05-01T12:10:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":40,"output_tokens":10,"total_tokens":110},"last_token_usage":{"input_tokens":100,"cached_input_tokens":40,"output_tokens":10,"total_tokens":110}}}}"#,
            ]
            .join("\n"),
        )
        .expect("parent session should write");
        std::fs::write(
            &child,
            [
                r#"{"timestamp":"2026-05-01T12:20:00Z","type":"session_meta","payload":{"id":"child-session","forked_from_id":"parent-session","timestamp":"2026-05-01T12:20:00Z"}}"#,
                r#"{"timestamp":"2026-05-01T12:25:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":130,"cached_input_tokens":50,"output_tokens":15,"total_tokens":145},"last_token_usage":{"input_tokens":30,"cached_input_tokens":10,"output_tokens":5,"total_tokens":35}}}}"#,
            ]
            .join("\n"),
        )
        .expect("child session should write");

        let usage = super::scan_codex_roots(&[root.clone()], utc_timestamp("2026-05-01T00:00:00Z")).finalize();

        assert_eq!(usage.files_scanned, 2);
        assert_eq!(usage.files_with_usage, 2);
        assert_eq!(usage.bucket.input_tokens, 130);
        assert_eq!(usage.bucket.cached_input_tokens, 50);
        assert_eq!(usage.bucket.output_tokens, 15);
        assert_eq!(usage.bucket.total_tokens, 145);

        std::fs::remove_dir_all(root).ok();
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
    fn fallback_path_date_compares_against_local_cutoff_date() {
        let cutoff = utc_timestamp("2026-05-04T16:00:00Z");
        let cutoff_date = cutoff.with_timezone(&chrono::Local).date_naive();
        let previous_date = cutoff_date
            .pred_opt()
            .expect("cutoff date should have a previous day")
            .format("%Y-%m-%d")
            .to_string();
        let cutoff_date = cutoff_date.format("%Y-%m-%d").to_string();

        assert!(super::fallback_date_before_cutoff(&previous_date, &cutoff));
        assert!(!super::fallback_date_before_cutoff(&cutoff_date, &cutoff));
    }

    fn test_codex_root(name: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("token-ledger-{name}-{stamp}"));
        std::fs::create_dir_all(&root).expect("test root should be created");
        root
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
        assert_eq!(parsed.bucket.total_tokens, 140);
        assert_eq!(parsed.model.as_deref(), Some("gpt-5.3-codex"));
        assert_eq!(parsed.source_kind, super::codex::UsageSourceKind::Delta);
    }

    #[test]
    fn codex_parser_flat_token_count_does_not_add_cached_input_to_total_fallback() {
        let line = r#"{"timestamp":"2026-05-01T12:00:00Z","type":"event_msg","payload":{"type":"token_count","input_tokens":100,"cached_input_tokens":40,"output_tokens":20,"model":"gpt-5.3-codex"}}"#;
        let parsed = super::codex::parse_codex_line(line)
            .expect("valid json should parse")
            .expect("usage should parse");

        assert_eq!(parsed.bucket.input_tokens, 100);
        assert_eq!(parsed.bucket.cached_input_tokens, 40);
        assert_eq!(parsed.bucket.output_tokens, 20);
        assert_eq!(parsed.bucket.total_tokens, 120);
        assert_eq!(parsed.source_kind, super::codex::UsageSourceKind::Delta);
    }

    #[test]
    fn codex_parser_fallback_does_not_add_reasoning_when_output_is_present() {
        let line = r#"{"timestamp":"2026-05-01T12:00:00Z","type":"event_msg","payload":{"type":"token_count","input_tokens":100,"cached_input_tokens":40,"output_tokens":20,"reasoning_output_tokens":7,"model":"gpt-5.3-codex"}}"#;
        let parsed = super::codex::parse_codex_line(line)
            .expect("valid json should parse")
            .expect("usage should parse");

        assert_eq!(parsed.bucket.input_tokens, 100);
        assert_eq!(parsed.bucket.cached_input_tokens, 40);
        assert_eq!(parsed.bucket.output_tokens, 20);
        assert_eq!(parsed.bucket.total_tokens, 120);
        assert_eq!(parsed.source_kind, super::codex::UsageSourceKind::Delta);
    }

    #[test]
    fn codex_parser_fallback_ignores_reasoning_when_output_is_absent() {
        let line = r#"{"timestamp":"2026-05-01T12:00:00Z","type":"event_msg","payload":{"type":"token_count","input_tokens":100,"cached_input_tokens":40,"reasoning_output_tokens":7,"model":"gpt-5.3-codex"}}"#;
        let parsed = super::codex::parse_codex_line(line)
            .expect("valid json should parse")
            .expect("usage should parse");

        assert_eq!(parsed.bucket.input_tokens, 100);
        assert_eq!(parsed.bucket.cached_input_tokens, 40);
        assert_eq!(parsed.bucket.output_tokens, 0);
        assert_eq!(parsed.bucket.total_tokens, 100);
        assert_eq!(parsed.source_kind, super::codex::UsageSourceKind::Delta);
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
        assert_eq!(parsed.source_kind, super::codex::UsageSourceKind::Delta);
    }

    #[test]
    fn codex_parser_keeps_total_token_usage_when_last_usage_is_empty() {
        let line = r#"{"timestamp":"2026-05-05T01:43:10.790Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1000,"cached_input_tokens":500,"output_tokens":100,"total_tokens":1100},"last_token_usage":{}}}}"#;
        let parsed = super::codex::parse_codex_line(line)
            .expect("valid json should parse")
            .expect("usage should parse");

        assert_eq!(parsed.bucket.total_tokens, 0);
        assert_eq!(
            parsed
                .cumulative_bucket
                .expect("total usage should be retained")
                .total_tokens,
            1100
        );
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
        assert_eq!(
            parsed.source_kind,
            super::codex::UsageSourceKind::Cumulative
        );
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
