import { useEffect, useRef, useState } from "react";
import { Check, Copy } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { cn } from "cn";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";

const CONFIRM_MS = 1500;

/** Selects the nearest `[data-copy-source]` text in the same container, for manual copying. */
function selectSource(button: HTMLButtonElement | null) {
  const source = button?.parentElement?.querySelector<HTMLElement>("[data-copy-source]");
  if (!source) return;
  const selection = window.getSelection();
  const range = document.createRange();
  range.selectNodeContents(source);
  selection?.removeAllRanges();
  selection?.addRange(range);
}

export function CopyButton({ text, label = "Copy command" }: { text: string; label?: string }) {
  const [copied, setCopied] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  async function handleClick() {
    try {
      if (!navigator.clipboard) throw new Error("Clipboard API unavailable");
      await navigator.clipboard.writeText(text);
      toast("Copied");
      setCopied(true);
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => setCopied(false), CONFIRM_MS);
    } catch {
      toast("Press Ctrl+C to copy");
      selectSource(buttonRef.current);
    }
  }

  const Icon = copied ? Check : Copy;
  return (
    <Button
      ref={buttonRef}
      type="button"
      variant="ghost"
      size="icon"
      aria-label={label}
      className={cn(
        "size-7 shrink-0 text-muted-foreground hover:text-foreground pointer-coarse:size-11",
        copied && "text-secondary-accent hover:text-secondary-accent",
      )}
      onClick={handleClick}
    >
      <AnimatePresence mode="popLayout" initial={false}>
        <motion.span
          key={copied ? "check" : "copy"}
          initial={{ scale: 0.4, opacity: 0, rotate: copied ? -45 : 0 }}
          animate={{ scale: 1, opacity: 1, rotate: 0 }}
          exit={{ scale: 0.4, opacity: 0 }}
          transition={{ type: "spring", stiffness: 500, damping: 28 }}
          className="inline-flex"
        >
          <Icon aria-hidden="true" className="size-3.5" />
        </motion.span>
      </AnimatePresence>
    </Button>
  );
}
