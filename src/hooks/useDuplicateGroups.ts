import { useCallback, useEffect, useRef, useState } from "react";
import { getGroupDetail, listDuplicateGroups } from "../lib/api";
import type { DuplicateGroupDetail, DuplicateGroupSummary } from "../lib/types";

export function useDuplicateGroups(sessionId: string | null) {
  const [groups, setGroups] = useState<DuplicateGroupSummary[]>([]);
  const [activeGroupId, setActiveGroupId] = useState<number | null>(null);
  const [activeGroup, setActiveGroup] = useState<DuplicateGroupDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const generation = useRef(0);
  const activeId = useRef(activeGroupId);
  activeId.current = activeGroupId;

  useEffect(() => {
    generation.current += 1;
    setError(null);
    setGroups([]);
    setActiveGroupId(null);
    setActiveGroup(null);
  }, [sessionId]);

  const refreshGroups = useCallback(async () => {
    if (!sessionId) return;
    const currentGeneration = generation.current;
    setLoading(true);
    setError(null);
    try {
      // Fetch every page so filters and AI results cover the whole session.
      const next: DuplicateGroupSummary[] = [];
      for (let offset = 0; ; offset += 100) {
        const page = await listDuplicateGroups(sessionId, "pending", 100, offset);
        next.push(...page);
        if (page.length < 100) break;
      }
      if (generation.current !== currentGeneration) return;
      setGroups(next);
      setActiveGroupId((current) => next.some((group) => group.id === current) ? current : next[0]?.id ?? null);
      setRevision((current) => current + 1);
    } catch (cause) {
      if (generation.current === currentGeneration) setError(String(cause));
    } finally {
      if (generation.current === currentGeneration) setLoading(false);
    }
  }, [sessionId]);

  useEffect(() => { void refreshGroups(); }, [refreshGroups]);

  useEffect(() => {
    let cancelled = false;
    setActiveGroup(null);
    if (activeGroupId !== null) {
      getGroupDetail(activeGroupId).then((detail) => {
        if (!cancelled && activeId.current === detail.id) setActiveGroup(detail);
      }).catch((cause) => { if (!cancelled) setError(String(cause)); });
    }
    return () => { cancelled = true; };
  }, [activeGroupId, revision, sessionId]);

  const selectNextGroup = useCallback(() => {
    if (activeGroupId === null || groups.length === 0) return;
    const index = groups.findIndex((group) => group.id === activeGroupId);
    setActiveGroupId((groups[index + 1] ?? groups[0])?.id ?? null);
  }, [activeGroupId, groups]);

  return { groups, activeGroupId, setActiveGroupId, activeGroup, loading, error, refreshGroups, selectNextGroup };
}
