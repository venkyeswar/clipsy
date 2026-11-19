import { DownloadsSection } from "@/components/downloads-section";
import { FeaturesSection } from "@/components/features-section";
import { HelpSection } from "@/components/help-section";
import { Hero } from "@/components/hero";
import { InstallSection } from "@/components/install-section";
import { ShortcutsSection } from "@/components/shortcuts-section";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";
import { Toaster } from "@/components/ui/sonner";

export default function App() {
  return (
    <div id="top" className="flex min-h-dvh flex-col">
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:absolute focus:left-4 focus:top-4 focus:z-50 focus:rounded-md focus:bg-primary focus:px-4 focus:py-2 focus:text-primary-foreground"
      >
        Skip to content
      </a>
      <SiteHeader />
      <main id="main" className="mx-auto w-full max-w-5xl flex-1 px-4">
        <Hero />
        <section id="install" className="py-12">
          <InstallSection />
        </section>
        <DownloadsSection />
        <section id="features" className="py-12">
          <FeaturesSection />
        </section>
        <section id="shortcuts" className="py-12">
          <ShortcutsSection />
        </section>
        <section id="help" className="py-12">
          <HelpSection />
        </section>
      </main>
      <SiteFooter />
      <Toaster />
    </div>
  );
}
