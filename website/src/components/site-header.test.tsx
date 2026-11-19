import { act, cleanup, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { SiteHeader } from "./site-header";
import { ThemeProvider } from "./theme-provider";
import { useReleases } from "@/hooks/use-releases";
import type { Release } from "@/lib/releases";
import { renderWithPlatform } from "@/test/render-with-platform";

vi.mock("@/hooks/use-releases", () => ({ useReleases: vi.fn() }));
const mockedUseReleases = vi.mocked(useReleases);

const release: Release = {
  version: "0.3.1",
  tag: "v0.3.1",
  date: "2026-11-18T12:00:00Z",
  prerelease: false,
  notesUrl: "https://example.test/notes",
  deb: { name: "clipsy_0.3.1_amd64.deb", url: "https://example.test/clipsy.deb", size: 1 },
  windowsInstaller: { name: "clipsy-0.3.1-setup.exe", url: "https://example.test/setup.exe", size: 1 },
};

type Callback = (entries: Partial<IntersectionObserverEntry>[]) => void;

let lastCallback: Callback | undefined;

class FakeIntersectionObserver {
  constructor(callback: Callback) {
    lastCallback = callback;
  }
  observe() {}
  unobserve() {}
  disconnect() {}
  takeRecords() {
    return [];
  }
}

function mockOsScheme(dark: boolean) {
  vi.spyOn(window, "matchMedia").mockImplementation(
    (query: string) =>
      ({
        matches: query === "(prefers-color-scheme: dark)" && dark,
        media: query,
        addEventListener: () => {},
        removeEventListener: () => {},
      }) as unknown as MediaQueryList,
  );
}

function renderHeader(platform: "linux" | "windows" = "linux") {
  return renderWithPlatform(
    <ThemeProvider>
      <SiteHeader />
    </ThemeProvider>,
    platform,
  );
}

beforeEach(() => {
  lastCallback = undefined;
  window.localStorage.clear();
  mockedUseReleases.mockReturnValue({ releases: [release], status: "live" });
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  document.documentElement.classList.remove("dark");
});

it("download_button_points_at_the_deb_on_linux", () => {
  renderHeader("linux");
  expect(screen.getByRole("link", { name: "Download" })).toHaveAttribute("href", "https://example.test/clipsy.deb");
});

it("download_button_points_at_the_windows_installer_on_windows", () => {
  renderHeader("windows");
  expect(screen.getByRole("link", { name: "Download for Windows" })).toHaveAttribute(
    "href",
    "https://example.test/setup.exe",
  );
});

it("download_button_goes_to_install_when_windows_is_coming_soon", () => {
  mockedUseReleases.mockReturnValue({ releases: [{ ...release, windowsInstaller: undefined }], status: "live" });
  renderHeader("windows");
  expect(screen.getByRole("link", { name: "Download" })).toHaveAttribute("href", "#install");
});

it("marks_active_section_link", () => {
  renderHeader();
  const downloads = document.createElement("section");
  downloads.id = "downloads";
  document.body.appendChild(downloads);

  act(() => {
    lastCallback?.([{ isIntersecting: true, target: downloads }]);
  });

  const link = screen.getByRole("link", { name: "Downloads" });
  expect(link).toHaveAttribute("aria-current", "location");
  expect(screen.getByRole("link", { name: "Install" })).not.toHaveAttribute("aria-current");

  downloads.remove();
});

it("closes_mobile_menu_when_link_clicked", async () => {
  renderHeader();
  await userEvent.click(screen.getByRole("button", { name: "Open menu" }));
  const dialog = await screen.findByRole("dialog");
  expect(dialog).toBeInTheDocument();

  await userEvent.click(within(dialog).getByRole("link", { name: "Help" }));
  await vi.waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
});

it("theme_button_describes_the_switch_and_toggles_in_place", async () => {
  mockOsScheme(false);
  renderHeader();

  const toggle = screen.getByRole("button", { name: "Switch to dark theme" });
  expect(toggle).toHaveAttribute("title", "Switch to dark theme");
  expect(screen.queryByRole("menu")).not.toBeInTheDocument();

  await userEvent.click(toggle);
  expect(window.localStorage.getItem("clipsy-theme")).toBe("dark");
  expect(document.documentElement).toHaveClass("dark");

  const back = screen.getByRole("button", { name: "Switch to light theme" });
  expect(back).toHaveAttribute("title", "Switch to light theme");
  await userEvent.click(back);
  expect(window.localStorage.getItem("clipsy-theme")).toBe("light");
  expect(document.documentElement).not.toHaveClass("dark");
});

it("theme_button_has_no_system_option", async () => {
  mockOsScheme(true);
  renderHeader();

  await userEvent.click(screen.getByRole("button", { name: "Switch to light theme" }));
  expect(screen.queryByRole("menuitemradio", { name: "System" })).not.toBeInTheDocument();
  expect(screen.queryByText("System")).not.toBeInTheDocument();
});
