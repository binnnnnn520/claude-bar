import type { DailyUsage, ModelUsage, ProviderId, ProviderUsage, TokenBucket, UsageSnapshot } from "../types";

type UsageWindow = UsageSnapshot["window"];

interface FloatingPopoverProps {
  snapshot: UsageSnapshot;
  selectedWindow: UsageWindow;
  onWindowChange: (window: UsageWindow) => void;
  onRefresh: () => void;
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

function windowLabel(window: UsageWindow): string {
  return WINDOWS.find((item) => item.id === window)?.label ?? window;
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

function ShareBar({ snapshot }: { snapshot: UsageSnapshot }) {
  const total = snapshotTotal(snapshot);
  const codex = provider(snapshot, "codex");
  const claude = provider(snapshot, "claude");
  const codexShare = total > 0 && codex ? (codex.totalTokens / total) * 100 : 0;
  const claudeShare = total > 0 && claude ? (claude.totalTokens / total) * 100 : 0;

  return (
    <div className="share-wrap">
      <div className="share-bar" aria-label="Provider token share">
        <span className="codex-fill" style={{ width: `${codexShare}%` }} />
        <span className="claude-fill" style={{ width: `${claudeShare}%` }} />
      </div>
      <div className="share-legend">
        <span>
          <i className="dot codex" />
          Codex {Math.round(codexShare)}%
        </span>
        <span>
          <i className="dot claude" />
          Claude {Math.round(claudeShare)}%
        </span>
      </div>
    </div>
  );
}

function SparkBars({ item }: { item?: ProviderUsage }) {
  const realDays = item?.daily.slice(-14) ?? [];
  const bars: Array<DailyUsage | null> =
    realDays.length > 0 ? realDays : Array.from({ length: 14 }, () => null);
  const max = Math.max(0, ...realDays.map((day) => day.totalTokens));

  return (
    <div className={`spark-bars ${item?.provider ?? "empty"}`} aria-label={`${item ? PROVIDER_LABEL[item.provider] : "Empty"} daily usage`}>
      {bars.map((day, index) => {
        const height = day && max > 0 ? Math.max(12, Math.round((day.totalTokens / max) * 100)) : 12;
        const title = day ? `${day.date}: ${formatTokens(day.totalTokens)}` : "No usage";
        return <i key={day?.date ?? `empty-${index}`} style={{ height: `${height}%` }} title={title} />;
      })}
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function ModelRows({ models }: { models: ModelUsage[] }) {
  const visible = models.slice(0, 5);
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
  const visible = days.slice(-7).reverse();
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
            <span>{day.date.slice(5)}</span>
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

function ProviderSection({ item, id }: { item?: ProviderUsage; id: ProviderId }) {
  const warnings = item?.parseWarnings ?? 0;
  const firstError = item?.errors[0];

  return (
    <section className={`provider-panel ${id}`} aria-label={`${PROVIDER_LABEL[id]} usage`}>
      <div className="provider-line">
        <div className="provider-name">
          <span className={`dot ${id}`} />
          {PROVIDER_LABEL[id]}
        </div>
        <SparkBars item={item} />
        <div className="provider-total">{formatTokens(item?.totalTokens ?? 0)}</div>
      </div>

      <div className="metric-grid">
        <Metric label="Total" value={formatTokens(item?.totalTokens ?? 0)} />
        <Metric label="Input" value={formatTokens(item?.inputTokens ?? 0)} />
        <Metric label="Cache" value={formatTokens(cacheTokens(item))} />
        <Metric label="Output" value={formatTokens(item?.outputTokens ?? 0)} />
      </div>

      <div className="provider-meta">
        <span>
          Files {formatNumber(item?.filesWithUsage ?? 0)}/{formatNumber(item?.filesScanned ?? 0)}
        </span>
        {warnings > 0 ? <span className="warn">Warnings {warnings}</span> : null}
      </div>

      {firstError ? (
        <div className="provider-error" title={firstError}>
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

export function FloatingPopover({
  snapshot,
  selectedWindow,
  onWindowChange,
  onRefresh,
  isLoading,
  error
}: FloatingPopoverProps) {
  const total = snapshotTotal(snapshot);

  return (
    <main className="popover-stage">
      <section className="floating-popover" aria-label="Local token popover" aria-busy={isLoading}>
        <header className="popover-head">
          <div className="title-group">
            <div className="ledger-glyph" aria-hidden="true">
              <span />
            </div>
            <div className="app-title">
              <strong>Token Ledger</strong>
              <span>{isLoading ? "Scanning" : `Scanned ${formatScanTime(snapshot.scannedAt)}`}</span>
            </div>
          </div>
          <button
            className="icon-button"
            title="Refresh usage"
            aria-label="Refresh usage"
            type="button"
            onClick={onRefresh}
            disabled={isLoading}
          >
            <span className={`refresh-icon ${isLoading ? "spinning" : ""}`} aria-hidden="true" />
          </button>
        </header>

        <div className="popover-body">
          <WindowSelector selectedWindow={selectedWindow} onWindowChange={onWindowChange} />

          <div className="total-block">
            <div className="total-copy">
              <span>{windowLabel(selectedWindow)} total</span>
              <strong>{formatTokens(total)}</strong>
            </div>
            <span className="local-badge">Local</span>
          </div>

          <ShareBar snapshot={snapshot} />

          {error ? <div className="notice danger" title={error}>{error}</div> : null}

          <div className="providers">
            {PROVIDERS.map((id) => (
              <ProviderSection item={provider(snapshot, id)} id={id} key={id} />
            ))}
          </div>
        </div>
      </section>
    </main>
  );
}
