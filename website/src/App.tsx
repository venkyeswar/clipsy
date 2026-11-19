import { MotionConfig } from "motion/react";
import { DownloadsSection } from "@/components/downloads-section";
import { FeaturesSection } from "@/components/features-section";
import { HelpSection } from "@/components/help-section";
import { Hero } from "@/components/hero";
import { InstallSection } from "@/components/install-section";
import { Reveal } from "@/components/reveal";
import { ShortcutsSection } from "@/components/shortcuts-section";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";
import { Toaster } from "@/components/ui/sonner";

const container = "mx-auto w-full max-w-6xl px-4 sm:px-6";

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
          <div className={container}>
            <Hero />
          </div>

          {/* Get Clipsy: install options and the release files, side by side on wide screens. */}
          <div className="border-y border-border bg-card/60 dark:bg-card/40">
            <Reveal className={`${container} grid gap-8 py-8 sm:py-10 lg:grid-cols-2 lg:gap-10 lg:py-12`}>
              <section id="install" className="min-w-0">
                <InstallSection />
              </section>
              <div className="min-w-0 lg:border-l lg:border-border lg:pl-10">
                <DownloadsSection />
              </div>
            </Reveal>
          </div>

          <section id="features" className={`${container} py-8 sm:py-10 lg:py-12`}>
            <Reveal>
              <FeaturesSection />
            </Reveal>
          </section>

          <Reveal className={`${container} grid gap-8 pb-10 sm:pb-12 lg:grid-cols-[minmax(0,5fr)_minmax(0,7fr)] lg:gap-10 lg:pb-14`}>
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
