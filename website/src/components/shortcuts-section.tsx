import { Keycap } from "@/components/keycap";
import { Separator } from "@/components/ui/separator";
import { config } from "@/config";

type Shortcut = { keys: string[]; label: string };

export function ShortcutsSection() {
  const shortcuts: Shortcut[] = [
    { keys: config.shortcut, label: "Open Clipsy" },
    { keys: ["↵"], label: "Paste the selected item" },
    { keys: ["⇧", "↵"], label: "Paste as plain text" },
    { keys: ["⌫"], label: "Remove the selected item" },
  ];

  return (
    <div>
      <h2 className="font-heading text-2xl font-semibold tracking-tight">Keyboard shortcuts</h2>
      <p className="mt-2 max-w-[60ch] text-muted-foreground">
        The shortcut opens Clipsy from anywhere. The other keys work inside the Clipsy window.
      </p>

      <dl className="mt-6 rounded-xl border border-border bg-card px-4">
        {shortcuts.map((shortcut, index) => (
          <div key={shortcut.label}>
            {index > 0 && <Separator />}
            <div className="flex flex-col gap-2 py-3 sm:flex-row sm:items-center sm:justify-between">
              <dt className="order-2 text-muted-foreground sm:order-none">{shortcut.label}</dt>
              <dd className="flex flex-wrap items-center gap-1">
                {shortcut.keys.map((key, keyIndex) => (
                  <span key={`${key}-${keyIndex}`} className="inline-flex items-center gap-1">
                    {keyIndex > 0 && (
                      <span aria-hidden="true" className="text-muted-foreground">
                        +
                      </span>
                    )}
                    <Keycap>{key}</Keycap>
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
