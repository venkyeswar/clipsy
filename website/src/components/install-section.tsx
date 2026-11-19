import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Keycap } from "@/components/keycap";
import { CommandLine } from "@/components/command-line";
import { Tabs, TabsContent, TabsIndicator, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { config } from "@/config";
import { useReleases } from "@/hooks/use-releases";
import { latestStable } from "@/lib/releases";
import {
  installCommands,
  releasesUrl,
  SNAP_APP_CENTER_URL,
} from "@/lib/install-commands";

export function InstallSection() {
  const { releases, status } = useReleases();
  const release = latestStable(releases);
  const tabs = installCommands(config.snap.channel, release);

  return (
    <div className="flex h-full flex-col">
      <h2 className="font-heading text-xl font-semibold tracking-[-0.02em] sm:text-2xl">Install Clipsy</h2>
      <p className="mt-1 max-w-[60ch] text-muted-foreground">
        Pick the way that suits you. Every option installs the same app.
      </p>

      <Tabs defaultValue={tabs[0].id} className="mt-4 gap-0">
        <TabsList
          variant="line"
          className="relative h-auto! w-full max-w-full justify-start gap-0 overflow-x-auto rounded-none border-b border-border p-0 [scrollbar-width:none]"
        >
          <TabsIndicator className="top-auto! bottom-[-1px] h-0.5! rounded-full bg-primary" />
          {tabs.map((tab) => (
            <TabsTrigger
              key={tab.id}
              value={tab.id}
              className="min-h-11 flex-none px-3 text-[0.875rem] after:hidden data-active:text-foreground"
            >
              {tab.title}
            </TabsTrigger>
          ))}
        </TabsList>

        {tabs.map((tab) => {
          // Deb and no-sudo depend on release data: show a placeholder while it loads.
          const waiting = status === "loading" && (tab.id === "deb" || tab.id === "nosudo");
          const noDeb = tab.id === "deb" && !release?.deb;
          const noTarball = tab.id === "nosudo" && !release?.tarball;

          return (
            <TabsContent
              key={tab.id}
              value={tab.id}
              className="flex flex-col gap-2 pt-4 text-[0.9375rem] data-[hidden]:hidden motion-safe:animate-in motion-safe:fade-in-0 motion-safe:duration-200"
            >
              {waiting ? (
                <Skeleton aria-hidden="true" className="h-11 w-full" />
              ) : (
                <p className="mb-1 max-w-[62ch] text-muted-foreground">{tab.note}</p>
              )}

              {!waiting &&
                tab.commands.map((command) => (
                  <CommandLine key={`${tab.id}-${command}`} text={command} />
                ))}

              {tab.id === "snap" && (
                <div className="mt-1">
                  <Button
                    variant="outline"
                    className="h-11 px-4"
                    render={<a href={SNAP_APP_CENTER_URL} rel="noopener" />}
                  >
                    Open in App Center
                  </Button>
                </div>
              )}

              {!waiting && (noDeb || noTarball) && (
                <div className="mt-1">
                  <Button
                    variant="outline"
                    className="h-11 px-4"
                    render={<a href={releasesUrl()} target="_blank" rel="noopener" />}
                  >
                    View releases on GitHub
                  </Button>
                </div>
              )}
            </TabsContent>
          );
        })}
      </Tabs>

      <div className="mt-auto flex flex-wrap items-center gap-x-2 gap-y-1 pt-5 text-sm text-muted-foreground">
        <span>After installing, press</span>
        <span className="inline-flex items-center gap-1">
          {config.shortcut.map((key, index) => (
            <span key={key} className="inline-flex items-center gap-1">
              {index > 0 && <span aria-hidden="true">+</span>}
              <Keycap>{key}</Keycap>
            </span>
          ))}
        </span>
        <span>to open Clipsy.</span>
      </div>
    </div>
  );
}
