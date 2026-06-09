import { ZoomIn } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { CompareSlider } from "../components/CompareSlider";
import { ImageTile } from "../components/ImageTile";
import { Lightbox } from "../components/Lightbox";
import { Button } from "../components/ui/button";
import { useDuplicateGroups } from "../hooks/useDuplicateGroups";
import { keepAllInGroup, keepSelectedAndTrash, setKeepers } from "../lib/api";
import type { FileMember } from "../lib/types";
import { cn, formatBytes, kindLabel } from "../lib/utils";

function suggestKeeper(members: FileMember[]): number | null {
  if (members.length === 0) return null;
  const sorted = [...members].sort((a, b) => {
    const areaA = (a.width ?? 0) * (a.height ?? 0);
    const areaB = (b.width ?? 0) * (b.height ?? 0);
    if (areaA !== areaB) return areaB - areaA;
    const takenA = a.exif?.dateTaken ? new Date(a.exif.dateTaken).getTime() : Infinity;
    const takenB = b.exif?.dateTaken ? new Date(b.exif.dateTaken).getTime() : Infinity;
    return takenA - takenB;
  });
  return sorted[0]?.fileId ?? null;
}

type HistoryEntry = {
  groupId: number;
  keeperIds: number[];
  detail: FileMember[];
};

