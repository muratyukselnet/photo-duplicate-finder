import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  DuplicateGroupDetail,
  DuplicateGroupSummary,
  ReviewStatus,
  ScanConfig,
  ScanPreset,
  ScanProgress,
  SessionSummary,
  TrashResult,
} from "./types";

export async function listSessions(): Promise<SessionSummary[]> {
  return invoke("list_sessions");
}

export async function createSession(
  rootPath: string,
  preset: ScanPreset = "visual_similar",
  name?: string,
  config?: ScanConfig,
): Promise<string> {
  return invoke("create_session", {
    request: { rootPath, name, preset, config },
  });
}

export async function deleteSession(sessionId: string): Promise<void> {
  return invoke("delete_session", { sessionId });
}

export async function pickDirectory(): Promise<string | null> {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Choose a folder of photos",
  });

  if (!selected || Array.isArray(selected)) {
    return null;
  }

  return selected;
}

export async function startScan(sessionId: string): Promise<void> {
  return invoke("start_scan", { sessionId });
}

export async function pauseScan(sessionId: string): Promise<void> {
  return invoke("pause_scan", { sessionId });
}

export async function resumeScan(sessionId: string): Promise<void> {
  return invoke("resume_scan", { sessionId });
}

export async function stopScan(sessionId: string): Promise<void> {
  return invoke("stop_scan", { sessionId });
}

export async function getScanProgress(sessionId: string): Promise<ScanProgress> {
  return invoke("get_scan_progress", { sessionId });
}

export async function listDuplicateGroups(
  sessionId: string,
  reviewStatus?: ReviewStatus,
  limit = 100,
  offset = 0,
): Promise<DuplicateGroupSummary[]> {
  return invoke("list_duplicate_groups", {
    sessionId,
    reviewStatus,
    limit,
    offset,
  });
}

export async function getGroupDetail(groupId: number): Promise<DuplicateGroupDetail> {
  return invoke("get_group_detail", { groupId });
}

export async function setKeepers(groupId: number, keeperFileIds: number[]): Promise<void> {
  return invoke("set_keepers", { groupId, keeperFileIds });
}

export async function keepAllInGroup(groupId: number): Promise<void> {
  return invoke("keep_all_in_group", { groupId });
}

export async function moveDuplicatesToTrash(groupId: number): Promise<TrashResult> {
  return invoke("move_duplicates_to_trash", { groupId });
}

export async function keepSelectedAndTrash(
  groupId: number,
  keeperFileIds: number[],
): Promise<TrashResult> {
  return invoke("keep_selected_and_trash", { groupId, keeperFileIds });
}

const thumbCache = new Map<string, string>();

export async function getThumbnailUrl(
  sourcePath: string,
  cacheKey: string,
): Promise<string> {
  const cacheId = `${cacheKey}`;
  if (thumbCache.has(cacheId)) {
    return thumbCache.get(cacheId)!;
  }

  const thumbPath = await invoke<string>("ensure_thumbnail", {
    sourcePath,
    cacheKey,
  });
  const url = convertFileSrc(thumbPath);
  thumbCache.set(cacheId, url);
  return url;
}

export function getFullImageUrl(sourcePath: string): string {
  return convertFileSrc(sourcePath);
}
