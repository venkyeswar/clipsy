import { cn } from "cn";
import { CopyButton } from "@/components/copy-button";

/**
 * One copyable shell command. The `$` prompt is decoration and is not copied.
 * `wrap` breaks long commands over lines instead of scrolling, so all of it is readable.
 */
export function CommandLine({
  text,
  label,
  className,
  wrap,
}: {
  text: string;
  label?: string;
  className?: string;
  wrap?: boolean;
}) {
  return (
    <div
      className={cn(
        "group/cmd flex min-h-9 min-w-0 items-center gap-2 rounded-lg border border-border bg-muted/70 py-1 pr-1 pl-3 transition-colors duration-150 hover:border-primary/40",
        className,
      )}
    >
      <span aria-hidden="true" className="font-mono text-[0.8125rem] text-secondary-accent select-none">
        $
      </span>
      <pre
        data-copy-source
        tabIndex={0}
        className={cn(
          "min-w-0 flex-1 overflow-x-auto rounded-sm py-1 font-mono text-[0.8125rem] text-foreground outline-none [scrollbar-width:thin] focus-visible:ring-2 focus-visible:ring-ring",
          wrap ? "whitespace-pre-wrap break-all" : "whitespace-pre",
        )}
      >
        <code>{text}</code>
      </pre>
      <CopyButton text={text} label={label} />
    </div>
  );
}
