import { describe, expect, it } from "vitest";
import { initialKeeperSelection } from "./keeperSelection";
import type { DuplicateGroupDetail, FileMember } from "./types";

function member(fileId: number, overrides: Partial<FileMember> = {}): FileMember {
  return { fileId, path: `/${fileId}.jpg`, fileName: `${fileId}.jpg`, size: 10, thumbnailKey: String(fileId), ...overrides };
}
function group(overrides: Partial<DuplicateGroupDetail> = {}): DuplicateGroupDetail {
  return { id: 1, kind: "burst", confidence: 0.9, reviewStatus: "pending", bytesRecoverable: 10, members: [member(1, { ai: { decision: "keep", score: 0.9, flags: [] } }), member(2, { ai: { decision: "discard", score: 0.2, flags: ["subject_soft"] } })], ...overrides };
}

describe("initialKeeperSelection", () => {
  it("preselects only suggested keepers in ready groups", () => {
    expect(initialKeeperSelection(group({ aiBucket: "ready_to_confirm" }))).toEqual([1]);
  });
  it("keeps all intentional variants", () => {
    expect(initialKeeperSelection(group({ aiBucket: "variants_kept" }))).toEqual([1, 2]);
  });
  it.each(["needs_you", null, undefined] as const)("does not preselect ambiguous or unanalyzed groups: %s", (aiBucket) => {
    expect(initialKeeperSelection(group({ aiBucket }))).toEqual([]);
  });
  it("preserves manual decisions ahead of AI suggestions", () => {
    expect(initialKeeperSelection(group({ aiBucket: "ready_to_confirm", members: [member(1, { isKeeper: false, ai: { decision: "keep", score: 1, flags: [] } }), member(2, { isKeeper: true })] }))).toEqual([2]);
  });
  it("preserves an explicitly empty manual selection", () => {
    expect(initialKeeperSelection(group({ aiBucket: "variants_kept", members: [member(1, { isKeeper: false })] }))).toEqual([]);
  });
  it("never suggests removing a protected photo in a ready group", () => {
    expect(initialKeeperSelection(group({ aiBucket: "ready_to_confirm", members: [member(1, { ai: { decision: "discard", score: 0, flags: [] }, protected: ["xmp_rating"] })] }))).toEqual([1]);
  });
});
