import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { NewScanCard, SessionCard } from "../components/SessionCard";
import { Button } from "../components/ui/button";
import {
  createSession,
  deleteSession,
  listSessions,
  pickDirectory,
} from "../lib/api";
import type { ScanPreset, SessionSummary } from "../lib/types";

const PRESETS: { id: ScanPreset; label: string; description: string }[] = [
  {
    id: "exact_only",
    label: "Exact only",
    description: "Fast pass for byte-identical copies.",
  },
  {
    id: "visual_similar",
    label: "Visual similar",
    description: "Default photographer workflow with perceptual matching.",
  },
  {
    id: "burst_time_window",
    label: "Burst / time window",
    description: "Group burst shots taken within a few seconds.",
  },
];

export function HomePage() {
  const navigate = useNavigate();
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [preset, setPreset] = useState<ScanPreset>("visual_similar");
  const [includeRaw, setIncludeRaw] = useState(false);
  const [busy, setBusy] = useState(false);

  const loadSessions = async () => {
    const data = await listSessions();
    setSessions(data);
  };

  useEffect(() => {
    loadSessions();
  }, []);

  const resumeSession = (session: SessionSummary) => {
    if (session.status === "scanning" || session.status === "paused") {
      navigate(`/scan/${session.id}`);
      return;
    }
    navigate(`/review/${session.id}`);
  };

  const startNewScan = async () => {
    setBusy(true);
    try {
      const directory = await pickDirectory();
      if (!directory) return;
      const sessionId = await createSession(directory, preset, undefined, {
        includeRaw,
      });
      navigate(`/scan/${sessionId}`);
    } finally {
      setBusy(false);
    }
  };

  const removeSession = async (sessionId: string) => {
    await deleteSession(sessionId);
    await loadSessions();
  };

  return (
    <div className="mx-auto flex min-h-full max-w-6xl flex-col gap-8 p-8">
      <header className="space-y-2">
        <p className="text-sm uppercase tracking-[0.2em] text-primary">
          Photo Duplicate Finder
        </p>
        <h1 className="text-4xl font-semibold tracking-tight">
          Resume where you left off
        </h1>
        <p className="max-w-2xl text-muted-foreground">
          Scan large libraries safely, review duplicate candidates side by side, and
          move unwanted copies to Trash without touching your keepers.
        </p>
      </header>

      <section className="space-y-4">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <h2 className="text-lg font-medium">Scan preset</h2>
          <div className="flex flex-wrap items-center gap-4">
            <label className="flex cursor-pointer items-center gap-2 text-sm text-foreground">
              <input
                type="checkbox"
                checked={includeRaw}
                onChange={(event) => setIncludeRaw(event.target.checked)}
                disabled={busy}
                className="h-4 w-4 rounded border-border accent-primary"
              />
              Include raw files
            </label>
            <Button disabled={busy} onClick={startNewScan}>
              Choose folder
            </Button>
          </div>
        </div>
        <div className="grid gap-3 md:grid-cols-3">
          {PRESETS.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => setPreset(item.id)}
              className={`rounded-xl border p-4 text-left transition ${
                preset === item.id
                  ? "border-primary bg-primary/10"
                  : "border-border bg-card hover:border-primary/50"
              }`}
            >
              <p className="font-medium">{item.label}</p>
              <p className="mt-1 text-sm text-muted-foreground">{item.description}</p>
            </button>
          ))}
        </div>
      </section>

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
        <NewScanCard onStart={startNewScan} />
        {sessions.map((session) => (
          <SessionCard
            key={session.id}
            session={session}
            onResume={() => resumeSession(session)}
            onDelete={() => removeSession(session.id)}
          />
        ))}
      </section>
    </div>
  );
}
