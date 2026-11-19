import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { DownloadsSection } from "./downloads-section";
import { useReleases } from "@/hooks/use-releases";
import { config } from "@/config";
import type { Release } from "@/lib/releases";

vi.mock("@/hooks/use-releases", () => ({ useReleases: vi.fn() }));
const mockedUseReleases = vi.mocked(useReleases);

const asset = (name: string) => ({ name, url: `https://example.test/${name}`, size: 4404019 });

const stable: Release = {
  version: "0.4.0",
  tag: "v0.4.0",
  date: "2026-11-18T12:00:00Z",
  prerelease: false,
  notesUrl: "https://example.test/notes/0.4.0",
  deb: asset("clipsy_0.4.0_amd64.deb"),
  tarball: asset("clipsy-0.4.0-linux-x86_64.tar.gz"),
  checksums: asset("SHA256SUMS"),
};

const older: Release = {
  version: "0.3.1",
  tag: "v0.3.1",
  date: "2026-10-02T12:00:00Z",
  prerelease: false,
  notesUrl: "https://example.test/notes/0.3.1",
  deb: asset("clipsy_0.3.1_amd64.deb"),
};

const preRelease: Release = {
  version: "0.5.0-rc.1",
  tag: "v0.5.0-rc.1",
  date: "2026-11-19T12:00:00Z",
  prerelease: true,
  notesUrl: "https://example.test/notes/0.5.0-rc.1",
};

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

it("renders_skeleton_while_loading", () => {
  mockedUseReleases.mockReturnValue({ releases: [], status: "loading" });
  const { container } = render(<DownloadsSection />);
  expect(container.querySelectorAll('[data-slot="skeleton"]').length).toBeGreaterThan(0);
  expect(screen.queryByRole("link", { name: /download/i })).not.toBeInTheDocument();
});

it("shows_github_link_when_empty", () => {
  mockedUseReleases.mockReturnValue({ releases: [], status: "empty" });
  render(<DownloadsSection />);
  expect(screen.getByText("No releases are published yet.")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "View releases on GitHub" })).toHaveAttribute(
    "href",
    `https://github.com/${config.repo}/releases`,
  );
});

it("hides_missing_asset_buttons", () => {
  const strippedLatest: Release = { ...stable, tarball: undefined, checksums: undefined };
  mockedUseReleases.mockReturnValue({ releases: [strippedLatest, older], status: "live" });
  render(<DownloadsSection />);
  const block = screen.getByRole("heading", { name: "Clipsy 0.4.0" }).closest("[data-release-block]");
  expect(block).not.toBeNull();
  const latestBlock = within(block as HTMLElement);
  expect(latestBlock.getByRole("link", { name: "Download .deb for 0.4.0" })).toBeInTheDocument();
  expect(latestBlock.queryByRole("link", { name: /tarball/i })).not.toBeInTheDocument();
  expect(latestBlock.queryByRole("link", { name: /SHA256SUMS/ })).not.toBeInTheDocument();
  expect(latestBlock.queryByText(/sha256sum -c/)).not.toBeInTheDocument();
});

it("picker_shows_selected_version_in_trigger", async () => {
  mockedUseReleases.mockReturnValue({ releases: [stable, older], status: "live" });
  render(<DownloadsSection />);
  const trigger = screen.getByRole("button", { name: /choose a version/i });
  expect(trigger).toHaveTextContent("Version 0.4.0");
  await userEvent.click(trigger);
  await userEvent.click(await screen.findByRole("menuitem", { name: "Clipsy 0.3.1" }));
  expect(screen.getByRole("button", { name: /choose a version/i })).toHaveTextContent("Version 0.3.1");
  const block = screen.getByRole("heading", { name: "Clipsy 0.3.1" }).closest("[data-release-block]");
  expect(block).not.toBeNull();
});

it("marks_prerelease_with_badge", () => {
  mockedUseReleases.mockReturnValue({ releases: [preRelease, stable], status: "live" });
  render(<DownloadsSection />);
  const table = screen.getByRole("table", { name: "Previous versions" });
  const row = within(table).getByText("0.5.0-rc.1").closest("tr");
  expect(row).not.toBeNull();
  expect(within(row as HTMLElement).getByText("Pre-release")).toBeInTheDocument();
});

it("lists_previous_versions_excluding_latest", () => {
  mockedUseReleases.mockReturnValue({ releases: [stable, preRelease, older], status: "live" });
  render(<DownloadsSection />);
  const table = screen.getByRole("table", { name: "Previous versions" });
  const rows = within(table).getAllByRole("row").slice(1);
  expect(rows).toHaveLength(2);
  expect(within(rows[0]).getByText("0.5.0-rc.1")).toBeInTheDocument();
  expect(within(rows[1]).getByText("0.3.1")).toBeInTheDocument();
  expect(within(rows[0]).getByText("Not available")).toBeInTheDocument();
  expect(within(rows[1]).getByRole("link", { name: "Download .deb for 0.3.1" })).toBeInTheDocument();
  expect(within(table).queryByText("0.4.0")).not.toBeInTheDocument();
});

it("snapshot_status_shows_notice", () => {
  mockedUseReleases.mockReturnValue({ releases: [stable], status: "snapshot" });
  render(<DownloadsSection />);
  expect(
    screen.getByText("Showing the last saved release list because GitHub could not be reached."),
  ).toBeInTheDocument();
});

it("shows_no_stable_message_when_only_prereleases", () => {
  mockedUseReleases.mockReturnValue({ releases: [preRelease], status: "live" });
  render(<DownloadsSection />);
  expect(screen.getByText("No stable release yet.")).toBeInTheDocument();
  const table = screen.getByRole("table", { name: "Previous versions" });
  expect(within(table).getByText("0.5.0-rc.1")).toBeInTheDocument();
});
