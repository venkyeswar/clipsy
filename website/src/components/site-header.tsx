import { Menu, Monitor, Moon, Sun } from "lucide-react";
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

const sectionLinks = [
  { href: "#install", label: "Install" },
  { href: "#downloads", label: "Downloads" },
  { href: "#help", label: "Help" },
];

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
        <Icon aria-hidden="true" />
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

export function SiteHeader() {
  return (
    <header className="sticky top-0 z-40 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/80">
      <div className="mx-auto flex h-16 w-full max-w-5xl items-center justify-between gap-2 px-4">
        <a
          href="#top"
          className="flex items-center gap-2 font-heading text-lg font-semibold tracking-tight"
        >
          <img
            src={`${import.meta.env.BASE_URL}logo.svg`}
            alt=""
            width={32}
            height={32}
            className="size-8"
          />
          Clipsy
        </a>

        <nav aria-label="Main" className="hidden items-center gap-1 md:flex">
          {sectionLinks.map((link) => (
            <Button key={link.href} variant="ghost" render={<a href={link.href} />}>
              {link.label}
            </Button>
          ))}
          <Button
            variant="ghost"
            render={
              <a href={githubUrl} target="_blank" rel="noopener" />
            }
          >
            GitHub
          </Button>
        </nav>

        <div className="flex items-center gap-1">
          <ThemeToggle />

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
