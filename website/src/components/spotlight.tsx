import type { PointerEvent, ReactNode } from "react";

/** A panel with a soft light that follows the cursor inside it (see .spotlight in index.css). */
export function Spotlight({ children, className = "" }: { children: ReactNode; className?: string }) {
  const onMove = (event: PointerEvent<HTMLDivElement>) => {
    const box = event.currentTarget.getBoundingClientRect();
    event.currentTarget.style.setProperty("--mx", `${event.clientX - box.left}px`);
    event.currentTarget.style.setProperty("--my", `${event.clientY - box.top}px`);
  };
  return (
    <div onPointerMove={onMove} className={`spotlight relative isolate ${className}`}>
      {children}
    </div>
  );
}
