export type SessionStatus = "scanning" | "reviewing" | "paused" | "completed";
export type ScanPhase = "walking" | "hashing" | "clustering" | "complete";
export type DuplicateKind = "exact" | "visual" | "burst" | "metadata";
export type ReviewStatus = "pending" | "resolved";
export type ScanPreset = "exact_only" | "visual_similar" | "burst_time_window" | "custom";

export interface SessionSummary {
  id: string;
  name: string;
  rootPath: string;
  status: SessionStatus;
  filesScanned: number;
  groupsPending: number;
  groupsTotal: number;
  createdAt: string;
  updatedAt: string;
}

export interface ScanProgress {
  sessionId: string;
  phase: ScanPhase;
  filesProcessed: number;
  filesTotalEstimate: number;
  filesPerSec: number;
  currentPath?: string | null;
  groupsFound: number;
}

export interface ExifData {
  cameraMake?: string | null;
  cameraModel?: string | null;
  iso?: number | null;
  aperture?: string | null;
  shutterSpeed?: string | null;
  focalLength?: string | null;
  dateTaken?: string | null;
}

export interface FileMember {
  fileId: number;
  path: string;
  fileName: string;
  size: number;
  width?: number | null;
  height?: number | null;
  createdAt?: string | null;
  modifiedAt?: string | null;
  exif?: ExifData | null;
  isKeeper?: boolean | null;
  thumbnailKey: string;
  companionRawPath?: string | null;
  companionRawSize?: number | null;
}

export interface DuplicateGroupSummary {
  id: number;
  kind: DuplicateKind;
  confidence: number;
  memberCount: number;
  reviewStatus: ReviewStatus;
  bytesRecoverable: number;
}

export interface DuplicateGroupDetail {
  id: number;
  kind: DuplicateKind;
  confidence: number;
  reviewStatus: ReviewStatus;
  members: FileMember[];
  bytesRecoverable: number;
}

export interface TrashResult {
  trashedCount: number;
  bytesFreed: number;
  errors: string[];
}

export interface ScanConfig {
  exactHash?: boolean;
  visualSimilar?: boolean;
  burstDetection?: boolean;
  filenameRanking?: boolean;
  phashThreshold?: number;
  burstWindowSecs?: number;
  includeRaw?: boolean;
}
