import type { DuplicateGroupDetail } from "./types";

export const bucketLabels = {
  ready_to_confirm: "Ready to confirm",
  needs_you: "Needs you",
  variants_kept: "Variants, kept",
  not_analyzed: "Not analyzed",
};

/** Manual decisions take precedence. Ambiguous groups never get AI selections. */
export function initialKeeperSelection(group: DuplicateGroupDetail): number[] {
  if (group.members.some((member) => member.isKeeper != null)) {
    return group.members.filter((member) => member.isKeeper).map((member) => member.fileId);
  }
  if (group.aiBucket !== "ready_to_confirm" && group.aiBucket !== "variants_kept") return [];
  return group.members.filter((member) => group.aiBucket === "variants_kept" || member.ai?.decision === "keep" || (member.protected?.length ?? 0) > 0).map((member) => member.fileId);
}
