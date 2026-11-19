import type { ReactNode } from "react";
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
  type InstallExtra,
  type InstallStep,
} from "@/lib/install-commands";

function Commands({ commands }: { commands: string[] }) {
  if (commands.length === 0) return null;
  return (
    <div className="mt-1.5 flex flex-col gap-1.5">
      {commands.map((command) => (
        <CommandLine key={command} text={command} wrap />
      ))}
    </div>
  );
}

function Step({
  step,
  number,
  waiting,
  children,
}: {
  step: InstallStep;
  number: number;
  waiting: boolean;
  children?: ReactNode;
}) {
  return (
    <li className="relative grid grid-cols-[1.75rem_minmax(0,1fr)] gap-x-3">
      <span
        aria-hidden="true"
        className="mt-px flex size-7 items-center justify-center rounded-full border border-secondary-accent/40 bg-brand-teal-soft font-mono text-xs font-semibold text-brand-teal"
      >
        {number}
      </span>
      <div className="min-w-0">
        <p className="font-medium text-foreground">{step.title}</p>
        <p className="text-sm text-muted-foreground">{step.detail}</p>
        {waiting && step.needsRelease ? (
          <Skeleton aria-hidden="true" className="mt-1.5 h-11 w-full" />
        ) : (
          <Commands commands={step.commands} />
        )}
        {children}
      </div>
    </li>
  );
}

function Extra({ extra }: { extra: InstallExtra }) {
  return (
    <div className="min-w-0">
      <p className="text-sm">
        <span className="font-medium text-foreground">{extra.label}:</span>{" "}
        <span className="text-muted-foreground">{extra.detail}</span>
      </p>
      <Commands commands={extra.commands} />
    </div>
  );
}

export function InstallSection() {
  const { releases, status } = useReleases();
  const release = latestStable(releases);
  const tabs = installCommands(config.snap.channel, release);

  return (
    <div className="flex flex-col">
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
          // Only the .deb commands come from release data; the rest never wait for it.
          const waiting = status === "loading";
          const showNotice = Boolean(tab.notice) && !waiting;

          return (
            <TabsContent
              key={tab.id}
              value={tab.id}
              className="pt-4 text-[0.9375rem] data-[hidden]:hidden motion-safe:animate-in motion-safe:fade-in-0 motion-safe:duration-200"
            >
              <p className="max-w-[62ch] text-muted-foreground">{tab.when}</p>

              {showNotice ? (
                <div className="mt-3">
                  <p className="max-w-[62ch] text-muted-foreground">{tab.notice}</p>
                  <Button
                    variant="outline"
                    className="mt-3 h-11 px-4"
                    render={<a href={releasesUrl()} target="_blank" rel="noopener" />}
                  >
                    View releases on GitHub
                  </Button>
                </div>
              ) : (
                <ol aria-label={`Steps for ${tab.title}`} className="mt-3 flex flex-col gap-3.5">
                  {tab.steps.map((step, index) => (
                    <Step key={step.title} step={step} number={index + 1} waiting={waiting}>
                      {tab.id === "snap" && index === 0 && (
                        <Button
                          variant="outline"
                          className="mt-1.5 h-11 px-4"
                          render={<a href={SNAP_APP_CENTER_URL} rel="noopener" />}
                        >
                          Open in App Center
                        </Button>
                      )}
                    </Step>
                  ))}
                </ol>
              )}

              {!showNotice && tab.extras.length > 0 && (
                <div className="mt-4 flex flex-col gap-2.5 border-t border-border pt-3">
                  {tab.extras.map((extra) => (
                    <Extra key={extra.label} extra={extra} />
                  ))}
                </div>
              )}
            </TabsContent>
          );
        })}
      </Tabs>

      <div className="mt-4 flex flex-wrap items-center gap-x-2 gap-y-1 rounded-lg border border-border bg-muted/50 px-3 py-2.5 text-sm text-muted-foreground">
        <span className="font-medium text-foreground">After installing:</span>
        <span>press</span>
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
