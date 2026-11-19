import type { ReactNode } from "react";
import { Download } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
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
      <div className="h-11 w-full sm:w-64">
        <Skeleton className="h-full w-full rounded-lg" />
        <span className="sr-only">Loading the latest release</span>
      </div>
    );
  } else if (latest && debUrl) {
    primary = (
      <Button size="lg" className="h-11 w-full px-5 text-[0.9375rem] shadow-[0_8px_20px_-10px_var(--primary)] hover:bg-primary-hover sm:w-64" render={<a href={debUrl} />}>
        <Download aria-hidden="true" className="size-4" />
        Download .deb v{latest.version}
      </Button>
    );
  } else {
    primary = (
      <Button
        size="lg"
        className="h-11 w-full px-5 text-[0.9375rem] shadow-[0_8px_20px_-10px_var(--primary)] hover:bg-primary-hover sm:w-64"
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
      className="relative isolate grid grid-cols-[minmax(0,1fr)] items-center gap-10 py-8 sm:py-10 lg:grid-cols-[minmax(0,1fr)_25rem] lg:gap-16 lg:py-14 xl:grid-cols-[minmax(0,1fr)_27rem]"
    >
      <div aria-hidden="true" className="hero-dots pointer-events-none absolute inset-y-0 -right-16 left-1/3 -z-10" />
      <div className="flex min-w-0 flex-col gap-5">
        <h1
          id="hero-title"
          className="max-w-[14ch] font-heading text-[2.5rem] leading-[1.02] font-semibold tracking-[-0.035em] text-balance sm:text-[3.25rem] lg:text-[3.75rem]"
        >
          Everything you copied, one shortcut away.
        </h1>
        <p className="max-w-[46ch] text-base text-muted-foreground sm:text-lg sm:leading-relaxed">
          Clipsy keeps your clipboard history on your computer. Press the shortcut, search, and paste anything
          you copied earlier.
        </p>

        <div className="mt-1 flex flex-col gap-2.5 sm:flex-row sm:flex-wrap sm:items-center">
          {primary}
          <Button
            variant="outline"
            size="lg"
            className="h-11 bg-card px-5 text-[0.9375rem] dark:bg-card"
            render={<a href="#install" />}
          >
            Other ways to install
          </Button>
        </div>

        <div className="flex flex-col gap-x-4 text-sm text-muted-foreground sm:flex-row sm:items-center">
          <p data-testid="version-line" className="min-h-5">
            {latest ? `Version ${latest.version}${releaseDate ? `, released ${releaseDate}` : ""}.` : null}
          </p>
          <a
            href="#install"
            className="inline-flex min-h-11 items-center font-medium text-secondary-accent underline decoration-1 underline-offset-4 hover:decoration-2"
          >
            Can't use sudo? Install to your home folder.
          </a>
        </div>
      </div>

      <PopupDemo />
    </section>
  );
}
