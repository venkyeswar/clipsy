import { cleanup, render, screen } from "@testing-library/react";
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

it("nosudo_tab_shows_releases_button_when_no_tarball", async () => {
  mockedUseReleases.mockReturnValue({ releases: [{ ...release, tarball: undefined }], status: "live" });
  render(<InstallSection />);
  await userEvent.click(screen.getByRole("tab", { name: "No sudo" }));
  expect(screen.getByRole("link", { name: "View releases on GitHub" })).toBeInTheDocument();
  expect(screen.queryByText(/^wget /)).not.toBeInTheDocument();
  expect(screen.queryByText(/tar -xzf/)).not.toBeInTheDocument();
});

it("shows_skeleton_not_fallback_while_loading", async () => {
  mockedUseReleases.mockReturnValue({ releases: [], status: "loading" });
  const { container } = render(<InstallSection />);
  await userEvent.click(screen.getByRole("tab", { name: ".deb package" }));
  expect(screen.queryByText(/No .deb package is published/)).not.toBeInTheDocument();
  expect(screen.queryByRole("link", { name: "View releases on GitHub" })).not.toBeInTheDocument();
  expect(container.querySelector('[data-slot="skeleton"]')).toBeInTheDocument();

  await userEvent.click(screen.getByRole("tab", { name: "No sudo" }));
  expect(screen.queryByText(/No no-sudo package/)).not.toBeInTheDocument();
  expect(container.querySelector('[data-slot="skeleton"]')).toBeInTheDocument();
});
