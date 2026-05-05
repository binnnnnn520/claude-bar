import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FloatingPopover } from "./components/FloatingPopover";
import type { UsageSnapshot } from "./types";

type UsageWindow = UsageSnapshot["window"];

const emptySnapshot: UsageSnapshot = {
  window: "last30d",
  scannedAt: new Date().toISOString(),
  providers: []
};

export default function App() {
  const [snapshot, setSnapshot] = useState<UsageSnapshot>(emptySnapshot);
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

  return (
    <FloatingPopover
      snapshot={snapshot}
      selectedWindow={selectedWindow}
      onWindowChange={setSelectedWindow}
      onRefresh={refresh}
      isLoading={isLoading}
      error={error}
    />
  );
}
