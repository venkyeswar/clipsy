import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Hero } from "./hero";
import { useReleases } from "@/hooks/use-releases";
import type { Release } from "@/lib/releases";

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

afterEach(cleanup);

it("reserves_version_line_while_loading", () => {
  mockedUseReleases.mockReturnValue({ releases: [], status: "loading" });
  render(<Hero />);
  const line = screen.getByTestId("version-line");
  expect(line).toHaveClass("min-h-5");
  expect(line).toBeEmptyDOMElement();

  cleanup();
  mockedUseReleases.mockReturnValue({ releases: [release], status: "live" });
  render(<Hero />);
  expect(screen.getByTestId("version-line")).toHaveTextContent(
    "Version 0.3.1, released Nov 18, 2026.",
  );
});

it("falls_back_to_view_on_github_without_deb", () => {
  mockedUseReleases.mockReturnValue({ releases: [{ ...release, deb: undefined }], status: "live" });
  render(<Hero />);
  expect(screen.getByRole("link", { name: "View on GitHub" })).toHaveAttribute(
    "href",
    "https://github.com/venkyeswar/clipsy/releases",
  );
  expect(screen.queryByRole("link", { name: /Download \.deb/ })).toBeNull();
});
