import { X } from "lucide-react";
import { useEffect } from "react";
import { getFullImageUrl } from "../lib/api";
import type { FileMember } from "../lib/types";
import { Button } from "./ui/button";
import { useBodyScrollLock, ZoomableImage } from "./ZoomableImage";

interface LightboxProps {
  member: FileMember;
  onClose: () => void;
}

export function Lightbox({ member, onClose }: LightboxProps) {
  useBodyScrollLock(true);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-50 flex flex-col bg-black/95"
      onClick={onClose}
      onWheel={(event) => event.stopPropagation()}
    >
      <div className="flex shrink-0 justify-end p-4">
        <Button variant="ghost" size="icon" onClick={onClose}>
          <X className="h-5 w-5" />
        </Button>
      </div>
      <div className="min-h-0 flex-1" onClick={(event) => event.stopPropagation()}>
        <ZoomableImage
          src={getFullImageUrl(member.path)}
          alt={member.fileName}
          member={member}
        />
      </div>
    </div>
  );
}
