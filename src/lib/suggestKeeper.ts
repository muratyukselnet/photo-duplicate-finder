import type { FileMember } from "./types";

export function suggestKeeper(members: FileMember[]): number | null {
  if (members.length === 0) return null;
  const captureTime = (member: FileMember) => {
    const value = member.exif?.dateTaken ? Date.parse(member.exif.dateTaken) : NaN;
    return Number.isFinite(value) ? value : Infinity;
  };
  const sorted = [...members].sort((a, b) => {
    const areaA = (a.width ?? 0) * (a.height ?? 0);
    const areaB = (b.width ?? 0) * (b.height ?? 0);
    if (areaA !== areaB) return areaB - areaA;
    const takenA = captureTime(a);
    const takenB = captureTime(b);
    return takenA - takenB;
  });
  return sorted[0]?.fileId ?? null;
}

