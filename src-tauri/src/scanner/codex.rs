use super::time::parse_timestamp;
use super::types::TokenBucket;
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageSourceKind {
    Delta,
    Cumulative,
}

#[derive(Debug, Clone)]
pub struct ParsedUsage {
    pub timestamp: Option<DateTime<Utc>>,
    pub model: Option<String>,
    pub bucket: TokenBucket,
    pub cumulative_bucket: Option<TokenBucket>,
    pub source_kind: UsageSourceKind,
}

#[derive(Debug, Clone, Default)]
pub struct SessionMetadata {
    pub id: Option<String>,
    pub forked_from_id: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
}

pub fn parse_codex_session_metadata(
    line: &str,
) -> Result<Option<SessionMetadata>, serde_json::Error> {
    let value: Value = serde_json::from_str(line)?;

    if value.get("type").and_then(Value::as_str) != Some("session_meta") {
        return Ok(None);
    }

    let payload = value.get("payload").unwrap_or(&value);
    Ok(Some(SessionMetadata {
        id: string_at_any(payload, &["session_id", "sessionId", "id"]),
        forked_from_id: string_at_any(
            payload,
            &[
                "forked_from_id",
                "forkedFromId",
                "parent_session_id",
                "parentSessionId",
            ],
        ),
        timestamp: parse_timestamp(payload).or_else(|| parse_timestamp(&value)),
    }))
}

pub fn parse_codex_turn_context_model(line: &str) -> Result<Option<String>, serde_json::Error> {
    let value: Value = serde_json::from_str(line)?;

    if value.get("type").and_then(Value::as_str) != Some("turn_context") {
        return Ok(None);
    }

    let payload = value.get("payload").unwrap_or(&value);
    Ok(string_at_any(payload, &["model", "model_slug", "modelSlug"]).or_else(|| {
        payload
            .get("collaboration_mode")
            .and_then(|collaboration_mode| collaboration_mode.get("settings"))
            .and_then(|settings| string_at_any(settings, &["model", "model_slug", "modelSlug"]))
    }))
}

pub fn parse_codex_line(line: &str) -> Result<Option<ParsedUsage>, serde_json::Error> {
    let value: Value = serde_json::from_str(line)?;
    let Some(payload) = value.get("payload").or_else(|| value.get("msg")) else {
        return Ok(None);
    };

    let payload_type = payload
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| value.get("type").and_then(Value::as_str));

    let event_has_token_count = payload
        .get("event")
        .and_then(Value::as_str)
        .map(|event| event.contains("token_count"))
        .unwrap_or(false);

    if payload_type != Some("token_count") && !event_has_token_count {
        return Ok(None);
    }

    let (usage, cumulative_usage, source_kind) = usage_source(payload);
    let bucket = bucket_from_usage(usage);
    let cumulative_bucket = cumulative_usage
        .map(bucket_from_usage)
        .filter(|bucket| bucket.total_tokens > 0);

    if bucket.total_tokens == 0 && cumulative_bucket.is_none() {
        return Ok(None);
    }

    Ok(Some(ParsedUsage {
        timestamp: parse_timestamp(&value),
        model: string_at_any(payload, &["model", "model_slug", "modelSlug"])
            .or_else(|| string_at_any(usage, &["model", "model_slug", "modelSlug"])),
        bucket,
        cumulative_bucket,
        source_kind,
    }))
}

fn bucket_from_usage(usage: &Value) -> TokenBucket {
    let input_tokens = number_at_any(
        usage,
        &[
            "input_tokens",
            "inputTokens",
            "prompt_tokens",
            "promptTokens",
        ],
    );
    let output_tokens = number_at_any(
        usage,
        &[
            "output_tokens",
            "outputTokens",
            "completion_tokens",
            "completionTokens",
        ],
    );
    TokenBucket {
        input_tokens,
        cached_input_tokens: number_at_any(
            usage,
            &[
                "cached_input_tokens",
                "cachedInputTokens",
                "cache_read_input_tokens",
                "cacheReadInputTokens",
            ],
        ),
        cache_read_tokens: 0,
        cache_creation_tokens: number_at_any(
            usage,
            &["cache_creation_input_tokens", "cacheCreationInputTokens"],
        ),
        output_tokens,
        total_tokens: codex_total_tokens(usage, input_tokens, output_tokens),
    }
}

fn number_at_any(value: &Value, keys: &[&str]) -> u64 {
    for key in keys {
        if let Some(number) = value.get(*key).and_then(Value::as_u64) {
            return number;
        }
    }
    0
}

fn usage_source(payload: &Value) -> (&Value, Option<&Value>, UsageSourceKind) {
    let info = payload.get("info");
    let total = info.and_then(|info| info.get("total_token_usage"));

    if let Some(usage) = info.and_then(|info| info.get("last_token_usage")) {
        return (usage, total, UsageSourceKind::Delta);
    }

    if let Some(usage) = total {
        return (usage, Some(usage), UsageSourceKind::Cumulative);
    }

    if let Some(usage) = payload.get("usage") {
        return (usage, None, UsageSourceKind::Delta);
    }

    (payload, None, UsageSourceKind::Delta)
}

fn codex_total_tokens(_usage: &Value, input_tokens: u64, output_tokens: u64) -> u64 {
    input_tokens + output_tokens
}

fn string_at_any(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = value.get(*key).and_then(Value::as_str) {
            return Some(text.to_owned());
        }
    }
    None
}
