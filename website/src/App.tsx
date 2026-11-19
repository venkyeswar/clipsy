import { Hero } from "@/components/hero";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";

const sections = [
  { id: "install", title: "Install" },
  { id: "downloads", title: "Downloads" },
  { id: "features", title: "Features" },
  { id: "shortcuts", title: "Shortcuts" },
  { id: "help", title: "Help" },
];

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
        {sections.map((section) => (
          <section key={section.id} id={section.id} className="py-12">
            <h2 className="font-heading text-2xl font-semibold tracking-tight">
              {section.title}
            </h2>
          </section>
        ))}
      </main>
      <SiteFooter />
    </div>
  );
}
