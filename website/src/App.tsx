import { MotionConfig } from "motion/react";
import { DownloadsHeading, DownloadsSection } from "@/components/downloads-section";
import { FeaturesSection } from "@/components/features-section";
import { HelpSection } from "@/components/help-section";
import { Hero } from "@/components/hero";
import { InstallHeading, InstallSection } from "@/components/install-section";
import { Reveal } from "@/components/reveal";
import { ScrollPanel } from "@/components/scroll-panel";
import { ShortcutsSection } from "@/components/shortcuts-section";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";
import { Toaster } from "@/components/ui/sonner";
import { PAGE_CONTAINER, SECTION_PADDING } from "@/lib/layout";

export default function App() {
  return (
    <MotionConfig reducedMotion="user">
      <div id="top" className="flex min-h-dvh flex-col">
        <a
          href="#main"
          className="sr-only focus:not-sr-only focus:absolute focus:left-4 focus:top-4 focus:z-50 focus:rounded-md focus:bg-primary focus:px-4 focus:py-2 focus:text-primary-foreground"
        >
          Skip to content
        </a>
        <SiteHeader />
        <main id="main" className="flex-1 overflow-x-clip">
          <div className={PAGE_CONTAINER}>
            <Hero />
          </div>

          {/*
           * Get Clipsy: install options and release files. On large screens they are two
           * equal-height panels that scroll inside their own boxes; below lg they stack.
           */}
          <div className="border-y border-border bg-[var(--panel-surface)]">
            <Reveal className={`${PAGE_CONTAINER} grid gap-8 py-8 sm:py-10 lg:grid-cols-2 lg:gap-10 lg:py-12`}>
              <ScrollPanel
                id="install"
                headingId="install-heading"
                heading={<InstallHeading />}
                label="Install options, scrollable"
              >
                <InstallSection />
              </ScrollPanel>
              <div className="min-w-0 lg:border-l lg:border-border lg:pl-10">
                <ScrollPanel
                  id="downloads"
                  headingId="downloads-heading"
                  heading={<DownloadsHeading />}
                  label="Downloads and release files, scrollable"
                >
                  <DownloadsSection />
                </ScrollPanel>
              </div>
            </Reveal>
          </div>

          <section id="features" className={`${PAGE_CONTAINER} ${SECTION_PADDING}`}>
            <Reveal>
              <FeaturesSection />
            </Reveal>
          </section>

          <Reveal
            className={`${PAGE_CONTAINER} grid gap-8 pb-[clamp(2rem,4vw,4.5rem)] lg:grid-cols-[minmax(0,5fr)_minmax(0,7fr)] lg:gap-10`}
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
