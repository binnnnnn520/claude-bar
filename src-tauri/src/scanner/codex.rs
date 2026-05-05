use super::time::parse_timestamp;
use super::types::TokenBucket;
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ParsedUsage {
    pub timestamp: Option<DateTime<Utc>>,
    pub model: Option<String>,
    pub bucket: TokenBucket,
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

    let usage = usage_source(payload);
    let bucket = TokenBucket {
        input_tokens: number_at_any(
            usage,
            &[
                "input_tokens",
                "inputTokens",
                "prompt_tokens",
                "promptTokens",
            ],
        ),
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
        output_tokens: number_at_any(
            usage,
            &[
                "output_tokens",
                "outputTokens",
                "completion_tokens",
                "completionTokens",
            ],
        ),
        total_tokens: number_at_any(usage, &["total_tokens", "totalTokens"]),
    };

    let bucket = if bucket.total_tokens == 0 {
        bucket.with_total()
    } else {
        bucket
    };

    if bucket.total_tokens == 0 {
        return Ok(None);
    }

    Ok(Some(ParsedUsage {
        timestamp: parse_timestamp(&value),
        model: string_at_any(payload, &["model", "model_slug", "modelSlug"])
            .or_else(|| string_at_any(usage, &["model", "model_slug", "modelSlug"])),
        bucket,
    }))
}

fn number_at_any(value: &Value, keys: &[&str]) -> u64 {
    for key in keys {
        if let Some(number) = value.get(*key).and_then(Value::as_u64) {
            return number;
        }
    }
    0
}

fn usage_source(payload: &Value) -> &Value {
    let info = payload.get("info");

    // Per-row aggregation should use the increment for this row; total_token_usage
    // is cumulative and would over-count if summed across token_count events.
    info.and_then(|info| info.get("last_token_usage"))
        .or_else(|| info.and_then(|info| info.get("total_token_usage")))
        .or_else(|| payload.get("usage"))
        .unwrap_or(payload)
}

fn string_at_any(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = value.get(*key).and_then(Value::as_str) {
            return Some(text.to_owned());
        }
    }
    None
}
