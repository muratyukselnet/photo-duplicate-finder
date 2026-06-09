import { describe, expect, it } from "vitest";
import type { FileMember } from "./types";

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

describe("suggestKeeper", () => {
  it("prefers highest resolution", () => {
    const members: FileMember[] = [
      {
        fileId: 1,
        path: "/a.jpg",
        fileName: "a.jpg",
        size: 100,
        width: 1000,
        height: 1000,
        thumbnailKey: "a",
      },
      {
        fileId: 2,
        path: "/b.jpg",
        fileName: "b.jpg",
        size: 100,
        width: 4000,
        height: 3000,
        thumbnailKey: "b",
      },
    ];

    expect(suggestKeeper(members)).toBe(2);
  });
});
