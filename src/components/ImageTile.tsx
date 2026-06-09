import { Check, ZoomIn } from "lucide-react";
import { useEffect, useState } from "react";
import { getThumbnailUrl } from "../lib/api";
import type { FileMember } from "../lib/types";
import { cn, formatBytes } from "../lib/utils";
import { MetadataBar } from "./MetadataBar";

interface ImageTileProps {
  member: FileMember;
  selected: boolean;
  focused?: boolean;
  suggestedKeeper?: boolean;
  onToggleSelect: () => void;
  onOpenZoom: () => void;
}

export function ImageTile({
  member,
  selected,
  focused = false,
  suggestedKeeper,
  onToggleSelect,
  onOpenZoom,
}: ImageTileProps) {
  const [thumbUrl, setThumbUrl] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    getThumbnailUrl(member.path, member.thumbnailKey).then((url) => {
      if (active) setThumbUrl(url);
    });
    return () => {
      active = false;
    };
  }, [member.path, member.thumbnailKey]);

  return (
    <div
      className={cn(
        "group relative flex flex-col overflow-hidden rounded-xl border bg-card transition-all",
        selected ? "border-primary ring-2 ring-primary/40" : "border-border",
        focused && "ring-2 ring-white/60",
        suggestedKeeper && "shadow-[0_0_0_1px_rgba(34,197,94,0.5)]",
      )}
    >
      <button
        type="button"
        className="relative aspect-[4/3] w-full overflow-hidden bg-muted"
        onClick={onOpenZoom}
      >
        {thumbUrl ? (
          <img
            src={thumbUrl}
            alt={member.fileName}
            className="h-full w-full object-cover transition-transform group-hover:scale-[1.02]"
            loading="lazy"
          />
        ) : (
          <div className="flex h-full items-center justify-center text-muted-foreground">
            Loading…
          </div>
        )}
        <div className="absolute inset-0 bg-black/0 transition group-hover:bg-black/20" />
        <div className="absolute right-2 top-2 flex gap-2 opacity-0 transition group-hover:opacity-100">
          <span className="rounded-full bg-black/60 p-2 text-white">
            <ZoomIn className="h-4 w-4" />
          </span>
        </div>
        {member.companionRawPath && (
          <span className="absolute left-2 bottom-2 rounded-full bg-black/70 px-2 py-1 text-[10px] font-semibold uppercase tracking-wide text-white">
            RAW
          </span>
        )}
        {suggestedKeeper && (
          <span className="absolute left-2 top-2 rounded-full bg-success px-2 py-1 text-[10px] font-semibold uppercase tracking-wide text-white">
            Suggested
          </span>
        )}
      </button>

      <button
        type="button"
        onClick={onToggleSelect}
        className="flex w-full cursor-pointer items-start gap-2 p-3 text-left transition hover:bg-muted/50"
        aria-label={selected ? "Deselect photo" : "Select photo"}
      >
        <span
          className={cn(
            "mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded border",
            selected
              ? "border-primary bg-primary text-white"
              : "border-border bg-background",
          )}
        >
          {selected && <Check className="h-3.5 w-3.5" />}
        </span>
        <div className="min-w-0 flex-1">
          <MetadataBar member={member} compact />
          <p className="mt-1 text-[11px] text-muted-foreground">
            {formatBytes(member.size)}
          </p>
        </div>
      </button>
    </div>
  );
}
