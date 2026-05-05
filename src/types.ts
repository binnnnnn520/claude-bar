export type ProviderId = "codex" | "claude";

export interface TokenBucket {
  inputTokens: number;
  cachedInputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  outputTokens: number;
  totalTokens: number;
}

export interface DailyUsage extends TokenBucket {
  date: string;
}

export interface ModelUsage extends TokenBucket {
  model: string;
}

export interface ProviderUsage extends TokenBucket {
  provider: ProviderId;
  filesScanned: number;
  filesWithUsage: number;
  parseWarnings: number;
  daily: DailyUsage[];
  models: ModelUsage[];
  errors: string[];
}

export interface UsageSnapshot {
  window: "last30d" | "month" | "today";
  scannedAt: string;
  providers: ProviderUsage[];
}
