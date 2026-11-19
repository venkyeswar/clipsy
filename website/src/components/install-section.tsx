import { useState, type ReactNode } from "react";
import { cn } from "cn";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Keycap } from "@/components/keycap";
import { CommandLine } from "@/components/command-line";
import { Tabs, TabsContent, TabsIndicator, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { config, shortcutFor } from "@/config";
import { useReleases } from "@/hooks/use-releases";
import { useSelectedPlatform, type SelectedPlatform } from "@/lib/platform-context";
import { latestStable, latestWithWindows, windowsStatus } from "@/lib/releases";
import {
  installCommands,
  releasesUrl,
  SMARTSCREEN_NOTE,
  SNAP_APP_CENTER_URL,
  windowsInstallTabs,
  type InstallExtra,
  type InstallStep,
  type WindowsInstallTab,
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
    <li className="relative grid grid-cols-[1.5rem_minmax(0,1fr)] gap-x-3">
      <span
        aria-hidden="true"
        className="mt-px flex size-6 items-center justify-center rounded-full border border-secondary-accent/40 bg-brand-teal-soft font-mono text-xs font-semibold text-brand-teal"
      >
        {number}
      </span>
      <div className="min-w-0">
        <p className="text-sm font-medium text-foreground">{step.title}</p>
        <p className="text-sm text-muted-foreground">{step.detail}</p>
        {waiting && step.needsRelease ? (
          <Skeleton aria-hidden="true" className="mt-1.5 h-9 w-full" />
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

/** Linux | Windows. Shared with the rest of the page through the platform context. */
function PlatformSwitch({ value, onChange }: { value: SelectedPlatform; onChange: (platform: SelectedPlatform) => void }) {
  const options: { value: SelectedPlatform; label: string }[] = [
    { value: "linux", label: "Linux" },
    { value: "windows", label: "Windows" },
  ];
  return (
    <div role="group" aria-label="Operating system" className="inline-flex h-8 gap-0.5 rounded-lg border border-border p-0.5">
      {options.map((option) => {
        const selected = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            aria-pressed={selected}
            onClick={() => onChange(option.value)}
            className={cn(
              "min-w-20 rounded-md px-3 text-sm font-medium transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
              selected ? "bg-muted text-foreground" : "text-muted-foreground hover:text-foreground",
            )}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

/** A Windows tab: the steps, plus the unsigned-app note on the tabs that run the app. */
function WindowsTabPanel({ tab, waiting }: { tab: WindowsInstallTab; waiting: boolean }) {
  const runsApp = tab.id === "windows-installer" || tab.id === "windows-portable";
  return (
    <>
      <p className="max-w-[62ch] text-muted-foreground">{tab.when}</p>

      {runsApp && (
        <p
          role="note"
          className="mt-3 max-w-[62ch] rounded-lg border border-brand-yellow/40 bg-brand-yellow-soft px-3 py-2 text-sm text-foreground"
        >
          <span className="font-medium">SmartScreen:</span> {SMARTSCREEN_NOTE}
        </p>
      )}

      {tab.notice && (
        <p className="mt-3 max-w-[62ch] font-medium text-foreground">{tab.notice}</p>
      )}

      <ol aria-label={`Steps for ${tab.title}`} className="mt-3 flex flex-col gap-3">
        {tab.steps.map((step, index) => (
          <Step key={step.title} step={step} number={index + 1} waiting={waiting} />
        ))}
      </ol>
    </>
  );
}

/** The section heading (rendered by ScrollPanel in App). */
export function InstallHeading() {
  return (
    <div>
      <h2 id="install-heading" className="font-heading text-lg font-medium tracking-tight">
        Install Clipsy
      </h2>
      <p className="mt-1 max-w-[60ch] text-sm text-muted-foreground">
        Pick the way that suits you. Every option installs the same app.
      </p>
    </div>
  );
}

export function InstallSection() {
  const { releases, status } = useReleases();
  const { platform, setPlatform } = useSelectedPlatform();
  // Only the .deb and Windows commands come from release data; the rest never wait for it.
  const waiting = status === "loading";
  const release = latestStable(releases);
  const windowsRelease = latestWithWindows(releases);
  const windowsComingSoon = !waiting && windowsStatus(releases) === "coming-soon";

  const linuxTabs = installCommands(config.snap.channel, release);
  const windowsTabs = windowsInstallTabs(windowsRelease, windowsComingSoon);
  const tabList = platform === "windows" ? windowsTabs : linuxTabs;

  // The tab the visitor picked on this platform, or the platform's first tab. Windows opens on
  // Build from source when there is no Windows download yet.
  const [choice, setChoice] = useState<{ platform: SelectedPlatform; id: string } | null>(null);
  const defaultId =
    platform === "windows" ? (windowsComingSoon ? "windows-source" : "windows-installer") : linuxTabs[0].id;
  const activeId =
    choice?.platform === platform && tabList.some((tab) => tab.id === choice.id) ? choice.id : defaultId;

  return (
    <div className="flex flex-col">
      <div>
        <PlatformSwitch value={platform} onChange={setPlatform} />
      </div>

      <Tabs
        key={platform}
        value={activeId}
        onValueChange={(value) => setChoice({ platform, id: String(value) })}
        className="mt-3 gap-0"
      >
        <TabsList
          variant="line"
          aria-label={`Install options for ${platform === "windows" ? "Windows" : "Linux"}`}
          className="relative h-auto! w-full max-w-full justify-start gap-0 overflow-x-auto rounded-none border-b border-border p-0 [scrollbar-width:none]"
        >
          <TabsIndicator className="top-auto! bottom-[-1px] h-[1.5px]! rounded-full bg-primary" />
          {tabList.map((tab) => (
            <TabsTrigger
              key={tab.id}
              value={tab.id}
              className="h-9 flex-none px-3 text-sm text-muted-foreground transition-colors duration-150 after:hidden hover:border-b-primary/40 hover:text-foreground data-active:text-foreground"
            >
              {tab.title}
            </TabsTrigger>
          ))}
        </TabsList>

        {platform === "windows"
          ? windowsTabs.map((tab) => (
              <TabsContent
                key={tab.id}
                value={tab.id}
                className="pt-3 text-sm data-[hidden]:hidden motion-safe:animate-in motion-safe:fade-in-0 motion-safe:duration-200"
              >
                <WindowsTabPanel tab={tab} waiting={waiting} />
              </TabsContent>
            ))
          : linuxTabs.map((tab) => {
              const showNotice = Boolean(tab.notice) && !waiting;

              return (
                <TabsContent
                  key={tab.id}
                  value={tab.id}
                  className="pt-3 text-sm data-[hidden]:hidden motion-safe:animate-in motion-safe:fade-in-0 motion-safe:duration-200"
                >
                  <p className="max-w-[62ch] text-muted-foreground">{tab.when}</p>

                  {showNotice ? (
                    <div className="mt-3">
                      <p className="max-w-[62ch] text-muted-foreground">{tab.notice}</p>
                      <Button
                        variant="outline"
                        className="mt-2 h-8 px-3 text-sm"
                        render={<a href={releasesUrl()} target="_blank" rel="noopener" />}
                      >
                        View releases on GitHub
                      </Button>
                    </div>
                  ) : (
                    <ol aria-label={`Steps for ${tab.title}`} className="mt-3 flex flex-col gap-3">
                      {tab.steps.map((step, index) => (
                        <Step key={step.title} step={step} number={index + 1} waiting={waiting}>
                          {tab.id === "snap" && index === 0 && (
                            <Button
                              variant="outline"
                              className="mt-1.5 h-8 px-3 text-sm"
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
                    <div className="mt-3 flex flex-col gap-2 border-t border-border pt-3">
                      {tab.extras.map((extra) => (
                        <Extra key={extra.label} extra={extra} />
                      ))}
                    </div>
                  )}
                </TabsContent>
              );
            })}
      </Tabs>

      <div className="mt-3 flex flex-wrap items-center gap-x-2 gap-y-1 rounded-lg border border-border bg-muted/50 px-3 py-2 text-sm text-muted-foreground">
        <span className="font-medium text-foreground">After installing:</span>
        <span>press</span>
        <span className="inline-flex items-center gap-1">
          {shortcutFor(platform).map((key, index) => (
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
