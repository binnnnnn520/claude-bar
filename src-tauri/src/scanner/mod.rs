pub mod claude;
pub mod codex;
pub mod roots;
pub mod time;
pub mod types;

#[cfg(test)]
mod tests {
    use super::types::{ProviderId, ProviderUsage, TokenBucket};

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
