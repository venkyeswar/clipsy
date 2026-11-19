import { MotionConfig } from "motion/react";
import { DownloadsHeading, DownloadsSection } from "@/components/downloads-section";
import { FeaturesSection } from "@/components/features-section";
import { HelpSection } from "@/components/help-section";
import { Hero } from "@/components/hero";
import { InstallHeading, InstallSection } from "@/components/install-section";
import { PointerLayer } from "@/components/pointer-layer";
import { Reveal } from "@/components/reveal";
import { ScrollPanel } from "@/components/scroll-panel";
import { ShortcutsSection } from "@/components/shortcuts-section";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";
import { Spotlight } from "@/components/spotlight";
import { Toaster } from "@/components/ui/sonner";
import { PAGE_CONTAINER, SECTION_PADDING } from "@/lib/layout";

export default function App() {
  return (
    <MotionConfig reducedMotion="user">
      <div id="top" className="flex min-h-dvh flex-col">
        <PointerLayer />
        <a
          href="#main"
          className="sr-only focus:not-sr-only focus:absolute focus:left-4 focus:top-4 focus:z-50 focus:rounded-md focus:bg-primary focus:px-4 focus:py-2 focus:text-primary-foreground"
        >
          Skip to content
        </a>
        <SiteHeader />
        <main id="main" className="flex-1 overflow-x-clip">
          <div className="border-b border-border">
            <div className={PAGE_CONTAINER}>
              <Hero />
            </div>
          </div>

          {/*
           * Get Clipsy: install options and release files. Two cards side by side on large
           * screens, stacked below lg. On lg and up each body is height-limited and scrolls
           * inside its own box only when it is taller than that; below lg the page scrolls.
           */}
          <div className="border-y border-border">
            <Reveal className={`${PAGE_CONTAINER} grid gap-4 py-6 lg:grid-cols-2 lg:gap-6 lg:py-8`}>
              <Spotlight className="rounded-xl border border-border/70 bg-background/80 p-4 backdrop-blur-sm sm:p-5">
                <ScrollPanel
                  id="install"
                  headingId="install-heading"
                  heading={<InstallHeading />}
                  label="Install options"
                  bounded
                >
                  <InstallSection />
                </ScrollPanel>
              </Spotlight>
              <Spotlight className="rounded-xl border border-border/70 bg-background/80 p-4 backdrop-blur-sm sm:p-5">
                <ScrollPanel
                  id="downloads"
                  headingId="downloads-heading"
                  heading={<DownloadsHeading />}
                  label="Downloads and release files"
                  bounded
                >
                  <DownloadsSection />
                </ScrollPanel>
              </Spotlight>
            </Reveal>
          </div>

          <section id="features" className={`${PAGE_CONTAINER} ${SECTION_PADDING}`}>
            <Reveal>
              <FeaturesSection />
            </Reveal>
          </section>

          <Reveal
            className={`${PAGE_CONTAINER} grid gap-4 pb-[clamp(1.25rem,2.2vw,2.25rem)] lg:grid-cols-[minmax(0,5fr)_minmax(0,7fr)] lg:gap-6`}
          >
            <section id="shortcuts" className="min-w-0">
              <ShortcutsSection />
            </section>
            <section id="help" className="min-w-0">
              <HelpSection />
            </section>
          </Reveal>
        </main>
        <SiteFooter />
        <Toaster />
      </div>
    </MotionConfig>
  );
}
