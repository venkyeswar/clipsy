import { useCallback, useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { AnimatePresence, MotionConfig, motion } from "motion/react";
import { FileText, Image as ImageIcon, Search, Star, Trash2, Type, type LucideIcon } from "lucide-react";
import { cn } from "cn";
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { clearTab, countClearable, visibleItems, type DemoItem, type DemoKind, type DemoTab } from "@/lib/demo";

const SEED: DemoItem[] = [
  { id: 1, kind: "text", label: "git clone git@github.com:venkyeswar/clipsy.git", pinned: false },
  { id: 2, kind: "text", label: "meeting notes: ship the website Friday", pinned: false },
  { id: 4, kind: "image", label: "screenshot-2026-11-19.png", pinned: false },
  { id: 5, kind: "files", label: "invoice.pdf", pinned: false },
  { id: 3, kind: "text", label: "sudo apt install ./clipsy_0.3.1_amd64.deb", pinned: true },
  { id: 6, kind: "image", label: "logo.svg", pinned: true },
];

/** Relative times shown on the right of each row. Static, since this is a demo. */
const AGE: Record<number, string> = { 1: "now", 2: "2m", 4: "9m", 5: "1h", 3: "2d", 6: "4d" };

/** The three newest items, which arrive one by one during autoplay (oldest first). */
const ARRIVALS: number[] = [4, 2, 1];
/** The item that gets starred (pinned) during autoplay. */
const STAR_ID = 2;

const OPENING_ITEMS = SEED.filter((item) => !ARRIVALS.includes(item.id));
const FINAL_ITEMS = SEED.map((item) => (item.id === STAR_ID ? { ...item, pinned: true } : item));

const KIND_ICON: Record<DemoKind, LucideIcon> = { text: Type, image: ImageIcon, files: FileText };

const TAB_META: Record<DemoTab, { label: string; clear: string; noun: string }> = {
  all: { label: "All", clear: "everything", noun: "items" },
  text: { label: "Text", clear: "text items", noun: "text items" },
  images: { label: "Images", clear: "images", noun: "images" },
  files: { label: "Files", clear: "files", noun: "files" },
  pinned: { label: "Pinned", clear: "pinned items", noun: "pinned items" },
};
const TABS: DemoTab[] = ["all", "text", "images", "files", "pinned"];

function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/**
 * A working mini version of the Clipsy popup. Autoplays once on load unless the
 * user has reduced motion; any user input cancels the autoplay immediately.
 */
export function PopupDemo({ autoplay = true }: { autoplay?: boolean }) {
  const checkboxId = useId();
  const [startPlaying] = useState(() => autoplay && !prefersReducedMotion());
  const [items, setItems] = useState<DemoItem[]>(() => (startPlaying ? OPENING_ITEMS : FINAL_ITEMS));
  const [tab, setTab] = useState<DemoTab>("all");
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [notice, setNotice] = useState("");
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [includePinned, setIncludePinned] = useState(false);
  const [lastArrived, setLastArrived] = useState<number | null>(null);
  const playingRef = useRef(startPlaying);
  const timersRef = useRef<number[]>([]);
  const rowRefs = useRef(new Map<number, HTMLLIElement>());
  const searchRef = useRef<HTMLInputElement>(null);
  const trashRef = useRef<HTMLButtonElement>(null);

  const stopAutoplay = useCallback(() => {
    if (!playingRef.current) return;
    playingRef.current = false;
    timersRef.current.forEach((timer) => window.clearTimeout(timer));
    timersRef.current = [];
    setItems(FINAL_ITEMS);
    setQuery("");
    setLastArrived(null);
  }, []);

  useEffect(() => {
    if (!playingRef.current) return;
    const arrive = (id: number) => {
      const seed = SEED.find((item) => item.id === id);
      if (!seed) return;
      setItems((prev) => (prev.some((item) => item.id === id) ? prev : [seed, ...prev]));
      setLastArrived(id);
    };
    const steps: [number, () => void][] = [
      [500, () => arrive(4)],
      [1100, () => arrive(2)],
      [1700, () => arrive(1)],
      [2500, () => setQuery("g")],
      [2620, () => setQuery("gi")],
      [2740, () => setQuery("git")],
      [3900, () => setQuery("")],
      [
        4400,
        () => {
          setItems((prev) => prev.map((item) => (item.id === STAR_ID ? { ...item, pinned: true } : item)));
          setLastArrived(null);
        },
      ],
      [
        4900,
        () => {
          playingRef.current = false;
          setLastArrived(null);
        },
      ],
    ];
    timersRef.current = steps.map(([at, run]) => window.setTimeout(run, at));
    const timers = timersRef;
    return () => timers.current.forEach((timer) => window.clearTimeout(timer));
  }, []);

  const visible = visibleItems(items, tab, query);
  const selected = visible.find((item) => item.id === selectedId) ?? visible[0];
  const meta = TAB_META[tab];
  const clearable = countClearable(items, tab, includePinned);
  const canClear = countClearable(items, tab, true) > 0;

  function select(id: number) {
    setSelectedId(id);
    setNotice("");
  }

  function focusRow(id: number) {
    rowRefs.current.get(id)?.focus();
  }

  function handleRowKey(event: KeyboardEvent<HTMLLIElement>, index: number, id: number) {
    const moveTo = (target: DemoItem | undefined) => {
      if (!target) return;
      event.preventDefault();
      select(target.id);
      focusRow(target.id);
    };
    switch (event.key) {
      case "ArrowDown":
        moveTo(visible[Math.min(index + 1, visible.length - 1)]);
        break;
      case "ArrowUp":
        moveTo(visible[Math.max(index - 1, 0)]);
        break;
      case "Home":
        moveTo(visible[0]);
        break;
      case "End":
        moveTo(visible[visible.length - 1]);
        break;
      case "Enter":
        event.preventDefault();
        setNotice("Demo only. Nothing is pasted.");
        break;
      case "Backspace":
      case "Delete": {
        event.preventDefault();
        const next = visible[index + 1] ?? visible[index - 1];
        setItems((prev) => prev.filter((item) => item.id !== id));
        if (next) {
          select(next.id);
          focusRow(next.id);
        } else {
          searchRef.current?.focus();
        }
        break;
      }
    }
  }

  function openConfirm() {
    setIncludePinned(false);
    setConfirmOpen(true);
  }

  function confirmClear() {
    setItems((prev) => clearTab(prev, tab, includePinned));
    setConfirmOpen(false);
  }

  return (
    <MotionConfig reducedMotion="user">
      <div className="w-full min-w-0 max-w-md">
        <div
          role="region"
          aria-label="Interactive Clipsy demo"
          onPointerDownCapture={stopAutoplay}
          onKeyDownCapture={stopAutoplay}
          className="w-full min-w-0 max-w-full rounded-2xl border border-border bg-card p-3 text-card-foreground shadow-lg sm:p-4"
        >
          <div aria-hidden="true" className="mx-auto mb-3 h-1 w-10 rounded-full bg-muted-foreground/40" />

          <div className="relative">
            <Search aria-hidden="true" className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              ref={searchRef}
              type="search"
              aria-label="Search clipboard history"
              placeholder="Search clipboard history"
              value={query}
              onChange={(event) => {
                stopAutoplay();
                setQuery(event.target.value);
              }}
              className="h-11 pl-9"
            />
          </div>

          <Tabs
            value={tab}
            onValueChange={(value) => {
              stopAutoplay();
              setTab(value as DemoTab);
              setNotice("");
            }}
            className="mt-3 min-w-0"
          >
            <div className="flex items-center gap-2">
              <TabsList aria-label="Filter clipboard items" className="h-auto! w-auto! min-w-0 flex-1 p-1">
                {TABS.map((value) => (
                  <TabsTrigger
                    key={value}
                    value={value}
                    aria-label={value === "pinned" ? "Pinned" : undefined}
                    className="min-h-11 flex-1 px-1 text-xs sm:text-sm data-active:bg-primary data-active:text-primary-foreground dark:data-active:bg-primary dark:data-active:text-primary-foreground"
                  >
                    {value === "pinned" ? <Star aria-hidden="true" className="fill-current" /> : TAB_META[value].label}
                  </TabsTrigger>
                ))}
              </TabsList>
              <Button
                ref={trashRef}
                type="button"
                variant="ghost"
                size="icon-lg"
                className="size-11 shrink-0"
                aria-label={`Clear ${meta.clear}`}
                disabled={!canClear}
                onClick={openConfirm}
              >
                <Trash2 />
              </Button>
            </div>
          </Tabs>

          {visible.length > 0 ? (
            <ul
              role="listbox"
              aria-label="Clipboard items"
              className="mt-2 flex h-72 flex-col gap-1 overflow-y-auto"
            >
              <AnimatePresence initial={false}>
                {visible.map((item, index) => {
                  const isSelected = selected?.id === item.id;
                  const KindIcon = KIND_ICON[item.kind];
                  return (
                    <motion.li
                      key={item.id}
                      ref={(node: HTMLLIElement | null) => {
                        if (node) rowRefs.current.set(item.id, node);
                        else rowRefs.current.delete(item.id);
                      }}
                      layout
                      role="option"
                      aria-selected={isSelected}
                      tabIndex={isSelected ? 0 : -1}
                      initial={item.id === lastArrived ? { opacity: 0, y: -8 } : false}
                      animate={{ opacity: 1, y: 0 }}
                      exit={{ opacity: 0 }}
                      transition={{ duration: 0.2 }}
                      onClick={() => select(item.id)}
                      onKeyDown={(event) => handleRowKey(event, index, item.id)}
                      className={cn(
                        "flex min-h-11 cursor-pointer items-center gap-2.5 rounded-lg px-2.5 text-sm outline-none focus-visible:ring-3 focus-visible:ring-ring/50",
                        isSelected ? "bg-primary/15 ring-1 ring-primary/40" : "hover:bg-muted",
                      )}
                    >
                      <KindIcon aria-hidden="true" className="size-4 shrink-0 text-muted-foreground" />
                      <span className="min-w-0 flex-1 truncate">{item.label}</span>
                      {item.pinned && (
                        <Badge variant="secondary" className="shrink-0 px-1">
                          <Star aria-hidden="true" className="fill-current" />
                          <span className="sr-only">Pinned</span>
                        </Badge>
                      )}
                      <span className="shrink-0 text-xs tabular-nums text-muted-foreground">{AGE[item.id]}</span>
                    </motion.li>
                  );
                })}
              </AnimatePresence>
            </ul>
          ) : (
            <div className="mt-2 flex h-72 items-center justify-center text-sm text-muted-foreground">
              {query ? "No matches." : "Nothing here yet."}
            </div>
          )}

          <p role="status" className="mt-2 min-h-4 text-xs text-muted-foreground">
            {notice}
          </p>
          <p className="mt-1 font-mono text-xs text-muted-foreground">↵ paste  ⇧↵ plain  ⌫ remove</p>
        </div>

        <p className="mt-3 text-sm text-muted-foreground">
          This is a working demo. Try the search, the tabs and the trash button.
        </p>

        <AlertDialog open={confirmOpen} onOpenChange={setConfirmOpen}>
          <AlertDialogContent
            size="sm"
            finalFocus={() => (canClear ? trashRef.current : searchRef.current)}
          >
            <AlertDialogHeader>
              <AlertDialogTitle>{`Clear ${meta.clear}?`}</AlertDialogTitle>
              <AlertDialogDescription>
                {clearable > 0
                  ? `This removes all ${clearable} ${meta.noun} and cannot be undone.`
                  : "Nothing to clear."}
              </AlertDialogDescription>
            </AlertDialogHeader>
            {tab !== "pinned" && (
              <div className="flex min-h-11 items-center gap-3">
                <Checkbox
                  id={checkboxId}
                  checked={includePinned}
                  onCheckedChange={(checked) => setIncludePinned(checked === true)}
                />
                <label htmlFor={checkboxId} className="text-sm">
                  Also remove pinned items
                </label>
              </div>
            )}
            <AlertDialogFooter>
              {clearable > 0 ? (
                <>
                  <AlertDialogCancel className="h-11">Cancel</AlertDialogCancel>
                  <AlertDialogAction variant="destructive" className="h-11" onClick={confirmClear}>
                    Clear
                  </AlertDialogAction>
                </>
              ) : (
                <AlertDialogCancel className="h-11">Close</AlertDialogCancel>
              )}
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </div>
    </MotionConfig>
  );
}
