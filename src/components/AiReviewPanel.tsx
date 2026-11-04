import { useCallback, useEffect, useRef, useState } from "react";
import { cancelAiReview, getAiReviewProgress, startAiReview } from "../lib/api";
import type { AiReviewProgress } from "../lib/types";
import { Button } from "./ui/button";

export function AiReviewPanel({ sessionId, onRefresh }: { sessionId: string; onRefresh: () => Promise<void> }) {
  const [progress, setProgress] = useState<AiReviewProgress | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const refresh = useRef(onRefresh);
  refresh.current = onRefresh;
  const running = progress?.status === "running";
  const poll = useCallback(async () => {
    const next = await getAiReviewProgress(sessionId);
    setProgress(next);
    return next;
  }, [sessionId]);

  useEffect(() => {
    let active = true;
    getAiReviewProgress(sessionId).then((next) => { if (active) setProgress(next); }).catch(() => {});
    return () => { active = false; };
  }, [sessionId]);

  useEffect(() => {
    if (!running) return;
    let active = true;
    let polling = false;
    const timer = window.setInterval(async () => {
      if (polling) return;
      polling = true;
      try {
        const next = await getAiReviewProgress(sessionId);
        if (!active) return;
        setProgress(next);
        if (next.status !== "running") await refresh.current();
      } catch (cause) { if (active) setError(String(cause)); } finally { polling = false; }
    }, 700);
    return () => { active = false; window.clearInterval(timer); };
  }, [running, sessionId]);

  const run = async () => {
    setBusy(true); setError(null);
    try { await startAiReview(sessionId); await poll(); await refresh.current(); }
    catch (cause) { setError(String(cause)); }
    finally { setBusy(false); }
  };
  const cancel = async () => {
    setBusy(true);
    try { await cancelAiReview(sessionId); await poll(); await refresh.current(); }
    catch (cause) { setError(String(cause)); }
    finally { setBusy(false); }
  };

  return <div className="space-y-2 border-b border-border bg-primary/5 px-6 py-3">
    <div className="flex flex-wrap items-center justify-between gap-3">
      <div><p className="text-sm font-medium">AI-assisted review · Local only · $0</p>
        <p className="text-xs text-muted-foreground">Rules and on-device analysis suggest keepers. Review every group before moving photos to Trash.</p></div>
      <Button size="sm" variant="secondary" disabled={busy} onClick={running ? cancel : run}>{running ? "Cancel analysis" : "Run AI review"}</Button>
    </div>
    {progress && <p className="text-xs text-muted-foreground" role="status">{progress.status === "running" ? "Analyzing" : progress.status === "complete" ? "Analysis complete" : progress.status === "cancelled" ? "Cancelled — completed suggestions are available" : "Analysis failed"} · {progress.groupsProcessed} / {progress.groupsTotal} groups</p>}
    {running && <progress aria-label="AI review progress" className="h-1 w-full accent-primary" value={progress?.groupsProcessed ?? 0} max={progress?.groupsTotal || 1} />}
    {(error || progress?.error) && <p role="alert" className="text-xs text-destructive">{error || progress?.error}</p>}
  </div>;
}
