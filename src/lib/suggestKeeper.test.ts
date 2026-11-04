import { describe, expect, it } from "vitest";
import type { FileMember } from "./types";

import { suggestKeeper } from "./suggestKeeper";

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

describe("suggestKeeper dates and stability", () => {
  const photo = (fileId: number, dateTaken?: string): FileMember => ({ fileId, path: "/photo.jpg", fileName: "photo.jpg", size: 100, thumbnailKey: String(fileId), width: 100, height: 100, exif: { dateTaken } });
  it("returns null for an empty group", () => expect(suggestKeeper([])).toBeNull());
  it("prefers the earliest valid capture date on equal resolution", () => {
    expect(suggestKeeper([photo(1, "invalid"), photo(2, "2025-01-02"), photo(3, "2025-01-01")])).toBe(3);
  });
  it("does not reorder the input when ranking", () => {
    const members = [photo(1, "2025-01-02"), photo(2, "2025-01-01")];
    suggestKeeper(members);
    expect(members.map((member) => member.fileId)).toEqual([1, 2]);
  });
});
