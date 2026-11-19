import { cn } from "cn";
import { Keycap } from "@/components/keycap";
import { Separator } from "@/components/ui/separator";
import { shortcutFor } from "@/config";
import { useSelectedPlatform } from "@/lib/platform-context";

type Shortcut = { keys: string[]; label: string };

export function ShortcutsSection() {
  const { platform } = useSelectedPlatform();
  const shortcuts: Shortcut[] = [
    { keys: shortcutFor(platform), label: "Open Clipsy" },
    { keys: ["↵"], label: "Paste the selected item" },
    { keys: ["⇧", "↵"], label: "Paste as plain text" },
    { keys: ["⌫"], label: "Remove the selected item" },
  ];

  return (
    <div>
      <h2 className="font-heading text-2xl font-semibold tracking-tight sm:text-3xl">Keyboard shortcuts</h2>
      <p className="mt-1 max-w-[60ch] text-muted-foreground">
        The shortcut opens Clipsy from anywhere. The other keys work inside the Clipsy window.
      </p>

      <dl className="mt-3 overflow-hidden rounded-xl border border-border/70 bg-background/80 backdrop-blur-sm">
        {shortcuts.map((shortcut, index) => (
          <div key={shortcut.label}>
            {index > 0 && <Separator />}
            <div
              className={cn(
                "group/keys flex min-h-10 items-center justify-between gap-3 px-4 py-1.5 transition-colors duration-150 hover:bg-muted/60",
                index === 0 && "bg-brand-teal-soft/50 hover:bg-brand-teal-soft/70",
              )}
            >
              <dt className={cn("text-sm", index === 0 ? "font-medium text-foreground" : "text-muted-foreground")}>
                {shortcut.label}
              </dt>
              <dd className="flex flex-wrap items-center justify-end gap-1">
                {shortcut.keys.map((key, keyIndex) => (
                  <span key={`${key}-${keyIndex}`} className="inline-flex items-center gap-1">
                    {keyIndex > 0 && (
                      <span aria-hidden="true" className="text-xs text-muted-foreground">
                        +
                      </span>
                    )}
                    <Keycap className="h-7 min-w-7">{key}</Keycap>
                  </span>
                ))}
              </dd>
            </div>
          </div>
        ))}
      </dl>
    </div>
  );
}
