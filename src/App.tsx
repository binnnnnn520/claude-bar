import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FloatingPopover } from "./components/FloatingPopover";
import { Dashboard } from "./components/Dashboard";
import type { UsageSnapshot } from "./types";

type UsageWindow = UsageSnapshot["window"];

const emptySnapshot: UsageSnapshot = {
  window: "last30d",
  scannedAt: new Date().toISOString(),
  providers: []
};

export default function App() {
  const [snapshot, setSnapshot] = useState<UsageSnapshot>(emptySnapshot);
  const [view, setView] = useState<"popover" | "dashboard">("popover");
  const [selectedWindow, setSelectedWindow] = useState<UsageWindow>("last30d");
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const refreshRequestId = useRef(0);

  const refresh = useCallback(async () => {
    const requestId = refreshRequestId.current + 1;
    refreshRequestId.current = requestId;
    setIsLoading(true);
    setError(null);
    try {
      const next = await invoke<UsageSnapshot>("scan_usage", { window: selectedWindow });
      if (refreshRequestId.current === requestId) {
        setSnapshot(next);
      }
    } catch (err) {
      if (refreshRequestId.current === requestId) {
        setError(err instanceof Error ? err.message : String(err));
      }
    } finally {
      if (refreshRequestId.current === requestId) {
        setIsLoading(false);
      }
    }
  }, [selectedWindow]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const content = useMemo(() => {
    if (view === "dashboard") {
      return (
        <Dashboard
          snapshot={snapshot}
          selectedWindow={selectedWindow}
          onWindowChange={setSelectedWindow}
          onRefresh={refresh}
          onBack={() => setView("popover")}
          isLoading={isLoading}
          error={error}
        />
      );
    }
    return (
      <FloatingPopover
        snapshot={snapshot}
        selectedWindow={selectedWindow}
        onWindowChange={setSelectedWindow}
        onRefresh={refresh}
        onOpenDashboard={() => setView("dashboard")}
        isLoading={isLoading}
        error={error}
      />
    );
  }, [view, snapshot, selectedWindow, refresh, isLoading, error]);

  return content;
}
