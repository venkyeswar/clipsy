import type { ReactNode } from "react";
import { Download } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { PopupDemo } from "@/components/popup-demo";
import { config, shortcutFor } from "@/config";
import { useReleases } from "@/hooks/use-releases";
import { useSelectedPlatform } from "@/lib/platform-context";
import { latestStable, latestWithWindows, windowsStatus } from "@/lib/releases";

const releasesUrl = `https://github.com/${config.repo}/releases`;
const dateFormat = new Intl.DateTimeFormat("en", { dateStyle: "medium" });

function formatDate(value: string): string | null {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : dateFormat.format(date);
}

const DOWNLOAD_BUTTON = "h-11 w-full px-5 text-[0.9375rem] sm:w-auto sm:min-w-64";
const PRIMARY_CLASS = `${DOWNLOAD_BUTTON} shadow-[0_8px_20px_-10px_var(--primary)] hover:bg-primary-hover`;
const SECONDARY_CLASS = `${DOWNLOAD_BUTTON} bg-card dark:bg-card`;

/** One platform's download column. The visitor's platform gets the filled button; the other is outlined. */
function DownloadSlot({ children }: { children: ReactNode }) {
  return <div className="flex flex-col gap-1.5 sm:items-start">{children}</div>;
}

export function Hero() {
  const { releases, status } = useReleases();
  const { platform, setPlatform } = useSelectedPlatform();
  const latest = latestStable(releases);
  const debUrl = latest?.deb?.url;
  const winRelease = latestWithWindows(releases);
  const winUrl = winRelease?.windowsInstaller?.url;
  const winAvailable = windowsStatus(releases) === "available" && Boolean(winUrl);
  const releaseDate = latest ? formatDate(latest.date) : null;
  const loading = status === "loading";
  const linuxIsPrimary = platform !== "windows";

  let linux: ReactNode;
  if (loading) {
    linux = (
      <div className="h-11 w-full sm:w-64">
        <Skeleton className="h-full w-full rounded-lg" />
        <span className="sr-only">Loading the latest release</span>
      </div>
    );
  } else if (latest && debUrl) {
    linux = (
      <Button
        size="lg"
        className={linuxIsPrimary ? PRIMARY_CLASS : SECONDARY_CLASS}
        variant={linuxIsPrimary ? "default" : "outline"}
        render={<a href={debUrl} />}
      >
        <Download aria-hidden="true" className="size-4" />
        Download for Linux (.deb)
      </Button>
    );
  } else {
    linux = (
      <Button
        size="lg"
        variant={linuxIsPrimary ? "default" : "outline"}
        className={linuxIsPrimary ? PRIMARY_CLASS : SECONDARY_CLASS}
        render={<a href={releasesUrl} target="_blank" rel="noopener" />}
      >
        View on GitHub
      </Button>
    );
  }

  const windowsPrimary = !linuxIsPrimary;
  let windows: ReactNode;
  if (loading) {
    windows = (
      <div className="h-11 w-full sm:w-64">
        <Skeleton className="h-full w-full rounded-lg" />
        <span className="sr-only">Loading the Windows download</span>
      </div>
    );
  } else if (winAvailable) {
    windows = (
      <Button
        size="lg"
        variant={windowsPrimary ? "default" : "outline"}
        className={windowsPrimary ? PRIMARY_CLASS : SECONDARY_CLASS}
        render={<a href={winUrl} />}
      >
        <Download aria-hidden="true" className="size-4" />
        Download for Windows (.exe)
      </Button>
    );
  } else {
    // Visibly disabled: aria-disabled keeps it in the focus order and announced, with no link.
    windows = (
      <>
        <Button
          size="lg"
          variant={windowsPrimary ? "default" : "outline"}
          aria-disabled="true"
          aria-describedby="windows-coming-soon"
          className={`${windowsPrimary ? PRIMARY_CLASS : SECONDARY_CLASS} cursor-not-allowed opacity-60 hover:bg-primary`}
        >
          <Download aria-hidden="true" className="size-4" />
          Download for Windows (.exe)
        </Button>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
          <Badge id="windows-coming-soon" variant="secondary" className="bg-brand-yellow-soft text-brand-yellow">
            Coming soon
          </Badge>
          <a
            href="#install"
            onClick={() => setPlatform("windows")}
            className="inline-flex min-h-11 items-center text-sm font-medium text-secondary-accent underline decoration-1 underline-offset-4 hover:decoration-2"
          >
            Build it from source on Windows
          </a>
        </div>
      </>
    );
  }

  return (
    <section
      id="top-hero"
      aria-labelledby="hero-title"
      className="relative isolate grid grid-cols-[minmax(0,1fr)] items-center gap-10 py-8 sm:py-10 lg:grid-cols-[minmax(0,1fr)_25rem] lg:gap-16 lg:py-14 xl:grid-cols-[minmax(0,1fr)_27rem] 3xl:grid-cols-[minmax(0,1fr)_34rem]"
    >
      <div aria-hidden="true" className="hero-dots pointer-events-none absolute inset-y-0 -right-16 left-1/3 -z-10" />
      <div className="flex min-w-0 flex-col gap-5">
        <h1
          id="hero-title"
          className="max-w-[14ch] font-heading text-[clamp(2.5rem,1.6rem+2.6vw,4.5rem)] leading-[1.02] font-semibold tracking-[-0.035em] text-balance"
        >
          Everything you copied, one shortcut away.
        </h1>
        <p className="max-w-[46ch] text-base text-muted-foreground sm:text-lg sm:leading-relaxed">
          Clipsy keeps your clipboard history on your computer, on Linux and Windows. Press the shortcut, search,
          and paste anything you copied earlier.
        </p>

        <div className="mt-1 flex flex-col gap-4 sm:flex-row sm:flex-wrap sm:items-start sm:gap-x-4 sm:gap-y-3">
          <DownloadSlot>{linux}</DownloadSlot>
          <DownloadSlot>{windows}</DownloadSlot>
        </div>

        <div className="flex flex-col gap-x-4 text-sm text-muted-foreground sm:flex-row sm:flex-wrap sm:items-center">
          <p data-testid="version-line" className="min-h-5">
            {latest ? `Version ${latest.version}${releaseDate ? `, released ${releaseDate}` : ""}.` : null}
          </p>
          <a
            href="#install"
            className="inline-flex min-h-11 items-center font-medium text-secondary-accent underline decoration-1 underline-offset-4 hover:decoration-2"
          >
            Other ways to install
          </a>
          <a
            href="#install"
            className="inline-flex min-h-11 items-center font-medium text-secondary-accent underline decoration-1 underline-offset-4 hover:decoration-2"
          >
            Can't use sudo? Install to your home folder.
          </a>
        </div>
      </div>

      <PopupDemo shortcut={shortcutFor(platform)} />
    </section>
  );
}
