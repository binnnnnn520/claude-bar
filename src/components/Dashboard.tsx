import type { DailyUsage, ModelUsage, ProviderId, ProviderUsage, TokenBucket, UsageSnapshot } from "../types";

type UsageWindow = UsageSnapshot["window"];

interface DashboardProps {
  snapshot: UsageSnapshot;
  selectedWindow: UsageWindow;
  onWindowChange: (window: UsageWindow) => void;
  onRefresh: () => void;
  onBack: () => void;
  isLoading: boolean;
  error: string | null;
}

const WINDOWS: Array<{ id: UsageWindow; label: string; title: string }> = [
  { id: "last30d", label: "30 days", title: "Show the last 30 days" },
  { id: "month", label: "Month", title: "Show this month" },
  { id: "today", label: "Today", title: "Show today" }
];

const PROVIDERS: ProviderId[] = ["codex", "claude"];

const PROVIDER_LABEL: Record<ProviderId, string> = {
  codex: "Codex",
  claude: "Claude"
};

type TokenShape = Pick<
  TokenBucket,
  "cachedInputTokens" | "cacheReadTokens" | "cacheCreationTokens" | "inputTokens" | "outputTokens" | "totalTokens"
>;

function cacheTokens(bucket?: TokenShape): number {
  if (!bucket) {
    return 0;
  }
  return bucket.cachedInputTokens + bucket.cacheReadTokens + bucket.cacheCreationTokens;
}

function formatTokens(value: number): string {
  if (!Number.isFinite(value) || value <= 0) {
    return "0";
  }
  return new Intl.NumberFormat("en", {
    notation: "compact",
    maximumFractionDigits: value >= 1_000_000 ? 2 : 1
  }).format(value);
}

function formatNumber(value: number): string {
  return new Intl.NumberFormat("en").format(value);
}

function formatScanTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "Not scanned";
  }
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit"
  }).format(date);
}

function provider(snapshot: UsageSnapshot, id: ProviderId): ProviderUsage | undefined {
  return snapshot.providers.find((item) => item.provider === id);
}

function snapshotTotal(snapshot: UsageSnapshot): number {
  return snapshot.providers.reduce((sum, item) => sum + item.totalTokens, 0);
}

