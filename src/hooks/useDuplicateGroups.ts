import { useCallback, useEffect, useState } from "react";
import { getGroupDetail, listDuplicateGroups } from "../lib/api";
import type { DuplicateGroupDetail, DuplicateGroupSummary } from "../lib/types";

export function useDuplicateGroups(sessionId: string | null) {
  const [groups, setGroups] = useState<DuplicateGroupSummary[]>([]);
  const [activeGroupId, setActiveGroupId] = useState<number | null>(null);
  const [activeGroup, setActiveGroup] = useState<DuplicateGroupDetail | null>(null);
  const [loading, setLoading] = useState(false);

  const refreshGroups = useCallback(async () => {
    if (!sessionId) return;
    setLoading(true);
    try {
      const next = await listDuplicateGroups(sessionId, "pending");
      setGroups(next);
      if (next.length === 0) {
        setActiveGroupId(null);
        setActiveGroup(null);
      } else if (!next.some((group) => group.id === activeGroupId)) {
        setActiveGroupId(next[0].id);
      }
    } finally {
      setLoading(false);
    }
  }, [sessionId, activeGroupId]);

  useEffect(() => {
    refreshGroups();
  }, [refreshGroups]);

  useEffect(() => {
    if (!activeGroupId) {
      setActiveGroup(null);
      return;
    }
    getGroupDetail(activeGroupId).then(setActiveGroup);
  }, [activeGroupId]);

  const selectNextGroup = useCallback(() => {
    if (!activeGroupId || groups.length === 0) return;
    const index = groups.findIndex((group) => group.id === activeGroupId);
    const next = groups[index + 1] ?? groups[0];
    setActiveGroupId(next?.id ?? null);
  }, [activeGroupId, groups]);

  return {
    groups,
    activeGroupId,
    setActiveGroupId,
    activeGroup,
    loading,
    refreshGroups,
    selectNextGroup,
  };
}
