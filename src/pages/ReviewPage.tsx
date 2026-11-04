import { withConfirmation } from "../lib/confirmation";
import { ZoomIn } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { AiReviewPanel } from "../components/AiReviewPanel";
import { suggestKeeper } from "../lib/suggestKeeper";
import { bucketLabels, initialKeeperSelection } from "../lib/keeperSelection";
import type { AiBucket } from "../lib/types";
import { CompareSlider } from "../components/CompareSlider";
import { ImageTile } from "../components/ImageTile";
import { Lightbox } from "../components/Lightbox";
import { Button } from "../components/ui/button";
import { useDuplicateGroups } from "../hooks/useDuplicateGroups";
import { keepAllInGroup, keepSelectedAndTrash, setKeepers } from "../lib/api";
import type { FileMember } from "../lib/types";
import { cn, formatBytes, kindLabel } from "../lib/utils";


type HistoryEntry = {
  groupId: number;
  keeperIds: number[];
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
    loading,
    error: loadError,
  } = useDuplicateGroups(sessionId ?? null);

  const [selectedIds, setSelectedIds] = useState<number[]>([]);
  const [focusedIndex, setFocusedIndex] = useState(0);
  const [lightboxMember, setLightboxMember] = useState<FileMember | null>(null);
  const [compareOpen, setCompareOpen] = useState(false);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [bucket, setBucket] = useState<AiBucket | "all" | "not_analyzed">("all");
  const drafts = useRef(new Map<number, number[]>());
  const visibleGroups = groups.filter((group) => bucket === "all" || (group.aiBucket ?? "not_analyzed") === bucket);

  useEffect(() => { drafts.current.clear(); setSelectedIds([]); setHistory([]); }, [sessionId]);

  useEffect(() => {
    if (!visibleGroups.some((group) => group.id === activeGroupId)) {
      setActiveGroupId(visibleGroups[0]?.id ?? null);
    }
  }, [bucket, groups, activeGroupId, setActiveGroupId]);

  useEffect(() => {
    if (!activeGroup) return;
    setSelectedIds(drafts.current.get(activeGroup.id) ?? initialKeeperSelection(activeGroup));
  }, [activeGroup]);

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

  const updateSelection = useCallback((next: number[]) => {
    if (!activeGroup) return;
    drafts.current.set(activeGroup.id, next);
    setSelectedIds(next);
  }, [activeGroup]);

  const toggleSelect = useCallback((fileId: number) => {
    if (activeGroup?.members.find((member) => member.fileId === fileId)?.protected?.length && selectedIds.includes(fileId)) return;
    updateSelection(selectedIds.includes(fileId) ? selectedIds.filter((id) => id !== fileId) : [...selectedIds, fileId]);
  }, [activeGroup, selectedIds, updateSelection]);

  const selectAll = useCallback(() => {
    if (activeGroup) updateSelection(activeGroup.members.map((member) => member.fileId));
  }, [activeGroup, updateSelection]);

  const pushHistory = useCallback((group: NonNullable<typeof activeGroup>) => {
    const keeperIds = group.members
      .filter((member) => member.isKeeper)
      .map((member) => member.fileId);
    setHistory((current) => [
      ...current,
      {
        groupId: group.id,
        keeperIds,
      },
    ]);
  }, []);

  const keepSelected = useCallback(async () => {
    if (!activeGroup || selectedIds.length === 0 || busy) return;
    const keepers = [...new Set([...selectedIds, ...activeGroup.members.filter((member) => member.protected?.length).map((member) => member.fileId)])];
    const discarded = activeGroup.members.filter((member) => !keepers.includes(member.fileId));
    setBusy(true);
    setError(null);
    try {
      await withConfirmation(`${activeGroup.aiVerdict ? "Review AI suggestions before continuing.\n\n" : ""}Keep ${keepers.length} photos and move ${discarded.length} photos (${formatBytes(discarded.reduce((sum, member) => sum + member.size + (member.companionRawSize ?? 0), 0))}) to macOS Trash? RAW companions move with their photos. Restore trashed files with Finder’s Put Back; undo only restores keeper flags.`, { title: "Confirm moving photos to Trash", kind: "warning", okLabel: "Move to Trash", cancelLabel: "Cancel" }, async () => {
      const result = await keepSelectedAndTrash(activeGroup.id, keepers);
      if (result.errors.length) setError(result.errors.join("\n"));
      pushHistory(activeGroup);
      await refreshGroups();
      });
    } catch (cause) { setError(String(cause)); } finally {
      setBusy(false);
    }
  }, [activeGroup, busy, pushHistory, refreshGroups, selectedIds]);

  const keepAll = useCallback(async () => {
    if (!activeGroup || busy) return;
    setBusy(true);
    try {
      await keepAllInGroup(activeGroup.id);
      pushHistory(activeGroup);
      await refreshGroups();
    } catch (cause) { setError(String(cause)); } finally {
      setBusy(false);
    }
  }, [activeGroup, busy, pushHistory, refreshGroups]);

  const undoLast = useCallback(async () => {
    const last = history[history.length - 1];
    if (!last || busy) return;
    setBusy(true);
    try {
      await setKeepers(last.groupId, last.keeperIds);
      drafts.current.delete(last.groupId);
      await refreshGroups();
      setHistory((current) => current.slice(0, -1));
      setActiveGroupId(last.groupId);
    } catch (cause) { setError(String(cause)); } finally {
      setBusy(false);
    }
  }, [busy, history, refreshGroups, setActiveGroupId]);

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
      if (!activeGroup || busy || overlayOpen || (event.target instanceof HTMLElement && (event.target.closest("input, textarea, select, button, [contenteditable=true], [role=dialog]")))) return;
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
    busy,
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
        <div className="px-4 pt-3"><label className="text-xs text-muted-foreground" htmlFor="bucket-filter">Review bucket</label><select id="bucket-filter" value={bucket} onChange={(event) => setBucket(event.target.value as typeof bucket)} className="mt-1 w-full rounded-md border border-border bg-background p-2 text-sm"><option value="all">All groups ({groups.length})</option>{Object.entries(bucketLabels).map(([key, label]) => <option key={key} value={key}>{label} ({groups.filter((group) => (group.aiBucket ?? "not_analyzed") === key).length})</option>)}</select></div>
        <div className="min-h-0 flex-1 overflow-y-auto p-4 pt-3">
          <div className="space-y-2 pr-1">
            {visibleGroups.map((group) => (
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
                <p className="mt-1 text-xs text-primary">{bucketLabels[group.aiBucket ?? "not_analyzed"]}</p>
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
              {activeGroup ? kindLabel(activeGroup.kind) : groups.length === 0 ? "No groups left" : "Review groups"}
            </h1>
          </div>
          <div className="flex gap-2">
            <Button variant="ghost" onClick={() => navigate("/settings")}>AI settings</Button>
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

        {sessionId && <AiReviewPanel key={sessionId} sessionId={sessionId} onRefresh={refreshGroups} />}
        {(error || loadError) && <p role="alert" className="px-6 py-2 text-sm text-destructive">{error || loadError}</p>}
        {activeGroup?.aiVerdict && <div className="border-b border-border px-6 py-3 text-sm"><span className="font-medium">{bucketLabels[activeGroup.aiBucket ?? "not_analyzed"]}</span><span className="ml-2 text-muted-foreground">{activeGroup.aiVerdict.reason}</span><p className="mt-1 text-xs text-muted-foreground">{activeGroup.aiVerdict.decidingTier} · Selection means keep. You can adjust suggestions before confirming.</p></div>}
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
                    suggestedKeeper={activeGroup.aiVerdict ? member.ai?.decision === "keep" : member.fileId === suggestedKeeperId}
                    onToggleSelect={() => toggleSelect(member.fileId)}
                    onOpenZoom={() => setLightboxMember(member)}
                  />
                ))}
              </div>
            ) : (
              <div className="flex h-full items-center justify-center text-muted-foreground">
                {loading ? "Loading groups…" : loadError ? "Could not load groups. Return Home and reopen this session to retry." : groups.length === 0 ? "All duplicate groups in this session have been reviewed." : visibleGroups.length === 0 ? "No groups in this bucket. Choose another filter." : "Loading group…"}
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
                Keep selected · Trash rest
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
              <Button variant="ghost" disabled={busy || history.length === 0} onClick={undoLast}>Undo keeper flags</Button>
              <p className="text-xs text-muted-foreground">Undo restores keeper flags only. To restore photos from Trash, use Finder’s Put Back.</p>
              <div className="rounded-lg bg-muted p-4 text-xs text-muted-foreground">
                <p className="font-medium text-foreground">Shortcuts</p>
                <p className="mt-2">Tab focus photo · Space toggle select</p>
                <p>⌘A select all · ⌘S keep selected · ⌘B compare</p>
                <p>⌘Z undo keeper flags · Esc close zoom/compare</p>
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
