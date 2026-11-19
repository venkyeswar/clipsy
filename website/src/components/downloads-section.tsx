import { useState } from "react";
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
import { CHECKSUM_VERIFY_COMMAND } from "@/lib/install-commands";
import { latestStable, type Release, type ReleaseAsset } from "@/lib/releases";

const RELEASES_URL = `https://github.com/${config.repo}/releases`;
const SECTION_CLASS = "flex h-full flex-col";
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

function ReleaseBlock({ release, isLatest }: { release: Release; isLatest: boolean }) {
  const { version, deb, checksums } = release;
  const date = formatDate(release.date);

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

      <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-2">
        {deb && (
          <AssetButton asset={deb} label={`Download .deb for ${version}`} text="Download .deb" />
        )}
        {checksums && (
          <AssetButton
            asset={checksums}
            variant="secondary"
            label={`Download SHA256SUMS for ${version}`}
            text="SHA256SUMS"
          />
        )}
      </div>

      {checksums && (
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

export function DownloadsSection() {
  const { releases, status } = useReleases();
  const [selectedTag, setSelectedTag] = useState<string | null>(null);

  const header = (
    <>
      <h2 className="font-heading text-xl font-semibold tracking-[-0.02em] sm:text-2xl">Downloads</h2>
      <p className="mt-1 max-w-[60ch] text-muted-foreground">
        The latest version is on top. Older versions stay available.
      </p>
    </>
  );

  if (status === "loading") {
    return (
      <section id="downloads" className={SECTION_CLASS} aria-busy="true">
        {header}
        <div className="mt-4 flex flex-col gap-4">
          <Skeleton aria-hidden="true" className="h-44 w-full" />
          <Skeleton aria-hidden="true" className="h-24 w-full" />
        </div>
      </section>
    );
  }

  if (status === "empty" || releases.length === 0) {
    return (
      <section id="downloads" className={SECTION_CLASS}>
        {header}
        <div className="mt-4">
          <p className="text-muted-foreground">No releases are published yet.</p>
          <Button
            variant="outline"
            className="mt-4 h-11 px-4"
            render={<a href={RELEASES_URL} target="_blank" rel="noopener" />}
          >
            View releases on GitHub
          </Button>
        </div>
      </section>
    );
  }

  const latest = latestStable(releases);
  const shown = releases.find((release) => release.tag === selectedTag) ?? latest;
  const previous = releases.filter((release) => release.tag !== latest?.tag);

  return (
    <section id="downloads" className={SECTION_CLASS}>
      {header}

      <div aria-live="polite" className="mt-2 min-h-0 text-sm text-muted-foreground empty:hidden">
        {status === "snapshot" &&
          "Showing the last saved release list because GitHub could not be reached."}
      </div>

      <div aria-live="polite" className="mt-4">
        {shown ? (
          <ReleaseBlock release={shown} isLatest={shown.tag === latest?.tag} />
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
                  <DropdownMenuItem key={release.tag} onClick={() => setSelectedTag(release.tag)}>
                    Clipsy {release.version}
                    {release.prerelease ? " (pre-release)" : ""}
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
                      label={`Download .deb for ${release.version}`}
                      text=".deb"
                    />
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
    </section>
  );
}
