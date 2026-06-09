import type { ExifData, FileMember } from "../lib/types";
import { formatBytes, formatDate } from "../lib/utils";

interface MetadataBarProps {
  member: FileMember;
  compact?: boolean;
  overlay?: boolean;
}

function exifLine(exif?: ExifData | null): string {
  if (!exif) return "";
  const parts = [
    exif.cameraMake || exif.cameraModel
      ? [exif.cameraMake, exif.cameraModel].filter(Boolean).join(" ")
      : null,
    exif.iso ? `ISO ${exif.iso}` : null,
    exif.aperture,
    exif.shutterSpeed,
    exif.focalLength,
  ].filter(Boolean);
  return parts.join(" · ");
}

export function MetadataBar({
  member,
  compact = false,
  overlay = false,
}: MetadataBarProps) {
  const dimensions =
    member.width && member.height ? `${member.width}×${member.height}` : "—";

  if (overlay) {
    return (
      <div className="space-y-1 text-xs">
        <p className="truncate font-medium">{member.fileName}</p>
        <p className="text-white/80">
          {dimensions} · {formatBytes(member.size)}
        </p>
        <p className="truncate text-white/70">{exifLine(member.exif) || "No EXIF"}</p>
      </div>
    );
  }

  if (compact) {
    return (
      <div className="space-y-1 text-xs text-muted-foreground">
        <p className="truncate font-medium text-foreground">{member.fileName}</p>
        <p>
          {dimensions} · {formatBytes(member.size)}
        </p>
        <p className="truncate">{exifLine(member.exif) || "No EXIF"}</p>
        {member.companionRawPath && (
          <p className="truncate text-primary">+ paired RAW file</p>
        )}
      </div>
    );
  }

  return (
    <div className="grid gap-2 rounded-lg border border-border bg-card p-4 text-sm">
      <div>
        <p className="font-medium">{member.fileName}</p>
        <p className="truncate text-muted-foreground">{member.path}</p>
      </div>
      <div className="grid grid-cols-2 gap-2 text-muted-foreground">
        <span>Dimensions</span>
        <span className="text-foreground">{dimensions}</span>
        <span>Size</span>
        <span className="text-foreground">{formatBytes(member.size)}</span>
        <span>Created</span>
        <span className="text-foreground">{formatDate(member.createdAt)}</span>
        <span>Modified</span>
        <span className="text-foreground">{formatDate(member.modifiedAt)}</span>
        <span>Taken</span>
        <span className="text-foreground">{formatDate(member.exif?.dateTaken)}</span>
      </div>
      {member.exif && (
        <p className="rounded-md bg-muted px-3 py-2 text-xs text-foreground">
          {exifLine(member.exif) || "No camera metadata"}
        </p>
      )}
    </div>
  );
}
