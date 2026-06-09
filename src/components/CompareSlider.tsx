import useEmblaCarousel from "embla-carousel-react";
import { ChevronLeft, ChevronRight, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { getFullImageUrl } from "../lib/api";
import type { FileMember } from "../lib/types";
import { Button } from "./ui/button";
import { useBodyScrollLock, ZoomableImage } from "./ZoomableImage";

interface CompareSliderProps {
  members: FileMember[];
  initialIndex?: number;
  onClose: () => void;
}

export function CompareSlider({
  members,
  initialIndex = 0,
  onClose,
}: CompareSliderProps) {
  useBodyScrollLock(true);
  const [emblaRef, emblaApi] = useEmblaCarousel({ loop: true, startIndex: initialIndex });
  const [index, setIndex] = useState(initialIndex);

  const scrollPrev = useCallback(() => emblaApi?.scrollPrev(), [emblaApi]);
  const scrollNext = useCallback(() => emblaApi?.scrollNext(), [emblaApi]);

  useEffect(() => {
    if (!emblaApi) return;
    const onSelect = () => setIndex(emblaApi.selectedScrollSnap());
    emblaApi.on("select", onSelect);
    onSelect();
    return () => {
      emblaApi.off("select", onSelect);
    };
  }, [emblaApi]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "ArrowLeft") scrollPrev();
      if (event.key === "ArrowRight") scrollNext();
      if (event.key === "Escape") {
        event.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose, scrollNext, scrollPrev]);

  const current = members[index];
  if (!current) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex flex-col bg-black/95"
      onWheel={(event) => event.stopPropagation()}
    >
      <div className="flex shrink-0 items-center justify-between border-b border-border/40 px-6 py-4">
        <div>
          <p className="text-sm text-muted-foreground">Compare selected photos</p>
          <p className="font-medium">
            {index + 1} of {members.length}
          </p>
        </div>
        <Button variant="ghost" size="icon" onClick={onClose}>
          <X className="h-5 w-5" />
        </Button>
      </div>

      <div className="relative flex min-h-0 flex-1">
        <Button
          variant="secondary"
          size="icon"
          className="absolute left-6 top-1/2 z-10 -translate-y-1/2"
          onClick={scrollPrev}
        >
          <ChevronLeft className="h-5 w-5" />
        </Button>

        <div className="h-full w-full overflow-hidden" ref={emblaRef}>
          <div className="flex h-full">
            {members.map((member) => (
              <div
                key={member.fileId}
                className="flex min-h-0 min-w-0 flex-[0_0_100%]"
              >
                <ZoomableImage
                  src={getFullImageUrl(member.path)}
                  alt={member.fileName}
                  member={member}
                />
              </div>
            ))}
          </div>
        </div>

        <Button
          variant="secondary"
          size="icon"
          className="absolute right-6 top-1/2 z-10 -translate-y-1/2"
          onClick={scrollNext}
        >
          <ChevronRight className="h-5 w-5" />
        </Button>
      </div>
    </div>
  );
}
