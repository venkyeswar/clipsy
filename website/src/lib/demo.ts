export type DemoKind = "text" | "image" | "files";
export type DemoItem = { id: number; kind: DemoKind; label: string; pinned: boolean };
export type DemoTab = "all" | "text" | "images" | "files" | "pinned";

const tabKind: Record<Exclude<DemoTab, "all" | "pinned">, DemoKind> = {
  text: "text",
  images: "image",
  files: "files",
};

/** Whether the given tab's clear action would remove this item. Mirrors the app's clear rules. */
function isClearedBy(item: DemoItem, tab: DemoTab, includePinned: boolean): boolean {
  if (tab === "pinned") return item.pinned;
  if (item.pinned && !includePinned) return false;
  if (tab === "all") return true;
  return item.kind === tabKind[tab];
}

/** Items that remain after clearing the given tab, in their original order. */
export function clearTab(items: DemoItem[], tab: DemoTab, includePinned: boolean): DemoItem[] {
  return items.filter((item) => !isClearedBy(item, tab, includePinned));
}

/** How many items the clear action would remove. */
export function countClearable(items: DemoItem[], tab: DemoTab, includePinned: boolean): number {
  return items.length - clearTab(items, tab, includePinned).length;
}

/** Items shown for a tab and search query: pinned first, then insertion order (newest first). */
export function visibleItems(items: DemoItem[], tab: DemoTab, query: string): DemoItem[] {
  const needle = query.trim().toLowerCase();
  const matches = items.filter((item) => {
    if (tab === "pinned" && !item.pinned) return false;
    if (tab !== "all" && tab !== "pinned" && item.kind !== tabKind[tab]) return false;
    return needle === "" || item.label.toLowerCase().includes(needle);
  });
  return [...matches.filter((item) => item.pinned), ...matches.filter((item) => !item.pinned)];
}
