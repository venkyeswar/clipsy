import type { ReactNode } from "react";

/** A single keyboard key, styled like the keycaps in the Clipsy app. */
export function Keycap({ children }: { children: ReactNode }) {
  return (
    <kbd className="inline-flex h-6 min-w-6 items-center justify-center rounded-md border border-border bg-muted px-1.5 font-mono text-xs font-medium text-foreground">
      {children}
    </kbd>
  );
}
