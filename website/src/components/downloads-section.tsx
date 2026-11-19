import { useState } from "react";
import { ChevronDown } from "lucide-react";
import { cn } from "cn";
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

/** One downloadable file: where it is for and its name on the left, size and a Download button on the right. */
function FileRow({
  where,
  asset,
  label,
  primary = false,
}: {
  where: string;
  asset: ReleaseAsset;
  label: string;
  /** The visitor's platform gets the one filled button in the release; the rest are outlined. */
  primary?: boolean;
}) {
  return (
    <li className="-mx-2 flex items-center gap-3 rounded-md px-2 py-2 transition-colors duration-150 hover:bg-muted/60">
      <p className="min-w-0 flex-1 text-sm break-all">
        <span className="text-muted-foreground">{where}</span>{" "}
        <span className="font-medium text-foreground">{asset.name}</span>
      </p>
      <div className="flex shrink-0 items-center gap-3">
        {asset.size > 0 && (
          <span className="text-xs tabular-nums text-muted-foreground">{formatBytes(asset.size)}</span>
        )}
        <Button
          variant={primary ? "default" : "outline"}
          className={cn("h-7 px-2.5 text-xs", primary && "hover:bg-primary-hover")}
          render={<a href={asset.url} rel="noopener" aria-label={label} />}
        >
          Download
        </Button>
      </div>
    </li>
  );
}

/** The latest release, or a previous one picked on mobile: a title row, then a thin list of files. */
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
  const windowsVisitor = platform === "windows";

  return (
    <div
      data-release-block
      className="rounded-xl border border-border/70 bg-card p-3 text-card-foreground sm:p-4"
    >
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
        <h3 className="font-heading text-base font-semibold tracking-tight">Clipsy {version}</h3>
        {isLatest && (
          <Badge variant="secondary" className="bg-brand-teal-soft text-brand-teal">
            Latest
          </Badge>
        )}
        {release.prerelease && <Badge variant="outline">Pre-release</Badge>}
        {date && (
          <span className="text-xs text-muted-foreground sm:ml-auto">
            <time dateTime={release.date}>{date}</time>
          </span>
        )}
      </div>

      <ul className="mt-3 divide-y divide-border/60 border-t border-border/60">
        {deb ? (
          <FileRow
            where="Linux"
            asset={deb}
            label={`Download .deb, version ${version}`}
            primary={!windowsVisitor}
          />
        ) : (
          <li className="py-2 text-sm">
            <span className="text-muted-foreground">Linux</span>{" "}
            <span className="text-muted-foreground">No .deb package for this version.</span>
          </li>
        )}
        {windowsInstaller && (
          <FileRow
            where="Windows"
            asset={windowsInstaller}
            label={`Installer (.exe), Windows setup for ${version}`}
            primary={windowsVisitor}
          />
        )}
        {windowsPortable && (
          <FileRow
            where="Windows"
            asset={windowsPortable}
            label={`Portable (.zip), Windows for ${version}`}
          />
        )}
        {!hasWindows && (
          <li className="flex flex-wrap items-center gap-x-2 gap-y-1 py-2 text-sm">
            <span className="text-muted-foreground">Windows</span>
            <Badge variant="secondary" className="bg-brand-yellow-soft text-brand-yellow">
              Coming soon
            </Badge>
            <span className="text-muted-foreground">The Windows download for this version is coming soon.</span>
          </li>
        )}
        {sums && (
          <FileRow
            where="Checksums"
            asset={sums}
            label={`SHA256SUMS, checksums for ${version}`}
          />
        )}
      </ul>

      {windowsVisitor
        ? windowsCommands.length > 0 && (
            <div className="mt-3">
              <p className="mb-1.5 text-xs text-muted-foreground">Check your download in PowerShell:</p>
              <div className="flex flex-col gap-1.5">
                {windowsCommands.map((command) => (
                  <CommandLine key={command} text={command} label="Copy PowerShell command" wrap />
                ))}
              </div>
            </div>
          )
        : sums && (
            <div className="mt-3">
              <p className="mb-1.5 text-xs text-muted-foreground">Check your download with:</p>
              <CommandLine text={CHECKSUM_VERIFY_COMMAND} label="Copy checksum command" />
            </div>
          )}

      <p className="mt-3 text-xs text-muted-foreground">
        Changes and details for this release are in the{" "}
        <a
          href={release.notesUrl}
          target="_blank"
          rel="noopener"
          className="font-medium text-secondary-accent underline underline-offset-4 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
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
      className="inline-flex min-h-8 items-center rounded-md px-2 font-medium text-foreground underline underline-offset-4 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
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

/** The section heading (rendered by ScrollPanel in App). */
export function DownloadsHeading() {
  return (
    <div>
      <h2 id="downloads-heading" className="font-heading text-lg font-medium tracking-tight">
        Downloads
      </h2>
      <p className="mt-1 max-w-[60ch] text-sm text-muted-foreground">
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
          <Skeleton aria-hidden="true" className="h-36 w-full" />
          <Skeleton aria-hidden="true" className="h-16 w-full" />
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
            className="mt-3 h-8 px-3 text-sm"
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

      {/* Mobile only: the version picker that switches the card above. */}
      <div className="mt-3 md:hidden">
        <DropdownMenu>
          <DropdownMenuTrigger render={<Button variant="outline" className="h-8 px-3 text-sm" />}>
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

      <div aria-live="polite" className="mt-3">
        {shown ? (
          <ReleaseBlock release={shown} isLatest={shown.tag === latest?.tag} platform={platform} />
        ) : (
          <p className="rounded-xl border border-border/70 bg-card/60 p-3.5 text-sm text-muted-foreground">
            No stable release yet.
          </p>
        )}
      </div>

      {previous.length > 0 && (
        <details className="group mt-3 overflow-hidden rounded-xl border border-border/70">
          <summary className="flex h-9 cursor-pointer list-none items-center justify-between gap-3 px-3 transition-colors duration-150 hover:bg-muted/60 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none [&::-webkit-details-marker]:hidden">
            <h3 className="font-heading text-sm font-medium tracking-tight">
              Previous versions{" "}
              <span className="tabular-nums text-muted-foreground">({previous.length})</span>
            </h3>
            <ChevronDown
              aria-hidden="true"
              className="size-4 shrink-0 text-muted-foreground transition-transform duration-200 group-open:rotate-180 motion-reduce:transition-none"
            />
          </summary>
          <div className="px-2 pb-2 sm:px-3">
          <Table aria-label="Previous versions">
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
                <TableRow key={release.tag} className="hover:bg-muted/60">
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
          </div>
        </details>
      )}
    </div>
  );
}
