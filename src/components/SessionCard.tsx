import { FolderOpen, Play, Trash2 } from "lucide-react";
import type { SessionSummary } from "../lib/types";
import { formatDate } from "../lib/utils";
import { Button } from "./ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "./ui/card";

interface SessionCardProps {
  session: SessionSummary;
  onResume: () => void;
  onDelete: () => void;
}

function statusLabel(status: SessionSummary["status"]): string {
  switch (status) {
    case "scanning":
      return "Scanning";
    case "reviewing":
      return "Ready to review";
    case "paused":
      return "Paused";
    case "completed":
      return "Completed";
  }
}

export function SessionCard({ session, onResume, onDelete }: SessionCardProps) {
  return (
    <Card className="overflow-hidden">
      <CardHeader>
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <CardTitle className="truncate">{session.name}</CardTitle>
            <CardDescription className="truncate">{session.rootPath}</CardDescription>
          </div>
          <span className="rounded-full bg-muted px-3 py-1 text-xs font-medium">
            {statusLabel(session.status)}
          </span>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid grid-cols-3 gap-3 text-sm">
          <div className="rounded-lg bg-muted p-3">
            <p className="text-muted-foreground">Scanned</p>
            <p className="text-lg font-semibold">
              {session.filesScanned.toLocaleString()}
            </p>
          </div>
          <div className="rounded-lg bg-muted p-3">
            <p className="text-muted-foreground">Pending</p>
            <p className="text-lg font-semibold">
              {session.groupsPending.toLocaleString()}
            </p>
          </div>
          <div className="rounded-lg bg-muted p-3">
            <p className="text-muted-foreground">Groups</p>
            <p className="text-lg font-semibold">
              {session.groupsTotal.toLocaleString()}
            </p>
          </div>
        </div>
        <p className="text-xs text-muted-foreground">
          Last updated {formatDate(session.updatedAt)}
        </p>
        <div className="flex gap-2">
          <Button className="flex-1" onClick={onResume}>
            <Play className="h-4 w-4" />
            Resume
          </Button>
          <Button variant="outline" size="icon" onClick={onDelete}>
            <Trash2 className="h-4 w-4" />
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

export function NewScanCard({ onStart }: { onStart: () => void }) {
  return (
    <button
      type="button"
      onClick={onStart}
      className="flex h-full min-h-[220px] flex-col items-center justify-center rounded-xl border border-dashed border-border bg-card/50 p-8 text-center transition hover:border-primary hover:bg-card"
    >
      <FolderOpen className="mb-4 h-10 w-10 text-primary" />
      <p className="text-lg font-medium">Start a new scan</p>
      <p className="mt-2 max-w-sm text-sm text-muted-foreground">
        Choose a folder on your Mac or an external drive. Scanning is read-only and
        never changes your files.
      </p>
    </button>
  );
}
