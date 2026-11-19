import { cleanup, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Hero } from "./hero";
import { useReleases } from "@/hooks/use-releases";
import type { Release } from "@/lib/releases";
import { renderWithPlatform } from "@/test/render-with-platform";

vi.mock("@/hooks/use-releases", () => ({ useReleases: vi.fn() }));
const mockedUseReleases = vi.mocked(useReleases);

const release: Release = {
  version: "0.3.1",
  tag: "v0.3.1",
  date: "2026-11-18T12:00:00",
  prerelease: false,
  notesUrl: "https://example.test/notes",
  deb: { name: "clipsy_0.3.1_amd64.deb", url: "https://example.test/clipsy.deb", size: 1 },
};

const withWindows: Release = {
  ...release,
  windowsInstaller: { name: "clipsy-0.3.1-setup.exe", url: "https://example.test/setup.exe", size: 1 },
  windowsPortable: { name: "clipsy-0.3.1-windows-x64.zip", url: "https://example.test/clipsy.zip", size: 1 },
};

beforeEach(() => {
  window.localStorage.clear();
});

afterEach(cleanup);

it("reserves_version_line_while_loading", () => {
  mockedUseReleases.mockReturnValue({ releases: [], status: "loading" });
  renderWithPlatform(<Hero />);
  const line = screen.getByTestId("version-line");
  expect(line).toHaveClass("min-h-5");
  expect(line).toBeEmptyDOMElement();

  cleanup();
  mockedUseReleases.mockReturnValue({ releases: [release], status: "live" });
  renderWithPlatform(<Hero />);
  expect(screen.getByTestId("version-line")).toHaveTextContent("Version 0.3.1, released Nov 18, 2026.");
});

it("falls_back_to_view_on_github_without_deb", () => {
  mockedUseReleases.mockReturnValue({ releases: [{ ...release, deb: undefined }], status: "live" });
  renderWithPlatform(<Hero />);
  expect(screen.getByRole("link", { name: "View on GitHub" })).toHaveAttribute(
    "href",
    "https://github.com/venkyeswar/clipsy/releases",
  );
  expect(screen.queryByRole("link", { name: /Download for Linux/ })).toBeNull();
});

it("shows_linux_and_windows_download_buttons_when_windows_is_available", () => {
  mockedUseReleases.mockReturnValue({ releases: [withWindows], status: "live" });
  renderWithPlatform(<Hero />);
  expect(screen.getByRole("link", { name: "Download for Linux (.deb)" })).toHaveAttribute(
    "href",
    "https://example.test/clipsy.deb",
  );
  expect(screen.getByRole("link", { name: "Download for Windows (.exe)" })).toHaveAttribute(
    "href",
    "https://example.test/setup.exe",
  );
  expect(screen.queryByText("Coming soon")).not.toBeInTheDocument();
});

it("shows_windows_as_coming_soon_without_a_link", () => {
  mockedUseReleases.mockReturnValue({ releases: [release], status: "live" });
  renderWithPlatform(<Hero />);
  const windows = screen.getByRole("button", { name: "Download for Windows (.exe)" });
  expect(windows).toHaveAttribute("aria-disabled", "true");
  expect(screen.queryByRole("link", { name: "Download for Windows (.exe)" })).not.toBeInTheDocument();
  expect(screen.getByText("Coming soon")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Build it from source on Windows" })).toHaveAttribute("href", "#install");
});

it("marks_the_visitors_platform_button_as_primary", () => {
  mockedUseReleases.mockReturnValue({ releases: [withWindows], status: "live" });
  const { unmount } = renderWithPlatform(<Hero />, "linux");
  expect(screen.getByRole("link", { name: "Download for Linux (.deb)" }).className).toContain("shadow-[");
  expect(screen.getByRole("link", { name: "Download for Windows (.exe)" }).className).not.toContain("shadow-[");
  unmount();

  renderWithPlatform(<Hero />, "windows");
  expect(screen.getByRole("link", { name: "Download for Windows (.exe)" }).className).toContain("shadow-[");
  expect(screen.getByRole("link", { name: "Download for Linux (.deb)" }).className).not.toContain("shadow-[");
});

it("shows_the_shortcut_for_the_selected_platform", () => {
  mockedUseReleases.mockReturnValue({ releases: [withWindows], status: "live" });
  renderWithPlatform(<Hero />, "windows");
  const chip = screen.getByRole("group", { name: "Default shortcut" });
  expect(within(chip).getAllByText(/^(Ctrl|Shift|V)$/).map((key) => key.textContent)).toEqual([
    "Ctrl",
    "Shift",
    "V",
  ]);
});

it("hides_the_no_sudo_link_on_windows", () => {
  mockedUseReleases.mockReturnValue({ releases: [withWindows], status: "live" });
  const { unmount } = renderWithPlatform(<Hero />, "windows");
  expect(screen.queryByText(/Can't use sudo/)).not.toBeInTheDocument();
  unmount();
  renderWithPlatform(<Hero />, "linux");
  expect(screen.getByText(/Can't use sudo/)).toBeInTheDocument();
});
