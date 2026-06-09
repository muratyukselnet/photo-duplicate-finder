import { useCallback, useEffect, useRef, useState } from "react";
import type { FileMember } from "../lib/types";
import { MetadataBar } from "./MetadataBar";

interface ZoomableImageProps {
  src: string;
  alt: string;
  member: FileMember;
}

export function ZoomableImage({ src, alt, member }: ZoomableImageProps) {
  const [scale, setScale] = useState(1);
  const containerRef = useRef<HTMLDivElement>(null);

  const onWheel = useCallback((event: WheelEvent) => {
    if (!event.metaKey) return;
    event.preventDefault();
    setScale((current) => {
      const delta = event.deltaY > 0 ? -0.15 : 0.15;
      return Math.min(5, Math.max(0.25, current + delta));
    });
  }, []);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    container.addEventListener("wheel", onWheel, { passive: false });
    return () => container.removeEventListener("wheel", onWheel);
  }, [onWheel]);

  return (
    <div
      ref={containerRef}
      className="relative h-full w-full overflow-auto"
      onClick={(event) => event.stopPropagation()}
    >
      <div className="pointer-events-none absolute right-4 top-4 z-10 max-w-sm rounded-lg bg-black/70 p-3 text-white shadow-lg backdrop-blur-sm">
        <MetadataBar member={member} overlay />
      </div>
      <div className="flex min-h-full min-w-full items-center justify-center p-4">
        <img
          src={src}
          alt={alt}
          draggable={false}
          style={{ transform: `scale(${scale})`, transformOrigin: "center center" }}
          className="max-h-[calc(100vh-6rem)] max-w-[calc(100vw-4rem)] object-contain"
        />
      </div>
    </div>
  );
}

export function useBodyScrollLock(locked: boolean) {
  useEffect(() => {
    if (!locked) return;
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previous;
    };
  }, [locked]);
}
