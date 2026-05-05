use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId {
    Codex,
    Claude,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenBucket {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

impl TokenBucket {
    pub fn with_total(mut self) -> Self {
        self.total_tokens = self.input_tokens
            + self.cached_input_tokens
            + self.cache_read_tokens
            + self.cache_creation_tokens
            + self.output_tokens;
        self
    }

    pub fn add(&mut self, other: &TokenBucket) {
        self.input_tokens += other.input_tokens;
        self.cached_input_tokens += other.cached_input_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
        self.cache_creation_tokens += other.cache_creation_tokens;
        self.output_tokens += other.output_tokens;
        self.total_tokens += other.total_tokens;
    }

    pub fn saturating_sub(&self, other: &TokenBucket) -> Self {
        Self {
            input_tokens: self.input_tokens.saturating_sub(other.input_tokens),
            cached_input_tokens: self
                .cached_input_tokens
                .saturating_sub(other.cached_input_tokens),
            cache_read_tokens: self.cache_read_tokens.saturating_sub(other.cache_read_tokens),
            cache_creation_tokens: self
                .cache_creation_tokens
                .saturating_sub(other.cache_creation_tokens),
            output_tokens: self.output_tokens.saturating_sub(other.output_tokens),
            total_tokens: self.total_tokens.saturating_sub(other.total_tokens),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyUsage {
    pub date: String,
    #[serde(flatten)]
    pub bucket: TokenBucket,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub model: String,
    #[serde(flatten)]
    pub bucket: TokenBucket,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsage {
    pub provider: ProviderId,
    #[serde(flatten)]
    pub bucket: TokenBucket,
    pub files_scanned: u64,
    pub files_with_usage: u64,
    pub parse_warnings: u64,
    pub daily: Vec<DailyUsage>,
    pub models: Vec<ModelUsage>,
    pub errors: Vec<String>,
    #[serde(skip)]
    pub daily_buckets: BTreeMap<String, TokenBucket>,
    #[serde(skip)]
    pub model_buckets: BTreeMap<String, TokenBucket>,
}

impl ProviderUsage {
    pub fn empty(provider: ProviderId) -> Self {
        Self {
            provider,
            bucket: TokenBucket::default(),
            files_scanned: 0,
            files_with_usage: 0,
            parse_warnings: 0,
            daily: Vec::new(),
            models: Vec::new(),
            errors: Vec::new(),
            daily_buckets: BTreeMap::new(),
            model_buckets: BTreeMap::new(),
        }
    }

    pub fn add_bucket(&mut self, bucket: TokenBucket) {
        self.bucket.add(&bucket);
    }

    pub fn add_daily_bucket(&mut self, date: String, bucket: TokenBucket) {
        self.add_bucket(bucket.clone());
        self.daily_buckets.entry(date).or_default().add(&bucket);
    }

    pub fn add_model_bucket(&mut self, model: String, bucket: TokenBucket) {
        self.model_buckets.entry(model).or_default().add(&bucket);
    }

    pub fn finalize(mut self) -> Self {
        self.daily = self
            .daily_buckets
            .iter()
            .map(|(date, bucket)| DailyUsage {
                date: date.clone(),
                bucket: bucket.clone(),
            })
            .collect();
        self.models = self
            .model_buckets
            .iter()
            .map(|(model, bucket)| ModelUsage {
                model: model.clone(),
                bucket: bucket.clone(),
            })
            .collect();
        self.models
            .sort_by(|a, b| b.bucket.total_tokens.cmp(&a.bucket.total_tokens));
        self
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    pub window: String,
    pub scanned_at: String,
    pub providers: Vec<ProviderUsage>,
}
