import { config } from "@/config";
import { PAGE_CONTAINER } from "@/lib/layout";

const repoUrl = `https://github.com/${config.repo}`;
const issuesUrl = `${repoUrl}/issues`;

export function SiteFooter() {
  return (
    <footer className="border-t">
      <div className={`${PAGE_CONTAINER} flex flex-col gap-1 py-4 text-sm text-muted-foreground sm:flex-row sm:items-center sm:justify-between`}>
        <p>Clipsy is free software released under the GPL-3.0 license.</p>
        <nav aria-label="Footer" className="flex flex-wrap items-center gap-x-5 gap-y-2">
          <a
            href={repoUrl}
            target="_blank"
            rel="noopener"
            className="inline-flex min-h-11 items-center text-foreground underline-offset-4 hover:underline"
          >
            GitHub
          </a>
          <span>GPL-3.0 license</span>
          <a
            href={issuesUrl}
            target="_blank"
            rel="noopener"
            className="inline-flex min-h-11 items-center text-foreground underline-offset-4 hover:underline"
          >
            Report an issue
          </a>
        </nav>
      </div>
    </footer>
  );
}
