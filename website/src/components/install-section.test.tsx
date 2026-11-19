import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { InstallSection } from "./install-section";
import { useReleases } from "@/hooks/use-releases";
import { config } from "@/config";
import type { Release } from "@/lib/releases";

vi.mock("@/hooks/use-releases", () => ({ useReleases: vi.fn() }));
const mockedUseReleases = vi.mocked(useReleases);

const release: Release = {
  version: "0.4.0",
  tag: "v0.4.0",
  date: "2026-11-18T12:00:00Z",
  prerelease: false,
  notesUrl: "https://example.test/notes",
  deb: { name: "clipsy_0.4.0_amd64.deb", url: "https://example.test/clipsy.deb", size: 1 },
  tarball: { name: "clipsy-0.4.0-linux-x86_64.tar.gz", url: "https://example.test/clipsy.tar.gz", size: 1 },
};

afterEach(() => {
  cleanup();
  config.snap.channel = "edge";
});

it("renders_four_tabs_with_snap_edge", () => {
  mockedUseReleases.mockReturnValue({ releases: [release], status: "live" });
  render(<InstallSection />);
  const tabs = screen.getAllByRole("tab");
  expect(tabs.map((tab) => tab.textContent)).toEqual([
    "Ubuntu App Center / Snap",
    ".deb package",
    "No sudo",
    "Build from source",
  ]);
  expect(tabs[0]).toHaveAttribute("aria-selected", "true");
  expect(screen.getByText("sudo snap install clipsy --edge")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Open in App Center" })).toHaveAttribute(
    "href",
    "snap://clipsy",
  );
});

it("hides_snap_tab_when_channel_null", () => {
  config.snap.channel = null;
  mockedUseReleases.mockReturnValue({ releases: [release], status: "live" });
  render(<InstallSection />);
  expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual([
    ".deb package",
    "No sudo",
    "Build from source",
  ]);
  expect(screen.queryByRole("link", { name: "Open in App Center" })).not.toBeInTheDocument();
});

it("deb_tab_shows_releases_button_when_no_deb", async () => {
  mockedUseReleases.mockReturnValue({ releases: [{ ...release, deb: undefined }], status: "live" });
  render(<InstallSection />);
  await userEvent.click(screen.getByRole("tab", { name: ".deb package" }));
  expect(screen.getByRole("link", { name: "View releases on GitHub" })).toHaveAttribute(
    "href",
    "https://github.com/venkyeswar/clipsy/releases",
  );
  expect(screen.queryByText(/^wget /)).not.toBeInTheDocument();
});

it("nosudo_tab_never_shows_releases_fallback", async () => {
  mockedUseReleases.mockReturnValue({ releases: [{ ...release, deb: undefined }], status: "live" });
  render(<InstallSection />);
  await userEvent.click(screen.getByRole("tab", { name: "No sudo" }));
  const panel = screen.getByRole("tabpanel");
  expect(within(panel).queryByRole("link", { name: "View releases on GitHub" })).not.toBeInTheDocument();
  expect(within(panel).getByText("sh install.sh")).toBeInTheDocument();
  expect(within(panel).queryByText(/tar -xzf/)).not.toBeInTheDocument();
});

it("shows_skeleton_not_fallback_while_loading", async () => {
  mockedUseReleases.mockReturnValue({ releases: [], status: "loading" });
  const { container } = render(<InstallSection />);
  await userEvent.click(screen.getByRole("tab", { name: ".deb package" }));
  expect(screen.queryByText(/No .deb package is published/)).not.toBeInTheDocument();
  expect(screen.queryByRole("link", { name: "View releases on GitHub" })).not.toBeInTheDocument();
  expect(container.querySelector('[data-slot="skeleton"]')).toBeInTheDocument();
  expect(screen.getByText("Download the package")).toBeInTheDocument();
});

it("nosudo_tab_shows_numbered_steps_and_commands_while_loading", async () => {
  mockedUseReleases.mockReturnValue({ releases: [], status: "loading" });
  render(<InstallSection />);
  await userEvent.click(screen.getByRole("tab", { name: "No sudo" }));
  const panel = screen.getByRole("tabpanel");
  expect(panel.querySelector('[data-slot="skeleton"]')).not.toBeInTheDocument();
  const steps = within(panel).getByRole("list", { name: "Steps for No sudo" });
  expect(steps.tagName).toBe("OL");
  const items = within(steps).getAllByRole("listitem");
  expect(items.map((item) => item.querySelector("p")?.textContent)).toEqual([
    "Download the installer",
    "Run it",
    "Start Clipsy",
  ]);
  expect(
    within(items[0]).getByText(
      "curl -fsSL https://github.com/venkyeswar/clipsy/releases/latest/download/install.sh -o install.sh",
    ),
  ).toBeInTheDocument();
  expect(within(items[1]).getByText("sh install.sh")).toBeInTheDocument();
  expect(within(items[2]).getByText("~/.local/bin/clipsy")).toBeInTheDocument();
  expect(within(panel).getByText("sh ~/.local/share/clipsy/uninstall.sh")).toBeInTheDocument();
  expect(within(panel).getAllByRole("button", { name: "Copy command" })).toHaveLength(4);
});

it("deb_tab_shows_numbered_steps_with_release_commands", async () => {
  mockedUseReleases.mockReturnValue({ releases: [release], status: "live" });
  render(<InstallSection />);
  await userEvent.click(screen.getByRole("tab", { name: ".deb package" }));
  const steps = screen.getByRole("list", { name: "Steps for .deb package" });
  expect(within(steps).getByText("wget https://example.test/clipsy.deb")).toBeInTheDocument();
  expect(within(steps).getByText("sudo apt install ./clipsy_0.4.0_amd64.deb")).toBeInTheDocument();
  expect(within(steps).getAllByRole("listitem").at(-1)).toHaveTextContent("Open Clipsy");
});

it("shows_after_installing_footer_with_shortcut", () => {
  mockedUseReleases.mockReturnValue({ releases: [release], status: "live" });
  const { container } = render(<InstallSection />);
  expect(screen.getByText("After installing:")).toBeInTheDocument();
  expect([...container.querySelectorAll("kbd")].map((key) => key.textContent)).toEqual(config.shortcut);
});
