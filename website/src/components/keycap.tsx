import type { ReactNode } from "react";
import { cn } from "cn";

/** A single keyboard key, styled like the keycaps in the Clipsy app. */
export function Keycap({
  children,
  pressed,
  className,
}: {
  children: ReactNode;
  pressed?: boolean;
  className?: string;
}) {
  return (
    <kbd
      data-pressed={pressed ? "true" : undefined}
      className={cn(
        "keycap inline-flex h-6 min-w-6 items-center justify-center rounded-md border border-border bg-card px-1.5 font-mono text-xs font-medium text-foreground",
        className,
      )}
    >
      {children}
    </kbd>
  );
}
