import { Button } from "@/components/ui/button";
import { CopyButton } from "@/components/copy-button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { config } from "@/config";
import { useReleases } from "@/hooks/use-releases";
import { latestStable } from "@/lib/releases";
import {
  installCommands,
  releasesUrl,
  SNAP_APP_CENTER_URL,
} from "@/lib/install-commands";

export function InstallSection() {
  const { releases } = useReleases();
  const release = latestStable(releases);
  const tabs = installCommands(config.snap.channel, release);

  return (
    <div>
      <h2 className="font-heading text-2xl font-semibold tracking-tight">Install Clipsy</h2>
      <p className="mt-2 max-w-[60ch] text-muted-foreground">
        Pick the way that suits you. Every option installs the same app.
      </p>

      <Tabs defaultValue={tabs[0].id} className="mt-6 gap-0">
        <TabsList className="!h-auto max-w-full flex-wrap justify-start gap-1 p-1">
          {tabs.map((tab) => (
            <TabsTrigger key={tab.id} value={tab.id} className="min-h-11 px-3">
              {tab.title}
            </TabsTrigger>
          ))}
        </TabsList>

        {tabs.map((tab) => (
          <TabsContent key={tab.id} value={tab.id} className="mt-4 flex flex-col gap-4">
            <p className="max-w-[60ch] text-muted-foreground">{tab.note}</p>

            {tab.commands.map((command) => (
              <div
                key={`${tab.id}-${command}`}
                className="flex items-center gap-2 rounded-lg border border-border bg-muted p-2 pl-3"
              >
                <pre
                  data-copy-source
                  tabIndex={0}
                  className="min-w-0 flex-1 overflow-x-auto rounded-sm font-mono text-sm whitespace-pre text-foreground outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
                >
                  <code>{command}</code>
                </pre>
                <CopyButton text={command} />
              </div>
            ))}

            {tab.id === "snap" && (
              <div>
                <Button
                  variant="outline"
                  className="h-11 px-4"
                  render={<a href={SNAP_APP_CENTER_URL} rel="noopener" />}
                >
                  Open in App Center
                </Button>
              </div>
            )}

            {tab.id === "deb" && !release?.deb && (
              <div>
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
        ))}
      </Tabs>
    </div>
  );
}
