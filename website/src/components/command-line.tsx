import { cn } from "cn";
import { CopyButton } from "@/components/copy-button";

/** One copyable shell command. The `$` prompt is decoration and is not copied. */
export function CommandLine({ text, label, className }: { text: string; label?: string; className?: string }) {
  return (
    <div
      className={cn(
        "group/cmd flex min-w-0 items-center gap-2 rounded-lg border border-border bg-muted/70 py-0.5 pr-0.5 pl-3 transition-colors hover:border-secondary-accent/50",
        className,
      )}
    >
      <span aria-hidden="true" className="font-mono text-[0.8125rem] text-secondary-accent select-none">
        $
      </span>
      <pre
        data-copy-source
        tabIndex={0}
        className="min-w-0 flex-1 overflow-x-auto rounded-sm py-2 font-mono text-[0.8125rem] whitespace-pre text-foreground outline-none [scrollbar-width:thin] focus-visible:ring-2 focus-visible:ring-ring"
      >
        <code>{text}</code>
      </pre>
      <CopyButton text={text} label={label} />
    </div>
  );
}
