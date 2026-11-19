import { useCallback, useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { AnimatePresence, MotionConfig, motion } from "motion/react";
import { Copy, FileText, Image as ImageIcon, Search, Star, Trash2, Type, type LucideIcon } from "lucide-react";
import { cn } from "cn";
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Tabs, TabsIndicator, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Keycap } from "@/components/keycap";
import { config } from "@/config";
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
/** Each kind gets one of the logo colours, so rows scan by type at a glance. */
const KIND_TONE: Record<DemoKind, string> = {
  text: "bg-brand-teal-soft text-brand-teal",
  image: "bg-brand-violet-soft text-brand-violet",
  files: "bg-brand-yellow-soft text-brand-yellow",
};

const TAB_META: Record<DemoTab, { label: string; clear: string; noun: string }> = {
  all: { label: "All", clear: "everything", noun: "items" },
  text: { label: "Text", clear: "text items", noun: "text items" },
  images: { label: "Images", clear: "images", noun: "images" },
  files: { label: "Files", clear: "files", noun: "files" },
  pinned: { label: "Pinned", clear: "pinned items", noun: "pinned items" },
};
const TABS: DemoTab[] = ["all", "text", "images", "files", "pinned"];

/** The opening beat: the shortcut keys press one by one, then the window opens. */
const KEY_PRESS_AT = [150, 300, 450];
const KEY_RELEASE_AT = 800;
const WINDOW_DELAY_S = 0.65;
/** Everything after the window has opened is shifted by this much. */
const OFFSET = 900;

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
export function PopupDemo({
  autoplay = true,
  shortcut = config.shortcut,
}: {
  autoplay?: boolean;
  /** Key labels for the opening shortcut chip (the Windows default differs). */
  shortcut?: string[];
}) {
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
  const [chipId, setChipId] = useState<number | null>(null);
  const [pressedKeys, setPressedKeys] = useState(0);
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
    setChipId(null);
    setPressedKeys(0);
  }, []);

  useEffect(() => {
    if (!playingRef.current) return;
    const arrive = (id: number) => {
      const seed = SEED.find((item) => item.id === id);
      if (!seed) return;
      setItems((prev) => (prev.some((item) => item.id === id) ? prev : [seed, ...prev]));
      setLastArrived(id);
      setChipId(id);
    };
    const steps: [number, () => void][] = [
      ...KEY_PRESS_AT.map((at, index): [number, () => void] => [at, () => setPressedKeys(index + 1)]),
      [KEY_RELEASE_AT, () => setPressedKeys(0)],
      [OFFSET + 500, () => arrive(4)],
      [OFFSET + 1100, () => arrive(2)],
      [OFFSET + 1700, () => arrive(1)],
      [OFFSET + 2400, () => setChipId(null)],
      [OFFSET + 2500, () => setQuery("g")],
      [OFFSET + 2620, () => setQuery("gi")],
      [OFFSET + 2740, () => setQuery("git")],
      [OFFSET + 3900, () => setQuery("")],
      [
        OFFSET + 4400,
        () => {
          setItems((prev) => prev.map((item) => (item.id === STAR_ID ? { ...item, pinned: true } : item)));
          setLastArrived(null);
        },
      ],
      [
        OFFSET + 4900,
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
  const chip = chipId === null ? undefined : SEED.find((item) => item.id === chipId);

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
      <div className="relative isolate mx-auto w-full pt-8">
        {/* Depth: the logo's violet and yellow sheets fanned behind the window. */}
        <motion.div
          aria-hidden="true"
          initial={startPlaying ? { rotate: 0, opacity: 0 } : false}
          animate={{ rotate: -7, opacity: 1 }}
          transition={{ delay: WINDOW_DELAY_S + 0.1, type: "spring", stiffness: 160, damping: 18 }}
          className="sheet-violet absolute inset-x-5 top-14 bottom-16 -z-10 origin-bottom rounded-3xl"
        />
        <motion.div
          aria-hidden="true"
          initial={startPlaying ? { rotate: 0, opacity: 0 } : false}
          animate={{ rotate: 6, opacity: 1 }}
          transition={{ delay: WINDOW_DELAY_S + 0.16, type: "spring", stiffness: 160, damping: 18 }}
          className="sheet-yellow absolute inset-x-5 top-14 bottom-16 -z-10 origin-bottom rounded-3xl"
        />

        {/* The shortcut, pressed during the opening beat. */}
        <div
          role="group"
          aria-label="Default shortcut"
          className="absolute top-0 left-3 z-20 flex items-center gap-1 rounded-xl border border-border bg-card/95 px-2 py-1.5 text-xs text-muted-foreground shadow-float backdrop-blur sm:-left-6"
        >
          {shortcut.map((key, index) => (
            <span key={key} className="inline-flex items-center gap-1">
              {index > 0 && <span aria-hidden="true">+</span>}
              <Keycap pressed={pressedKeys > index}>{key}</Keycap>
            </span>
          ))}
          <span className="ml-1 hidden sm:inline">opens Clipsy</span>
        </div>

        {/* A copied item flying into the history. */}
        <AnimatePresence>
          {chip && (
            <motion.div
              key={chip.id}
              aria-hidden="true"
              initial={{ opacity: 0, x: -28, y: 6, scale: 0.9 }}
              animate={{ opacity: 1, x: 0, y: 0, scale: 1 }}
              exit={{ opacity: 0, x: 24, y: 18, scale: 0.85 }}
              transition={{ type: "spring", stiffness: 320, damping: 24 }}
              className="absolute top-[13rem] right-3 z-20 flex max-w-[13rem] items-center gap-1.5 rounded-full border border-border bg-card py-1 pr-3 pl-1.5 text-xs font-medium text-foreground shadow-float sm:-right-20"
            >
              <span className="flex size-5 items-center justify-center rounded-full bg-brand-teal-soft text-brand-teal">
                <Copy className="size-3" />
              </span>
              <span className="truncate">{chip.label}</span>
            </motion.div>
          )}
        </AnimatePresence>

        <motion.div
          initial={startPlaying ? { opacity: 0, y: 14, scale: 0.96 } : false}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          transition={{ delay: WINDOW_DELAY_S, type: "spring", stiffness: 260, damping: 24 }}
          className="dark relative"
        >
          <div
            role="region"
            aria-label="Interactive Clipsy demo"
            onPointerDownCapture={stopAutoplay}
            onKeyDownCapture={stopAutoplay}
            className="w-full min-w-0 max-w-full rounded-2xl border border-border bg-card p-2.5 text-card-foreground shadow-window"
          >
            <div className="flex items-center gap-2 px-1 pt-0.5 pb-2">
              <img src={`${import.meta.env.BASE_URL}logo.svg`} alt="" width={16} height={16} className="size-4" />
              <span className="font-heading text-xs font-semibold tracking-tight">Clipsy</span>
              <span className="ml-auto text-xs tabular-nums text-muted-foreground">
                {items.length} {items.length === 1 ? "item" : "items"}
              </span>
            </div>

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
                className="h-11 rounded-lg border-border bg-muted/60 pl-9 text-sm"
              />
            </div>

            <Tabs
              value={tab}
              onValueChange={(value) => {
                stopAutoplay();
                setTab(value as DemoTab);
                setNotice("");
              }}
              className="mt-2 min-w-0"
            >
              <div className="flex items-center gap-1.5">
                <TabsList aria-label="Filter clipboard items" className="relative h-auto! w-auto! min-w-0 flex-1 rounded-lg bg-muted/60 p-0.5">
                  <TabsIndicator className="bg-ember shadow-[0_4px_14px_-4px_var(--ember)]" />
                  {TABS.map((value) => (
                    <TabsTrigger
                      key={value}
                      value={value}
                      aria-label={value === "pinned" ? "Pinned" : undefined}
                      className="z-10 min-h-11 flex-1 px-1 text-xs transition-colors data-active:bg-transparent data-active:text-ember-foreground data-active:shadow-none dark:data-active:border-transparent dark:data-active:bg-transparent dark:data-active:text-ember-foreground"
                    >
                      {value === "pinned" ? <Star aria-hidden="true" className="size-3.5 fill-current" /> : TAB_META[value].label}
                    </TabsTrigger>
                  ))}
                </TabsList>
                <Button
                  ref={trashRef}
                  type="button"
                  variant="ghost"
                  size="icon-lg"
                  className="size-11 shrink-0 text-muted-foreground hover:text-destructive"
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
                className="relative mt-2 flex h-[14.5rem] flex-col gap-0.5 overflow-y-auto pr-0.5 [scrollbar-width:thin]"
              >
                <AnimatePresence initial={false} mode="popLayout">
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
                        initial={item.id === lastArrived ? { opacity: 0, y: -10, scale: 0.98 } : false}
                        animate={{ opacity: 1, y: 0, scale: 1 }}
                        exit={{ opacity: 0, x: 16 }}
                        transition={{ duration: 0.22 }}
                        onClick={() => select(item.id)}
                        onKeyDown={(event) => handleRowKey(event, index, item.id)}
                        className={cn(
                          "flex min-h-11 shrink-0 cursor-pointer items-center gap-2.5 rounded-lg px-2 text-[0.8125rem] outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring",
                          isSelected ? "bg-ember/16 ring-1 ring-ember/55" : "hover:bg-muted/70",
                        )}
                      >
                        <span className={cn("flex size-7 shrink-0 items-center justify-center rounded-md", KIND_TONE[item.kind])}>
                          <KindIcon aria-hidden="true" className="size-3.5" />
                        </span>
                        <span className="min-w-0 flex-1 truncate">{item.label}</span>
                        {item.pinned && (
                          <Badge variant="outline" className="shrink-0 border-transparent px-0.5 text-brand-yellow">
                            <Star aria-hidden="true" className="fill-current" />
                            <span className="sr-only">Pinned</span>
                          </Badge>
                        )}
                        <span className="w-7 shrink-0 text-right text-xs tabular-nums text-muted-foreground">{AGE[item.id]}</span>
                      </motion.li>
                    );
                  })}
                </AnimatePresence>
              </ul>
            ) : (
              <div className="mt-2 flex h-[14.5rem] items-center justify-center text-sm text-muted-foreground">
                {query ? "No matches." : "Nothing here yet."}
              </div>
            )}

            <div className="mt-1.5 flex min-h-5 items-center border-t border-border px-1 pt-1.5 text-xs text-muted-foreground">
              <p role="status" className={cn(notice ? "" : "sr-only")}>
                {notice}
              </p>
              {!notice && (
                <p aria-hidden="true" className="font-mono text-[0.6875rem] tracking-tight">
                  ↵ paste  ⇧↵ plain  ⌫ remove
                </p>
              )}
            </div>
          </div>
        </motion.div>

        <p className="mx-auto mt-4 max-w-[32ch] text-center text-xs text-balance text-muted-foreground">
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
