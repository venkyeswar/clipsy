import { useEffect, useRef, useState, type ReactNode } from "react";
import { cn } from "cn";

/**
 * A page section with a heading and a body. The heading always stays visible. When `bounded`,
 * the body is height-limited on large screens (lg and up) and scrolls inside itself only when
 * its content is taller than that limit. Below lg the body flows with the page.
 *
 * The region is labelled, and it is keyboard focusable only while it can scroll. Overscroll is
 * not contained, so a wheel that reaches the end of the body keeps scrolling the page.
 */
export function ScrollPanel({
  id,
  headingId,
  heading,
  label,
  children,
  className,
  bounded = false,
}: {
  id: string;
  headingId: string;
  heading: ReactNode;
  /** Names the content region for assistive technology. */
  label: string;
  children: ReactNode;
  className?: string;
  /** Limit the body's height on lg and up, so it scrolls inside its own box. */
  bounded?: boolean;
}) {
  const bodyRef = useRef<HTMLDivElement>(null);
  const [scrollable, setScrollable] = useState(false);

  useEffect(() => {
    const body = bodyRef.current;
    if (!body) return;
    const measure = () => setScrollable(body.scrollHeight > body.clientHeight + 1);
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(body);
    if (body.firstElementChild) observer.observe(body.firstElementChild);
    return () => observer.disconnect();
  }, []);

  return (
    <section id={id} aria-labelledby={headingId} className={cn("flex min-w-0 flex-col", className)}>
      {heading}
      <div
        ref={bodyRef}
        role="region"
        aria-label={label}
        tabIndex={scrollable ? 0 : undefined}
        className={cn(
          "mt-3 min-w-0",
          bounded && "lg:max-h-[min(36rem,calc(100dvh-9rem))] lg:overflow-y-auto lg:pr-2",
        )}
      >
        {children}
      </div>
    </section>
  );
}
