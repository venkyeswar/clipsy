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
import { CopyButton } from "@/components/copy-button";
import { config } from "@/config";
import { useReleases } from "@/hooks/use-releases";
import { formatBytes } from "@/lib/format";
import { latestStable, type Release, type ReleaseAsset } from "@/lib/releases";

const CHECKSUM_COMMAND = "sha256sum -c SHA256SUMS";
const RELEASES_URL = `https://github.com/${config.repo}/releases`;
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
    <div className="flex items-center gap-3">
      <Button
        variant={variant}
        className="h-11 px-4"
        render={<a href={asset.url} rel="noopener" aria-label={label} />}
      >
        {text}
      </Button>
      {asset.size > 0 && (
        <span className="text-sm text-muted-foreground">{formatBytes(asset.size)}</span>
      )}
    </div>
  );
}

function ReleaseBlock({ release, isLatest }: { release: Release; isLatest: boolean }) {
  const { version, deb, tarball, checksums } = release;
  const date = formatDate(release.date);

  return (
    <div
      data-release-block
      className="rounded-xl border border-border bg-card p-5 text-card-foreground sm:p-6"
    >
      <div className="flex flex-wrap items-center gap-2">
        <h3 className="font-heading text-xl font-semibold tracking-tight">Clipsy {version}</h3>
        {isLatest && <Badge variant="secondary">Latest</Badge>}
        {release.prerelease && <Badge variant="outline">Pre-release</Badge>}
      </div>
      {date && (
        <p className="mt-1 text-sm text-muted-foreground">
          <time dateTime={release.date}>{date}</time>
        </p>
      )}

      <div className="mt-5 flex flex-wrap items-center gap-3">
        {deb && (
          <AssetButton asset={deb} label={`Download .deb for ${version}`} text="Download .deb" />
        )}
        {tarball && (
          <AssetButton
            asset={tarball}
            variant="outline"
            label={`Download no-sudo tarball for ${version}`}
            text="Download no-sudo tarball"
          />
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
        <div className="mt-5 flex flex-wrap items-center gap-1 text-sm text-muted-foreground">
          <span>Check your download with:</span>
          <span className="flex items-center gap-1 rounded-md bg-muted py-1 pr-1 pl-2">
            <code data-copy-source className="font-mono text-foreground">
              {CHECKSUM_COMMAND}
            </code>
            <CopyButton text={CHECKSUM_COMMAND} label="Copy checksum command" />
          </span>
        </div>
      )}

      <p className="mt-4 text-sm text-muted-foreground">
        Changes and details for this release are in the{" "}
        <a
          href={release.notesUrl}
          target="_blank"
          rel="noopener"
          className="font-medium text-foreground underline underline-offset-4 focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none"
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
      <h2 className="font-heading text-2xl font-semibold tracking-tight">Downloads</h2>
      <p className="mt-2 max-w-[60ch] text-muted-foreground">
        The latest version is on top. Older versions stay available.
      </p>
    </>
  );

  if (status === "loading") {
    return (
      <section id="downloads" className="py-12" aria-busy="true">
        {header}
        <div className="mt-6 flex flex-col gap-10">
          <Skeleton aria-hidden="true" className="h-56 w-full" />
          <Skeleton aria-hidden="true" className="h-64 w-full" />
        </div>
      </section>
    );
  }

  if (status === "empty" || releases.length === 0) {
    return (
      <section id="downloads" className="py-12">
        {header}
        <div className="mt-6">
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
    <section id="downloads" className="py-12">
      {header}

      <div aria-live="polite" className="mt-4 min-h-0 text-sm text-muted-foreground">
        {status === "snapshot" &&
          "Showing the last saved release list because GitHub could not be reached."}
      </div>

      <div aria-live="polite" className="mt-4">
        {shown ? (
          <ReleaseBlock release={shown} isLatest={shown.tag === latest?.tag} />
        ) : (
          <p className="rounded-xl border border-border bg-card p-5 text-muted-foreground sm:p-6">
            No stable release yet.
          </p>
        )}
      </div>

      <div className="mt-10">
        <div className="mb-3 flex flex-wrap items-center justify-between gap-3 md:mb-4">
          <h3 id="previous-versions-heading" className="font-heading text-lg font-semibold tracking-tight">
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
                <TableHead scope="col">Tarball</TableHead>
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
                    <AssetLink
                      asset={release.tarball}
                      label={`Download no-sudo tarball for ${release.version}`}
                      text="Tarball"
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
          <p className="text-muted-foreground">No other releases yet.</p>
        )}
      </div>
    </section>
  );
}
