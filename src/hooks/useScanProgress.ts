import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { getScanProgress } from "../lib/api";
import type { ScanProgress } from "../lib/types";

export function useScanProgress(sessionId: string | null) {
  const [progress, setProgress] = useState<ScanProgress | null>(null);

  useEffect(() => {
    if (!sessionId) return;

    let active = true;
    getScanProgress(sessionId).then((value) => {
      if (active) setProgress(value);
    });

    const unlistenPromise = listen<ScanProgress>("scan:progress", (event) => {
      if (event.payload.sessionId === sessionId) {
        setProgress(event.payload);
      }
    });

    return () => {
      active = false;
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [sessionId]);

  return progress;
}
