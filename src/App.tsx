import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FloatingPopover } from "./components/FloatingPopover";
import { Dashboard } from "./components/Dashboard";
import type { UsageSnapshot } from "./types";

const emptySnapshot: UsageSnapshot = {
  window: "last30d",
  scannedAt: new Date().toISOString(),
  providers: []
};

export default function App() {
  const [snapshot, setSnapshot] = useState<UsageSnapshot>(emptySnapshot);
  const [view, setView] = useState<"popover" | "dashboard">("popover");
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    setIsLoading(true);
    setError(null);
    try {
      const next = await invoke<UsageSnapshot>("scan_usage", { window: "last30d" });
      setSnapshot(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  const content = useMemo(() => {
    if (view === "dashboard") {
      return <Dashboard snapshot={snapshot} onRefresh={refresh} onBack={() => setView("popover")} isLoading={isLoading} error={error} />;
    }
    return <FloatingPopover snapshot={snapshot} onRefresh={refresh} onOpenDashboard={() => setView("dashboard")} isLoading={isLoading} error={error} />;
  }, [view, snapshot, isLoading, error]);

  return content;
}
