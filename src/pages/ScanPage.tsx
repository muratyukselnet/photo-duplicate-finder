import { listen } from "@tauri-apps/api/event";
import { Pause, Play, Square } from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { Button } from "../components/ui/button";
import { pauseScan, resumeScan, startScan, stopScan } from "../lib/api";
import { useScanProgress } from "../hooks/useScanProgress";

const PHASE_LABELS: Record<string, string> = {
  walking: "Walking directories",
  hashing: "Hashing photos",
  clustering: "Finding duplicate groups",
  complete: "Scan complete",
};

export function ScanPage() {
  const { sessionId } = useParams();
  const navigate = useNavigate();
  const progress = useScanProgress(sessionId ?? null);
  const [scanning, setScanning] = useState(false);
  const [paused, setPaused] = useState(false);

  useEffect(() => {
    if (!sessionId) return;

    let active = true;
    setScanning(true);
    startScan(sessionId)
      .catch(console.error)
      .finally(() => {
        if (active) setScanning(false);
      });

    const unlisten = listen<string>("scan:complete", (event) => {
      if (event.payload === sessionId) {
        navigate(`/review/${sessionId}`);
      }
    });

    return () => {
      active = false;
      unlisten.then((fn) => fn());
    };
  }, [navigate, sessionId]);

  if (!sessionId) return null;

  const total = progress?.filesTotalEstimate ?? 0;
  const processed = progress?.filesProcessed ?? 0;
  const percent = total > 0 ? Math.min(100, (processed / total) * 100) : 0;

  return (
    <div className="mx-auto flex min-h-full max-w-3xl flex-col justify-center gap-8 p-8">
      <div className="space-y-3 text-center">
        <p className="text-sm uppercase tracking-[0.2em] text-primary">Scanning</p>
        <h1 className="text-3xl font-semibold">Collecting photo fingerprints</h1>
        <p className="text-muted-foreground">
          Scanning only — no files will be changed.
        </p>
      </div>

      <div className="rounded-2xl border border-border bg-card p-8">
        <div className="mb-3 flex items-center justify-between text-sm">
          <span>{PHASE_LABELS[progress?.phase ?? "walking"]}</span>
          <span>{percent.toFixed(1)}%</span>
        </div>
        <div className="h-3 overflow-hidden rounded-full bg-muted">
          <div
            className="h-full rounded-full bg-primary transition-all"
            style={{ width: `${percent}%` }}
          />
        </div>

        <div className="mt-6 grid grid-cols-3 gap-4 text-center text-sm">
          <div>
            <p className="text-muted-foreground">Processed</p>
            <p className="text-xl font-semibold">{processed.toLocaleString()}</p>
          </div>
          <div>
            <p className="text-muted-foreground">Estimated total</p>
            <p className="text-xl font-semibold">{total.toLocaleString()}</p>
          </div>
          <div>
            <p className="text-muted-foreground">Speed</p>
            <p className="text-xl font-semibold">
              {(progress?.filesPerSec ?? 0).toFixed(0)}/s
            </p>
          </div>
        </div>

        {progress?.currentPath && (
          <p className="mt-6 truncate text-xs text-muted-foreground">
            {progress.currentPath}
          </p>
        )}

        <div className="mt-8 flex justify-center gap-3">
          {!paused ? (
            <Button
              variant="secondary"
              onClick={async () => {
                await pauseScan(sessionId);
                setPaused(true);
              }}
              disabled={!scanning}
            >
              <Pause className="h-4 w-4" />
              Pause
            </Button>
          ) : (
            <Button
              onClick={async () => {
                setPaused(false);
                await resumeScan(sessionId);
              }}
            >
              <Play className="h-4 w-4" />
              Resume
            </Button>
          )}
          <Button
            variant="outline"
            onClick={async () => {
              await stopScan(sessionId);
              navigate("/");
            }}
          >
            <Square className="h-4 w-4" />
            Stop & save
          </Button>
        </div>
      </div>
    </div>
  );
}
