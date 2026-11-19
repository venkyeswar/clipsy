import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { cn } from "cn";

/**
 * A page section whose body scrolls inside its own box on large screens (lg and up), so two
 * side-by-side panels stay the same height. The heading stays fixed above the scroll area.
 * Below lg the body is not height-bounded and the page scrolls instead.
 */
export function ScrollPanel({
  id,
  headingId,
  heading,
  label,
  children,
  className,
}: {
  id: string;
  headingId: string;
  heading: ReactNode;
  /** Names the scroll region for assistive technology. */
  label: string;
  children: ReactNode;
  className?: string;
}) {
  const regionRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const [moreBelow, setMoreBelow] = useState(false);

  const update = useCallback(() => {
    const region = regionRef.current;
    if (!region) return;
    setMoreBelow(region.scrollTop + region.clientHeight < region.scrollHeight - 2);
  }, []);

  // Re-check when the panel or its content changes size (tabs, releases loading, resizing).
  useEffect(() => {
    update();
    window.addEventListener("resize", update);
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(update);
    if (regionRef.current) observer?.observe(regionRef.current);
    if (contentRef.current) observer?.observe(contentRef.current);
    return () => {
      window.removeEventListener("resize", update);
      observer?.disconnect();
    };
  }, [update]);

  return (
    <section
      id={id}
      aria-labelledby={headingId}
      className={cn("flex min-w-0 flex-col lg:h-[clamp(28rem,70vh,46rem)]", className)}
    >
      {heading}

      <div className="relative mt-4 flex min-h-0 flex-1 flex-col">
        <div
          ref={regionRef}
          role="region"
          aria-label={label}
          tabIndex={0}
          onScroll={update}
          className="scroll-panel min-h-0 flex-1 overflow-y-auto rounded-md pr-3 pb-8 focus-visible:outline-2 focus-visible:outline-offset-4 lg:pr-4"
        >
          <div ref={contentRef}>{children}</div>
        </div>

        {/* A soft fade at the bottom while more content is below (large screens only). */}
        <div
          aria-hidden="true"
          className={cn(
            "pointer-events-none absolute inset-x-0 bottom-0 hidden h-16 bg-linear-to-t from-[var(--panel-surface)] to-transparent transition-opacity duration-200 motion-reduce:transition-none lg:block",
            moreBelow ? "opacity-100" : "opacity-0",
          )}
        />
      </div>
    </section>
  );
}
