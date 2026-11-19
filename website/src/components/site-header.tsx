import { useEffect, useState } from "react";
import { Download, Menu, Monitor, Moon, Sun } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Sheet,
  SheetClose,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";
import { config } from "@/config";
import { useTheme, type Theme } from "@/components/theme-provider";
import { useReleases } from "@/hooks/use-releases";
import { useSelectedPlatform } from "@/lib/platform-context";
import { latestStable, latestWithWindows } from "@/lib/releases";

const sectionLinks = [
  { href: "#install", label: "Install" },
  { href: "#downloads", label: "Downloads" },
  { href: "#features", label: "Features" },
  { href: "#help", label: "Help" },
];

/** Every page section, in page order. The observer tracks all of them. */
const trackedSectionIds = ["install", "downloads", "features", "shortcuts", "help"];

const githubUrl = `https://github.com/${config.repo}`;

const themeIcon = {
  light: Sun,
  dark: Moon,
  system: Monitor,
} satisfies Record<Theme, typeof Sun>;

function ThemeToggle() {
  const { theme, setTheme } = useTheme();
  const Icon = themeIcon[theme];

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            variant="ghost"
            size="icon"
            className="size-11"
            aria-label={`Theme: ${theme}. Change theme`}
          />
        }
      >
        <AnimatePresence mode="popLayout" initial={false}>
          <motion.span
            key={theme}
            initial={{ rotate: -90, scale: 0.5, opacity: 0 }}
            animate={{ rotate: 0, scale: 1, opacity: 1 }}
            exit={{ rotate: 90, scale: 0.5, opacity: 0 }}
            transition={{ type: "spring", stiffness: 400, damping: 26 }}
            className="inline-flex"
          >
            <Icon aria-hidden="true" className="size-4" />
          </motion.span>
        </AnimatePresence>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuRadioGroup
          value={theme}
          onValueChange={(value) => setTheme(value as Theme)}
        >
          <DropdownMenuRadioItem value="light">Light</DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="dark">Dark</DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="system">System</DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

/**
 * Returns the id of the section currently in view, or "" before any section is visible.
 * Falls back to "" when IntersectionObserver is missing (e.g. old browsers, jsdom).
 */
function useActiveSection(ids: string[]): string {
  const [active, setActive] = useState("");

  useEffect(() => {
    if (typeof IntersectionObserver === "undefined") return;

    const visible = new Set<string>();
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) visible.add(entry.target.id);
          else visible.delete(entry.target.id);
        }
        // Pick the first visible section in page order, so the highlight is stable.
        const first = ids.find((id) => visible.has(id));
        if (first) setActive(first);
      },
      { rootMargin: "-40% 0px -55% 0px" },
    );

    for (const id of ids) {
      const element = document.getElementById(id);
      if (element) observer.observe(element);
    }
    return () => observer.disconnect();
  }, [ids]);

  return active;
}

/**
 * Where the header Download button goes for the visitor's platform: the Windows installer
 * when there is one, otherwise the Install section; the .deb on Linux, otherwise Downloads.
 */
function useHeaderDownload(): { href: string; label: string } {
  const { releases } = useReleases();
  const { platform } = useSelectedPlatform();
  if (platform === "windows") {
    const url = latestWithWindows(releases)?.windowsInstaller?.url;
    return url ? { href: url, label: "Download for Windows" } : { href: "#install", label: "Download" };
  }
  const deb = latestStable(releases)?.deb?.url;
  return deb ? { href: deb, label: "Download" } : { href: "#downloads", label: "Download" };
}

export function SiteHeader() {
  const active = useActiveSection(trackedSectionIds);
  const download = useHeaderDownload();

  return (
    <header className="sticky top-0 z-40 border-b border-border/70 bg-background/85 backdrop-blur-md supports-[backdrop-filter]:bg-background/70">
      <div className="mx-auto flex h-14 w-full max-w-6xl items-center justify-between gap-2 px-4 sm:px-6">
        <a
          href="#top"
          className="-ml-1 flex min-h-11 items-center gap-2 rounded-lg px-1 font-heading text-lg font-semibold tracking-[-0.02em]"
        >
          <img
            src={`${import.meta.env.BASE_URL}logo.svg`}
            alt=""
            width={28}
            height={28}
            className="size-7"
          />
          Clipsy
        </a>

        <nav aria-label="Main" className="ml-4 hidden flex-1 items-center gap-0.5 md:flex">
          {sectionLinks.map((link) => (
            <Button
              key={link.href}
              variant="ghost"
              className="h-11 px-3 text-muted-foreground hover:text-foreground aria-[current=location]:bg-muted aria-[current=location]:text-foreground"
              render={<a href={link.href} />}
              aria-current={active === link.href.slice(1) ? "location" : undefined}
            >
              {link.label}
            </Button>
          ))}
          <Button
            variant="ghost"
            className="h-11 px-3 text-muted-foreground hover:text-foreground"
            render={
              <a href={githubUrl} target="_blank" rel="noopener" />
            }
          >
            GitHub
          </Button>
        </nav>

        <div className="flex items-center gap-1">
          <ThemeToggle />
          <Button
            className="ml-1 hidden h-11 px-3.5 hover:bg-primary-hover md:inline-flex"
            render={<a href={download.href} />}
          >
            <Download aria-hidden="true" />
            {download.label}
          </Button>

          <Sheet>
            <SheetTrigger
              render={
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-11 md:hidden"
                  aria-label="Open menu"
                />
              }
            >
              <Menu aria-hidden="true" />
            </SheetTrigger>
            <SheetContent side="right" className="w-72">
              <SheetHeader>
                <SheetTitle>Menu</SheetTitle>
              </SheetHeader>
              <nav aria-label="Mobile" className="flex flex-col gap-1 px-4 pb-4">
                {sectionLinks.map((link) => (
                  <SheetClose
                    key={link.href}
                    render={
                      <Button
                        variant="ghost"
                        className="h-11 justify-start text-base"
                        render={<a href={link.href} />}
                        aria-current={active === link.href.slice(1) ? "location" : undefined}
                      />
                    }
                  >
                    {link.label}
                  </SheetClose>
                ))}
                <SheetClose
                  render={
                    <Button
                      variant="ghost"
                      className="h-11 justify-start text-base"
                      render={<a href={githubUrl} target="_blank" rel="noopener" />}
                    />
                  }
                >
                  GitHub
                </SheetClose>
              </nav>
            </SheetContent>
          </Sheet>
        </div>
      </div>
    </header>
  );
}
