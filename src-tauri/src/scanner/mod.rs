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

        assert_eq!(usage.total_tokens, 17);
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.cached_input_tokens, 2);
        assert_eq!(usage.output_tokens, 5);
    }
}
