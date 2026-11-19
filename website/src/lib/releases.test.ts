import { describe, expect, it } from "vitest";
import { compareVersions, latestStable, parseReleases } from "./releases";

const asset = (name: string, size = 10) => ({
  name,
  browser_download_url: `https://example.test/download/${name}`,
  size,
});

const release = (overrides: Record<string, unknown> = {}) => ({
  tag_name: "v0.2.0",
  html_url: "https://github.com/venkyeswar/clipsy/releases/tag/v0.2.0",
  published_at: "2026-01-01T00:00:00Z",
  created_at: "2025-12-31T00:00:00Z",
  draft: false,
  prerelease: false,
  assets: [],
  ...overrides,
});

describe("releases", () => {
  it("compareVersions_orders_numerically", () => {
    expect(compareVersions("0.10.0", "0.9.2")).toBeGreaterThan(0);
    expect(compareVersions("0.9.2", "0.10.0")).toBeLessThan(0);
    expect(compareVersions("1.0.0", "1.0.0-rc.1")).toBeGreaterThan(0);
    expect(compareVersions("1.0.0-rc.1", "1.0.0")).toBeLessThan(0);
    expect(compareVersions("1.2.3", "1.2.3")).toBe(0);
  });

  it("parseReleases_drops_drafts_and_junk", () => {
    const raw = [
      release({ tag_name: "v1.0.0" }),
      release({ tag_name: "v2.0.0", draft: true }),
      null,
      "junk",
      42,
      { html_url: "https://example.test/no-tag" },
      { tag_name: 5 },
    ];
    const versions = parseReleases(raw).map((r) => r.version);
    expect(versions).toEqual(["1.0.0"]);
  });

  it("parseReleases_picks_assets_by_pattern", () => {
    const [r] = parseReleases([
      release({
        assets: [
          asset("SHA256SUMS"),
          asset("notes.txt"),
          asset("clipsy-0.2.0-linux-aarch64.tar.gz"),
          asset("clipsy-0.2.0-linux-x86_64.tar.gz", 999),
          asset("clipsy_0.2.0_amd64.deb", 123),
          asset("clipsy_0.2.0_arm64.deb", 456),
        ],
      }),
    ]);
    expect(r.deb).toEqual({
      name: "clipsy_0.2.0_amd64.deb",
      url: "https://example.test/download/clipsy_0.2.0_amd64.deb",
      size: 123,
    });
    expect(r.tarball?.name).toBe("clipsy-0.2.0-linux-x86_64.tar.gz");
    expect(r.tarball?.size).toBe(999);
    expect(r.checksums?.name).toBe("SHA256SUMS");
  });

  it("parseReleases_handles_release_without_deb", () => {
    const [r] = parseReleases([
      release({ assets: [asset("clipsy-0.2.0-linux-x86_64.tar.gz")] }),
      release({ tag_name: "v0.1.0", assets: undefined }),
    ]);
    expect(r.deb).toBeUndefined();
    expect(r.tarball?.name).toBe("clipsy-0.2.0-linux-x86_64.tar.gz");
    expect(parseReleases([release({ tag_name: "v0.1.0", assets: undefined })])[0].checksums).toBeUndefined();
  });

  it("parseReleases_fills_defaults", () => {
    const [withDate] = parseReleases([release({ published_at: undefined })]);
    expect(withDate.date).toBe("2025-12-31T00:00:00Z");
    const [noDate] = parseReleases([release({ published_at: undefined, created_at: undefined })]);
    expect(noDate.date).toBe("");
    const [noHtml] = parseReleases([release({ tag_name: "v3.1.0", html_url: undefined })]);
    expect(noHtml.notesUrl).toBe("https://github.com/venkyeswar/clipsy/releases/tag/v3.1.0");
    expect(noHtml.tag).toBe("v3.1.0");
    expect(noHtml.version).toBe("3.1.0");
  });

  it("latestStable_skips_prereleases", () => {
    const releases = parseReleases([
      release({ tag_name: "v0.3.0-rc.1", prerelease: true }),
      release({ tag_name: "v0.2.0" }),
      release({ tag_name: "v0.1.0" }),
    ]);
    expect(latestStable(releases)?.version).toBe("0.2.0");
    expect(latestStable([])).toBeUndefined();
    expect(latestStable(parseReleases([release({ tag_name: "v0.3.0", prerelease: true })]))).toBeUndefined();
  });

  it("parseReleases_non_array_returns_empty", () => {
    expect(parseReleases(null)).toEqual([]);
    expect(parseReleases(undefined)).toEqual([]);
    expect(parseReleases({ tag_name: "v1.0.0" })).toEqual([]);
    expect(parseReleases("[]")).toEqual([]);
  });

  it("parseReleases_sorts_by_semver_not_date", () => {
    const releases = parseReleases([
      release({ tag_name: "v0.9.2", published_at: "2026-06-01T00:00:00Z" }),
      release({ tag_name: "v0.10.0", published_at: "2026-01-01T00:00:00Z" }),
      release({ tag_name: "v1.0.0-rc.1", published_at: "2026-07-01T00:00:00Z", prerelease: true }),
      release({ tag_name: "v1.0.0", published_at: "2026-02-01T00:00:00Z" }),
    ]);
    expect(releases.map((r) => r.version)).toEqual(["1.0.0", "1.0.0-rc.1", "0.10.0", "0.9.2"]);
  });
});
