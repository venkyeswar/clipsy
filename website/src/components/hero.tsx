import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Keycap } from "@/components/keycap";
import { PopupDemo } from "@/components/popup-demo";
import { config } from "@/config";
import { useReleases } from "@/hooks/use-releases";
import { latestStable } from "@/lib/releases";

const releasesUrl = `https://github.com/${config.repo}/releases`;
const dateFormat = new Intl.DateTimeFormat("en", { dateStyle: "medium" });

function formatDate(value: string): string | null {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : dateFormat.format(date);
}

export function Hero() {
  const { releases, status } = useReleases();
  const latest = latestStable(releases);
  const debUrl = latest?.deb?.url;
  const releaseDate = latest ? formatDate(latest.date) : null;

  let primary: ReactNode;
  if (status === "loading") {
    primary = (
      <div className="h-11 w-60 max-w-full">
        <Skeleton className="h-full w-full rounded-lg" />
        <span className="sr-only">Loading the latest release</span>
      </div>
    );
  } else if (latest && debUrl) {
    primary = (
      <Button size="lg" className="h-11 px-5 text-base hover:brightness-95" render={<a href={debUrl} />}>
        Download .deb v{latest.version}
      </Button>
    );
  } else {
    primary = (
      <Button
        size="lg"
        className="h-11 px-5 text-base hover:brightness-95"
        render={<a href={releasesUrl} target="_blank" rel="noopener" />}
      >
        View on GitHub
      </Button>
    );
  }

  return (
    <section
      id="top-hero"
      aria-labelledby="hero-title"
      className="grid gap-10 py-12 lg:grid-cols-[minmax(0,1fr)_minmax(0,28rem)] lg:items-center lg:gap-12 lg:py-20"
    >
      <div className="flex min-w-0 flex-col gap-6">
        <h1
          id="hero-title"
          className="font-heading text-4xl font-semibold tracking-tight text-balance sm:text-5xl"
        >
          Everything you copied, one shortcut away.
        </h1>
        <p className="max-w-[60ch] text-lg text-muted-foreground">
          Clipsy keeps your clipboard history on your computer. Press the shortcut, search, and paste anything
          you copied earlier.
        </p>
        <div role="group" aria-label="Default shortcut" className="flex flex-wrap items-center gap-1.5 text-sm text-muted-foreground">
          <span>Default shortcut:</span>
          {config.shortcut.map((key, index) => (
            <span key={key} className="inline-flex items-center gap-1.5">
              {index > 0 && <span aria-hidden="true">+</span>}
              <Keycap>{key}</Keycap>
            </span>
          ))}
        </div>

        <div className="flex flex-col gap-3 sm:flex-row sm:flex-wrap sm:items-center">
          {primary}
          <Button
            variant="outline"
            size="lg"
            className="h-11 px-5 text-base"
            render={<a href="#install" />}
          >
            Other ways to install
          </Button>
        </div>

        {latest && (
          <p className="text-sm text-muted-foreground">
            Version {latest.version}
            {releaseDate ? `, released ${releaseDate}` : ""}.
          </p>
        )}

        <a
          href="#install"
          className="inline-flex min-h-11 items-center text-sm text-foreground underline underline-offset-4"
        >
          Can't use sudo? Install to your home folder.
        </a>
      </div>

      <PopupDemo />
    </section>
  );
}
