import { useState, type ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { CommandLine } from "@/components/command-line";
import { config } from "@/config";
import { useReleases } from "@/hooks/use-releases";
import { formatBytes } from "@/lib/format";
import { useSelectedPlatform, type SelectedPlatform } from "@/lib/platform-context";
import { CHECKSUM_VERIFY_COMMAND, windowsVerifyCommands } from "@/lib/install-commands";
import { latestStable, type Release, type ReleaseAsset } from "@/lib/releases";

const RELEASES_URL = `https://github.com/${config.repo}/releases`;
const BODY_CLASS = "flex min-w-0 flex-col";
const dateFormat = new Intl.DateTimeFormat("en", { dateStyle: "medium" });

function formatDate(iso: string): string {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? "" : dateFormat.format(date);
}

/** A download button with its size shown next to it (kept outside the button for contrast). */
function AssetButton({
  asset,
  label,
  text,
  variant = "default",
}: {
  asset: ReleaseAsset;
  label: string;
  text: string;
  variant?: "default" | "outline" | "secondary";
}) {
  return (
    <div className="flex items-center gap-2">
      <Button
        variant={variant}
        className={variant === "default" ? "h-11 px-4 hover:bg-primary-hover" : "h-11 px-4"}
        render={<a href={asset.url} rel="noopener" aria-label={label} />}
      >
        {text}
      </Button>
      {asset.size > 0 && (
        <span className="text-xs tabular-nums text-muted-foreground">{formatBytes(asset.size)}</span>
      )}
    </div>
  );
}

/** One platform's files in the release card. Missing files show the coming-soon state. */
function PlatformFiles({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="mt-3 flex flex-col gap-2">
      <h4 className="text-sm font-medium text-foreground">{title}</h4>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">{children}</div>
    </div>
  );
}

function ComingSoon({ what }: { what: string }) {
  return (
    <p className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
      <Badge variant="secondary" className="bg-brand-yellow-soft text-brand-yellow">
        Coming soon
      </Badge>
      <span>{what}</span>
    </p>
  );
}

function ReleaseBlock({
  release,
  isLatest,
  platform,
}: {
  release: Release;
  isLatest: boolean;
  platform: SelectedPlatform;
}) {
  const { version, deb, windowsInstaller, windowsPortable } = release;
  const sums = release.sha256sums ?? release.checksums;
  const date = formatDate(release.date);
  const windowsCommands = windowsVerifyCommands(release);
  const hasWindows = Boolean(windowsInstaller || windowsPortable);

  return (
    <div
      data-release-block
      className="relative overflow-hidden rounded-xl border border-border bg-card p-4 text-card-foreground sm:p-5"
    >
      <div aria-hidden="true" className="absolute inset-y-0 left-0 w-1 bg-secondary-accent" />
      <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
        <h3 className="font-heading text-lg font-semibold tracking-[-0.02em]">Clipsy {version}</h3>
        {isLatest && (
          <Badge variant="secondary" className="bg-brand-teal-soft text-brand-teal">
            Latest
          </Badge>
        )}
        {release.prerelease && <Badge variant="outline">Pre-release</Badge>}
        {date && (
          <span className="text-sm text-muted-foreground sm:ml-auto">
            <time dateTime={release.date}>{date}</time>
          </span>
        )}
      </div>

      <PlatformFiles title="Linux">
        {deb ? (
          <AssetButton asset={deb} label={`Download .deb, version ${version}`} text="Download .deb" />
        ) : (
          <p className="text-sm text-muted-foreground">No .deb package for this version.</p>
        )}
      </PlatformFiles>

      <PlatformFiles title="Windows">
        {windowsInstaller && (
          <AssetButton
            asset={windowsInstaller}
            label={`Installer (.exe), Windows setup for ${version}`}
            text="Installer (.exe)"
          />
        )}
        {windowsPortable && (
          <AssetButton
            asset={windowsPortable}
            variant="outline"
            label={`Portable (.zip), Windows for ${version}`}
            text="Portable (.zip)"
          />
        )}
        {!hasWindows && <ComingSoon what="The Windows download for this version is coming soon." />}
      </PlatformFiles>

      {sums && (
        <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-2">
          <AssetButton
            asset={sums}
            variant="secondary"
            label={`SHA256SUMS, checksums for ${version}`}
            text="SHA256SUMS"
          />
        </div>
      )}

      {platform === "windows"
        ? windowsCommands.length > 0 && (
            <div className="mt-3">
              <p className="mb-1.5 text-sm text-muted-foreground">Check your download in PowerShell:</p>
              <div className="flex flex-col gap-1.5">
                {windowsCommands.map((command) => (
                  <CommandLine key={command} text={command} label="Copy PowerShell command" wrap />
                ))}
              </div>
            </div>
          )
        : sums && (
            <div className="mt-3">
              <p className="mb-1.5 text-sm text-muted-foreground">Check your download with:</p>
              <CommandLine text={CHECKSUM_VERIFY_COMMAND} label="Copy checksum command" />
            </div>
          )}

      <p className="mt-3 text-sm text-muted-foreground">
        Changes and details for this release are in the{" "}
        <a
          href={release.notesUrl}
          target="_blank"
          rel="noopener"
          className="font-medium text-secondary-accent underline underline-offset-4 focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none"
        >
          release notes on GitHub
        </a>
        .
      </p>
    </div>
  );
}

function TableLink({ href, label, text }: { href: string; label: string; text: string }) {
  return (
    <a
      href={href}
      rel="noopener"
      aria-label={label}
      className="inline-flex min-h-11 items-center rounded-md px-2 font-medium text-foreground underline underline-offset-4 focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none"
    >
      {text}
    </a>
  );
}

function AssetLink({ asset, label, text }: { asset?: ReleaseAsset; label: string; text: string }) {
  if (!asset) return <span className="text-muted-foreground">Not available</span>;
  return <TableLink href={asset.url} label={label} text={text} />;
}

/** A short list of what a version ships, for the mobile version picker. */
function shipsSummary(release: Release): string {
  const linux = release.deb ? ".deb" : null;
  const windows = release.windowsInstaller || release.windowsPortable ? "Windows" : null;
  const parts = [linux, windows].filter(Boolean);
  return parts.length > 0 ? parts.join(", ") : "no installers yet";
}

/** The heading stays fixed above the scrolling body (see ScrollPanel in App). */
export function DownloadsHeading() {
  return (
    <div>
      <h2 id="downloads-heading" className="font-heading text-[clamp(1.25rem,1.05rem+0.7vw,1.75rem)] font-semibold tracking-[-0.02em]">
        Downloads
      </h2>
      <p className="mt-1 max-w-[60ch] text-muted-foreground">
        The latest version is on top. Older versions stay available.
      </p>
    </div>
  );
}

export function DownloadsSection() {
  const { releases, status } = useReleases();
  const { platform } = useSelectedPlatform();
  const [selectedTag, setSelectedTag] = useState<string | null>(null);

  if (status === "loading") {
    return (
      <div className={BODY_CLASS} aria-busy="true">
        <div className="flex flex-col gap-4">
          <Skeleton aria-hidden="true" className="h-44 w-full" />
          <Skeleton aria-hidden="true" className="h-24 w-full" />
        </div>
      </div>
    );
  }

  if (status === "empty" || releases.length === 0) {
    return (
      <div className={BODY_CLASS}>
        <div>
          <p className="text-muted-foreground">No releases are published yet.</p>
          <Button
            variant="outline"
            className="mt-4 h-11 px-4"
            render={<a href={RELEASES_URL} target="_blank" rel="noopener" />}
          >
            View releases on GitHub
          </Button>
        </div>
      </div>
    );
  }

  const latest = latestStable(releases);
  const shown = releases.find((release) => release.tag === selectedTag) ?? latest;
  const previous = releases.filter((release) => release.tag !== latest?.tag);

  return (
    <div className={BODY_CLASS}>
      <div aria-live="polite" className="mt-2 min-h-0 text-sm text-muted-foreground empty:hidden">
        {status === "snapshot" &&
          "Showing the last saved release list because GitHub could not be reached."}
      </div>

      <div aria-live="polite" className="mt-4">
        {shown ? (
          <ReleaseBlock release={shown} isLatest={shown.tag === latest?.tag} platform={platform} />
        ) : (
          <p className="rounded-xl border border-border bg-card p-4 text-muted-foreground sm:p-5">
            No stable release yet.
          </p>
        )}
      </div>

      <div className="mt-5">
        <div className="mb-2 flex flex-wrap items-center justify-between gap-3">
          <h3 id="previous-versions-heading" className="font-heading text-base font-semibold tracking-[-0.01em]">
            Previous versions
          </h3>
          <div className="md:hidden">
            <DropdownMenu>
              <DropdownMenuTrigger render={<Button variant="outline" className="h-11 px-4" />}>
                {shown && <span className="sr-only">Choose a version: </span>}
                {shown ? `Version ${shown.version}` : "Choose a version"}
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                {releases.map((release) => (
                  <DropdownMenuItem
                    key={release.tag}
                    onClick={() => setSelectedTag(release.tag)}
                    className="flex flex-col items-start gap-0.5"
                  >
                    <span>
                      Clipsy {release.version}
                      {release.prerelease ? " (pre-release)" : ""}
                    </span>
                    <span className="text-xs text-muted-foreground">{shipsSummary(release)}</span>
                  </DropdownMenuItem>
                ))}
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>

        {previous.length > 0 ? (
          <Table aria-labelledby="previous-versions-heading">
            <TableHeader>
              <TableRow>
                <TableHead scope="col">Version</TableHead>
                <TableHead scope="col">Date</TableHead>
                <TableHead scope="col">.deb</TableHead>
                <TableHead scope="col">Windows</TableHead>
                <TableHead scope="col">Notes</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {previous.map((release) => (
                <TableRow key={release.tag}>
                  <TableCell>
                    <div className="flex items-center gap-2">
                      <span className="tabular-nums">{release.version}</span>
                      {release.prerelease && <Badge variant="outline">Pre-release</Badge>}
                    </div>
                  </TableCell>
                  <TableCell>{formatDate(release.date)}</TableCell>
                  <TableCell>
                    <AssetLink
                      asset={release.deb}
                      label={`.deb, version ${release.version}`}
                      text=".deb"
                    />
                  </TableCell>
                  <TableCell>
                    {release.windowsInstaller ? (
                      <TableLink
                        href={release.windowsInstaller.url}
                        label={`.exe, Windows installer for ${release.version}`}
                        text=".exe"
                      />
                    ) : (
                      <span className="text-muted-foreground">
                        <span aria-hidden="true">—</span>
                        <span className="sr-only">No Windows download</span>
                      </span>
                    )}
                  </TableCell>
                  <TableCell>
                    <TableLink
                      href={release.notesUrl}
                      label={`Release notes for ${release.version}`}
                      text="Notes"
                    />
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        ) : (
          <p className="text-sm text-muted-foreground">No other releases yet.</p>
        )}
      </div>
    </div>
  );
}
