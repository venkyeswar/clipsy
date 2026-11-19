import { useEffect, useRef, useState } from "react";
import { Check, Copy } from "lucide-react";
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
      className="size-11 shrink-0"
      onClick={handleClick}
    >
      <Icon aria-hidden="true" />
    </Button>
  );
}