function WindowSelector({
  selectedWindow,
  onWindowChange
}: {
  selectedWindow: UsageWindow;
  onWindowChange: (window: UsageWindow) => void;
}) {
  return (
    <div className="window-tabs" aria-label="Usage window">
      {WINDOWS.map((item) => (
        <button
          className={item.id === selectedWindow ? "active" : ""}
          key={item.id}
          type="button"
          title={item.title}
          aria-pressed={item.id === selectedWindow}
          onClick={() => onWindowChange(item.id)}
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}

function SummaryMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="summary-metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function TokenRows({ item }: { item?: TokenShape }) {
  return (
    <div className="token-rows">
      <div>
        <span>Total</span>
        <strong>{formatTokens(item?.totalTokens ?? 0)}</strong>
      </div>
      <div>
        <span>Input</span>
        <strong>{formatTokens(item?.inputTokens ?? 0)}</strong>
      </div>
      <div>
        <span>Cache</span>
        <strong>{formatTokens(cacheTokens(item))}</strong>
      </div>
      <div>
        <span>Output</span>
        <strong>{formatTokens(item?.outputTokens ?? 0)}</strong>
      </div>
    </div>
  );
}

function ModelRows({ models }: { models: ModelUsage[] }) {
  const visible = models.slice(0, 6);
  if (visible.length === 0) {
    return <div className="empty-row">No model rows</div>;
  }

  return (
    <div className="model-list">
      {visible.map((model) => (
        <div className="model-row" key={model.model}>
          <span title={model.model}>{model.model}</span>
          <strong>{formatTokens(model.totalTokens)}</strong>
        </div>
      ))}
    </div>
  );
}

function DailyRows({ days }: { days: DailyUsage[] }) {
  const visible = days.slice(-10).reverse();
  const max = Math.max(0, ...visible.map((day) => day.totalTokens));

  if (visible.length === 0) {
    return <div className="empty-row">No daily rows</div>;
  }

  return (
    <div className="daily-list">
      {visible.map((day) => {
        const width = max > 0 ? Math.max(4, Math.round((day.totalTokens / max) * 100)) : 0;
        return (
          <div className="daily-row" key={day.date}>
            <span>{day.date}</span>
            <div className="daily-track" aria-hidden="true">
              <i style={{ width: `${width}%` }} />
            </div>
            <strong>{formatTokens(day.totalTokens)}</strong>
          </div>
        );
      })}
    </div>
  );
}

function ProviderCard({ item, id }: { item?: ProviderUsage; id: ProviderId }) {
  const warnings = item?.parseWarnings ?? 0;
  const firstError = item?.errors[0];

  return (
    <section className={`dashboard-provider ${id}`} aria-label={`${PROVIDER_LABEL[id]} detail`}>
      <div className="provider-card-head">
        <div className="provider-name">
          <span className={`dot ${id}`} />
          {PROVIDER_LABEL[id]}
        </div>
        <strong>{formatTokens(item?.totalTokens ?? 0)}</strong>
      </div>

      <TokenRows item={item} />

      <div className="scan-line">
        <span>Files</span>
        <strong>
          {formatNumber(item?.filesWithUsage ?? 0)} / {formatNumber(item?.filesScanned ?? 0)}
        </strong>
      </div>
      {warnings > 0 ? (
        <div className="scan-line warn">
          <span>Warnings</span>
          <strong>{formatNumber(warnings)}</strong>
        </div>
      ) : null}
      {firstError ? (
        <div className="provider-error dashboard-error" title={firstError}>
          Error: {firstError}
        </div>
      ) : null}

      <div className="detail-columns">
        <div>
          <h2>Models</h2>
          <ModelRows models={item?.models ?? []} />
        </div>
        <div>
          <h2>Daily</h2>
          <DailyRows days={item?.daily ?? []} />
        </div>
      </div>
    </section>
  );
}

export function Dashboard({
  snapshot,
  selectedWindow,
  onWindowChange,
  onRefresh,
  onBack,
  isLoading,
  error
}: DashboardProps) {
  const total = snapshotTotal(snapshot);
  const codex = provider(snapshot, "codex");
  const claude = provider(snapshot, "claude");

  return (
    <main className="dashboard-stage" aria-busy={isLoading}>
      <section className="dashboard-shell" aria-label="Token dashboard">
        <header className="dashboard-head">
          <div className="title-group">
            <button className="back-button" type="button" onClick={onBack} title="Back to popover" aria-label="Back to popover">
              <span className="back-icon" aria-hidden="true" />
            </button>
            <div className="app-title">
              <strong>Token Ledger</strong>
              <span>{isLoading ? "Scanning" : `Scanned ${formatScanTime(snapshot.scannedAt)}`}</span>
            </div>
          </div>
          <div className="dashboard-actions">
            <WindowSelector selectedWindow={selectedWindow} onWindowChange={onWindowChange} />
            <button className="secondary-button" type="button" onClick={onRefresh} disabled={isLoading} title="Refresh usage">
              Refresh
            </button>
          </div>
        </header>

        <div className="dashboard-body">
          {error ? <div className="notice danger" title={error}>{error}</div> : null}

          <div className="summary-strip">
            <SummaryMetric label="Total" value={formatTokens(total)} />
            <SummaryMetric label="Codex" value={formatTokens(codex?.totalTokens ?? 0)} />
            <SummaryMetric label="Claude" value={formatTokens(claude?.totalTokens ?? 0)} />
            <SummaryMetric label="Files with usage" value={formatNumber(snapshot.providers.reduce((sum, item) => sum + item.filesWithUsage, 0))} />
          </div>

          <div className="dashboard-grid">
            {PROVIDERS.map((id) => (
              <ProviderCard item={provider(snapshot, id)} id={id} key={id} />
            ))}
          </div>
        </div>
      </section>
    </main>
  );
}