export function ReviewPage() {
  const { sessionId } = useParams();
  const navigate = useNavigate();
  const {
    groups,
    activeGroupId,
    setActiveGroupId,
    activeGroup,
    refreshGroups,
    selectNextGroup,
  } = useDuplicateGroups(sessionId ?? null);

  const [selectedIds, setSelectedIds] = useState<number[]>([]);
  const [focusedIndex, setFocusedIndex] = useState(0);
  const [lightboxMember, setLightboxMember] = useState<FileMember | null>(null);
  const [compareOpen, setCompareOpen] = useState(false);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [busy, setBusy] = useState(false);

  const suggestedKeeperId = useMemo(
    () => (activeGroup ? suggestKeeper(activeGroup.members) : null),
    [activeGroup],
  );

  const overlayOpen = lightboxMember !== null || compareOpen;

  useEffect(() => {
    setSelectedIds([]);
    setFocusedIndex(0);
    setLightboxMember(null);
    setCompareOpen(false);
  }, [activeGroupId]);

  const toggleSelect = useCallback((fileId: number) => {
    setSelectedIds((current) =>
      current.includes(fileId)
        ? current.filter((id) => id !== fileId)
        : [...current, fileId],
    );
  }, []);

  const selectAll = useCallback(() => {
    if (!activeGroup) return;
    setSelectedIds(activeGroup.members.map((member) => member.fileId));
  }, [activeGroup]);

  const pushHistory = useCallback((group: NonNullable<typeof activeGroup>) => {
    const keeperIds = group.members
      .filter((member) => member.isKeeper)
      .map((member) => member.fileId);
    setHistory((current) => [
      ...current,
      {
        groupId: group.id,
        keeperIds,
        detail: group.members,
      },
    ]);
  }, []);

  const keepSelected = useCallback(async () => {
    if (!activeGroup || selectedIds.length === 0 || busy) return;
    setBusy(true);
    try {
      pushHistory(activeGroup);
      await keepSelectedAndTrash(activeGroup.id, selectedIds);
      await refreshGroups();
      selectNextGroup();
    } finally {
      setBusy(false);
    }
  }, [activeGroup, busy, pushHistory, refreshGroups, selectNextGroup, selectedIds]);

  const keepAll = useCallback(async () => {
    if (!activeGroup || busy) return;
    setBusy(true);
    try {
      pushHistory(activeGroup);
      await keepAllInGroup(activeGroup.id);
      await refreshGroups();
      selectNextGroup();
    } finally {
      setBusy(false);
    }
  }, [activeGroup, busy, pushHistory, refreshGroups, selectNextGroup]);

  const undoLast = useCallback(async () => {
    const last = history[history.length - 1];
    if (!last || busy) return;
    setBusy(true);
    try {
      await setKeepers(last.groupId, last.keeperIds);
      setHistory((current) => current.slice(0, -1));
      setActiveGroupId(last.groupId);
    } finally {
      setBusy(false);
    }
  }, [busy, history, setActiveGroupId]);

  const focusNextMember = useCallback(() => {
    if (!activeGroup || activeGroup.members.length === 0) return;
    setFocusedIndex((current) => (current + 1) % activeGroup.members.length);
  }, [activeGroup]);

  const toggleFocusedSelect = useCallback(() => {
    if (!activeGroup || activeGroup.members.length === 0) return;
    const member = activeGroup.members[focusedIndex];
    if (member) toggleSelect(member.fileId);
  }, [activeGroup, focusedIndex, toggleSelect]);

  const openCompare = useCallback(() => {
    const count =
      activeGroup?.members.filter((member) => selectedIds.includes(member.fileId)).length ?? 0;
    if (count >= 2) setCompareOpen(true);
  }, [activeGroup, selectedIds]);

  useEffect(() => {
    const onKeyDown = async (event: KeyboardEvent) => {
      if (!activeGroup || overlayOpen) return;
      const meta = event.metaKey;

      if (event.code === "Space") {
        event.preventDefault();
        toggleFocusedSelect();
        return;
      }
      if (meta && event.key.toLowerCase() === "a") {
        event.preventDefault();
        selectAll();
        return;
      }
      if (meta && event.key.toLowerCase() === "s") {
        event.preventDefault();
        await keepSelected();
        return;
      }
      if (event.key === "Tab") {
        event.preventDefault();
        focusNextMember();
        return;
      }
      if (meta && event.key.toLowerCase() === "z") {
        event.preventDefault();
        await undoLast();
        return;
      }
      if (meta && event.key.toLowerCase() === "b") {
        event.preventDefault();
        openCompare();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [
    activeGroup,
    focusNextMember,
    keepSelected,
    openCompare,
    overlayOpen,
    selectAll,
    toggleFocusedSelect,
    undoLast,
  ]);

  const compareMembers =
    selectedIds.length > 0
      ? activeGroup?.members.filter((member) => selectedIds.includes(member.fileId)) ?? []
      : [];

  return (
    <div className="flex h-full min-h-0 overflow-hidden">
      <aside className="flex w-72 shrink-0 flex-col border-r border-border bg-card/40">
        <div className="shrink-0 border-b border-border p-4">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-muted-foreground">Duplicate groups</p>
              <p className="text-lg font-semibold">{groups.length} pending</p>
            </div>
            <Button variant="ghost" size="sm" onClick={() => navigate("/")}>
              Home
            </Button>
          </div>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto p-4 pt-3">
          <div className="space-y-2 pr-1">
            {groups.map((group) => (
              <button
                key={group.id}
                type="button"
                onClick={() => setActiveGroupId(group.id)}
                className={cn(
                  "w-full rounded-lg border px-3 py-3 text-left transition",
                  activeGroupId === group.id
                    ? "border-primary bg-primary/10"
                    : "border-border bg-card hover:border-primary/40",
                )}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="font-medium">{kindLabel(group.kind)}</span>
                  <span className="text-xs text-muted-foreground">
                    {Math.round(group.confidence * 100)}%
                  </span>
                </div>
                <p className="mt-1 text-sm text-muted-foreground">
                  {group.memberCount} photos · {formatBytes(group.bytesRecoverable)} recoverable
                </p>
              </button>
            ))}
          </div>
        </div>
      </aside>

      <main className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
        <header className="flex shrink-0 items-center justify-between border-b border-border px-6 py-4">
          <div>
            <p className="text-sm text-muted-foreground">Reviewing duplicates</p>
            <h1 className="text-xl font-semibold">
              {activeGroup ? kindLabel(activeGroup.kind) : "No groups left"}
            </h1>
          </div>
          <div className="flex gap-2">
            <Button
              variant="secondary"
              disabled={compareMembers.length < 2}
              onClick={openCompare}
            >
              <ZoomIn className="h-4 w-4" />
              Compare selected
            </Button>
          </div>
        </header>

        <div className="flex min-h-0 flex-1 overflow-hidden">
          <section className="min-h-0 min-w-0 flex-1 overflow-y-auto p-6">
            {activeGroup ? (
              <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">
                {activeGroup.members.map((member, index) => (
                  <ImageTile
                    key={member.fileId}
                    member={member}
                    selected={selectedIds.includes(member.fileId)}
                    focused={index === focusedIndex}
                    suggestedKeeper={member.fileId === suggestedKeeperId}
                    onToggleSelect={() => toggleSelect(member.fileId)}
                    onOpenZoom={() => setLightboxMember(member)}
                  />
                ))}
              </div>
            ) : (
              <div className="flex h-full items-center justify-center text-muted-foreground">
                All duplicate groups in this session have been reviewed.
              </div>
            )}
          </section>

          <aside className="flex w-72 shrink-0 flex-col border-l border-border bg-card/40">
            <div className="sticky top-0 flex flex-col gap-3 p-4">
              <Button
                size="lg"
                className="w-full"
                disabled={busy || selectedIds.length === 0}
                onClick={keepSelected}
              >
                Keep selected
              </Button>
              <Button
                size="lg"
                variant="secondary"
                className="w-full"
                disabled={busy || !activeGroup}
                onClick={keepAll}
              >
                Keep all
              </Button>
              <div className="rounded-lg bg-muted p-4 text-xs text-muted-foreground">
                <p className="font-medium text-foreground">Shortcuts</p>
                <p className="mt-2">Tab focus photo · Space toggle select</p>
                <p>⌘A select all · ⌘S keep selected · ⌘B compare</p>
                <p>⌘Z undo · Esc close zoom/compare</p>
              </div>
            </div>
          </aside>
        </div>
      </main>

      {lightboxMember && (
        <Lightbox member={lightboxMember} onClose={() => setLightboxMember(null)} />
      )}

      {compareOpen && compareMembers.length >= 2 && (
        <CompareSlider
          members={compareMembers}
          onClose={() => setCompareOpen(false)}
        />
      )}
    </div>
  );
}
