import type { ReactNode } from "react";
import { EyeOff, FileText, Image as ImageIcon, Lock, Search, Star, Trash2, Type } from "lucide-react";
import { cn } from "cn";
import { Card } from "@/components/ui/card";
import { Spotlight } from "@/components/spotlight";
import { Tilt } from "@/components/tilt";

/*
 * Small illustrations, one per tile. They are decorative (aria-hidden) and use
 * only theme tokens, so they follow light and dark.
 */

function HistoryArt() {
  const rows = [
    { icon: Type, tone: "bg-brand-teal-soft text-brand-teal", label: "git clone git@github.com:…", age: "now" },
    { icon: ImageIcon, tone: "bg-brand-violet-soft text-brand-violet", label: "screenshot-2026-11-19.png", age: "9m" },
    { icon: FileText, tone: "bg-brand-yellow-soft text-brand-yellow", label: "invoice.pdf", age: "1h" },
  ];
  return (
    <div aria-hidden="true" className="relative mt-auto pt-5">
      <div className="flex flex-col gap-1.5">
        {rows.map((row, index) => (
          <div
            key={row.label}
            style={{ marginLeft: `${index * 0.75}rem`, marginRight: `${(2 - index) * 0.75}rem` }}
            className="flex items-center gap-2.5 rounded-lg border border-border bg-card px-2.5 py-2 text-xs shadow-float transition-transform duration-300 group-hover/tile:translate-x-1"
          >
            <span className={cn("flex size-6 items-center justify-center rounded-md", row.tone)}>
              <row.icon className="size-3.5" />
            </span>
            <span className="min-w-0 flex-1 truncate font-mono text-foreground">{row.label}</span>
            <span className="text-muted-foreground">{row.age}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function SearchArt() {
  return (
    <div aria-hidden="true" className="mt-auto pt-4">
      <div className="flex h-9 items-center gap-2 rounded-lg border border-secondary-accent/60 bg-card px-2.5 text-xs ring-3 ring-secondary-accent/15">
        <Search className="size-3.5 text-muted-foreground" />
        <span className="font-mono text-foreground">inv</span>
        <span className="h-4 w-px bg-foreground motion-safe:animate-pulse" />
      </div>
      <p className="mt-1.5 truncate px-1 font-mono text-xs text-muted-foreground">
        <span className="rounded-sm bg-brand-yellow-soft px-0.5 text-foreground">inv</span>oice.pdf
      </p>
    </div>
  );
}

function PinArt() {
  return (
    <div aria-hidden="true" className="mt-auto flex items-center gap-2 pt-4">
      <span className="flex size-9 items-center justify-center rounded-lg bg-brand-yellow-soft text-brand-yellow transition-transform duration-300 group-hover/tile:-rotate-12 group-hover/tile:scale-110">
        <Star className="size-5 fill-current" />
      </span>
      <span className="h-2 w-16 rounded-full bg-muted" />
    </div>
  );
}

function ClearArt() {
  const tabs = ["All", "Text", "Images", "Files"];
  return (
    <div aria-hidden="true" className="mt-auto flex flex-wrap items-center gap-1.5 pt-4">
      {tabs.map((tab) => (
        <span
          key={tab}
          className={cn(
            "rounded-md border px-2 py-1 text-xs",
            tab === "Images"
              ? "border-primary/50 bg-primary/10 text-foreground line-through decoration-primary"
              : "border-border bg-card text-muted-foreground",
          )}
        >
          {tab}
        </span>
      ))}
      <span className="ml-1 flex size-9 items-center justify-center rounded-lg bg-brand-teal-soft text-brand-teal transition-transform duration-300 group-hover/tile:-rotate-6">
        <Trash2 className="size-4" />
      </span>
      <span className="inline-flex items-center gap-1 rounded-md bg-brand-yellow-soft px-2 py-1 text-xs text-foreground">
        <Star className="size-3 fill-current text-brand-yellow" /> kept
      </span>
    </div>
  );
}

function PrivateArt() {
  return (
    <div aria-hidden="true" className="mt-auto flex items-center gap-2 pt-4">
      <span className="flex size-9 items-center justify-center rounded-lg bg-brand-violet-soft text-brand-violet">
        <Lock className="size-5" />
      </span>
      <span className="flex items-center gap-1.5 rounded-md border border-dashed border-border px-2 py-1 font-mono text-xs text-muted-foreground">
        <EyeOff className="size-3.5" /> ••••••••
      </span>
    </div>
  );
}

function DesktopArt() {
  return (
    <div aria-hidden="true" className="mt-auto flex flex-wrap gap-1.5 pt-4">
      <span className="rounded-md bg-brand-teal-soft px-2 py-1 font-mono text-xs text-brand-teal">X11</span>
      <span className="rounded-md bg-brand-violet-soft px-2 py-1 font-mono text-xs text-brand-violet">GNOME Wayland</span>
      <span className="rounded-md bg-brand-yellow-soft px-2 py-1 font-mono text-xs text-brand-yellow">Windows 10/11</span>
    </div>
  );
}

/** `span` sizes the grid cell (kept on the tilt wrapper); `tone` is the card's background. */
type Feature = { name: string; description: string; art: ReactNode; span: string; tone: string };

const features: Feature[] = [
  {
    name: "Remembers text, images and files",
    description: "Everything you copy stays in your history, so you can paste it again later.",
    art: <HistoryArt />,
    span: "sm:col-span-2 lg:col-span-1",
    tone: "[background-image:linear-gradient(color-mix(in_oklch,var(--brand-teal-soft)_50%,transparent),color-mix(in_oklch,var(--brand-teal-soft)_50%,transparent))]",
  },
  {
    name: "Search in a keystroke",
    description: "Type to filter your history as you go.",
    art: <SearchArt />,
    span: "",
    tone: "",
  },
  {
    name: "Pin what you reuse",
    description: "Pinned items stay at the top and are kept until you remove them.",
    art: <PinArt />,
    span: "",
    tone: "",
  },
  {
    name: "Clear one tab at a time",
    description:
      "Clear text, images or files separately. Pinned items are kept unless you choose to remove them too.",
    art: <ClearArt />,
    span: "sm:col-span-2 lg:col-span-1",
    tone: "",
  },
  {
    name: "Private by default",
    description:
      "Copies from password managers are skipped, and your history stays on your computer.",
    art: <PrivateArt />,
    span: "",
    tone: "",
  },
  {
    name: "Works where you work",
    description: "Runs on Linux (X11 and GNOME with Wayland) and on Windows 10 and 11.",
    art: <DesktopArt />,
    span: "",
    tone: "",
  },
];

export function FeaturesSection() {
  return (
    <div>
      <div>
        <h2 className="font-heading text-2xl font-semibold tracking-tight sm:text-3xl">What Clipsy does</h2>
        <p className="mt-1 max-w-[60ch] text-muted-foreground">
          Clipsy watches what you copy, keeps it in order, and stays out of the way until you
          need it again.
        </p>
      </div>

      <dl className="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 lg:gap-4">
        {features.map((feature) => (
          // The wrapper stretches to the grid row; [&>div] makes the tilt layer fill it too,
          // so cards in the same row end at the same height.
          <Tilt key={feature.name} max={4} className={cn("h-full [&>div]:h-full", feature.span)}>
            <Spotlight className="h-full rounded-xl">
              <Card
                className={cn(
                  "group/tile h-full gap-0 bg-background/80 p-3.5 ring-border backdrop-blur-sm transition-[transform,box-shadow] duration-150 hover:-translate-y-0.5 hover:shadow-float hover:ring-primary/40 motion-reduce:transition-none motion-reduce:hover:translate-y-0",
                  feature.tone,
                )}
              >
                <dt className="font-heading text-base leading-snug font-semibold tracking-[-0.01em] text-foreground">
                  {feature.name}
                </dt>
                <dd className="mt-1 flex flex-1 flex-col text-sm text-muted-foreground">
                  <p>{feature.description}</p>
                  {feature.art}
                </dd>
              </Card>
            </Spotlight>
          </Tilt>
        ))}
      </dl>
    </div>
  );
}
