use super::time::parse_timestamp;
use super::types::TokenBucket;
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ParsedUsage {
    pub timestamp: Option<DateTime<Utc>>,
    pub model: Option<String>,
    pub dedupe_key: Option<String>,
    pub bucket: TokenBucket,
}

pub fn parse_claude_line(line: &str) -> Option<ParsedUsage> {
    let value: Value = serde_json::from_str(line).ok()?;
    if value.get("type").and_then(Value::as_str) != Some("assistant") {
        return None;
    }

    let message = value.get("message")?;
    let usage = message.get("usage")?;

    let bucket = TokenBucket {
        input_tokens: usage
            .get("input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        cached_input_tokens: 0,
        cache_read_tokens: usage
            .get("cache_read_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        cache_creation_tokens: usage
            .get("cache_creation_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        output_tokens: usage
            .get("output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        total_tokens: 0,
    }
    .with_total();

    if bucket.total_tokens == 0 {
        return None;
    }

    Some(ParsedUsage {
        timestamp: parse_timestamp(&value),
        model: message.get("model").and_then(Value::as_str).map(str::to_owned),
        dedupe_key: message
            .get("id")
            .and_then(Value::as_str)
            .or_else(|| value.get("requestId").and_then(Value::as_str))
            .map(str::to_owned),
        bucket,
    })
}
