import { config } from "@/config";

const repoUrl = `https://github.com/${config.repo}`;
const issuesUrl = `${repoUrl}/issues`;

export function SiteFooter() {
  return (
    <footer className="border-t">
      <div className="mx-auto flex w-full max-w-5xl flex-col gap-3 px-4 py-8 text-sm text-muted-foreground sm:flex-row sm:items-center sm:justify-between">
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
