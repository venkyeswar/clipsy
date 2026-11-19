import { describe, expect, it } from "vitest";
import { buildSnapshot } from "../../scripts/snapshot-lib.mjs";

const asset = (name: string) => ({
  name,
  browser_download_url: `https://example.test/${name}`,
  size: 1,
  uploader: { login: "someone" },
});

const release = (overrides: Record<string, unknown> = {}) => ({
  tag_name: "v0.2.0",
  html_url: "https://github.com/venkyeswar/clipsy/releases/tag/v0.2.0",
  published_at: "2026-01-01T00:00:00Z",
  created_at: "2025-12-31T00:00:00Z",
  draft: false,
  prerelease: false,
  body: "release notes",
  assets: [asset("clipsy_0.2.0_amd64.deb")],
  ...overrides,
});

describe("snapshot-lib", () => {
  it("buildSnapshot_drops_drafts", () => {
    const out = buildSnapshot([
      release({ tag_name: "v0.3.0", draft: true }),
      release({ tag_name: "v0.2.0" }),
      null,
    ]);
    expect(out?.map((r: { tag_name: string }) => r.tag_name)).toEqual(["v0.2.0"]);
  });

  it("buildSnapshot_keeps_only_slim_fields", () => {
    const [out] = buildSnapshot([release()]) ?? [];
    expect(Object.keys(out).sort()).toEqual(
      ["assets", "created_at", "draft", "html_url", "prerelease", "published_at", "tag_name"],
    );
    expect(Object.keys(out.assets[0]).sort()).toEqual(["browser_download_url", "name", "size"]);
  });

  it("buildSnapshot_returns_null_for_non_array", () => {
    expect(buildSnapshot({ message: "Not Found" })).toBeNull();
    expect(buildSnapshot(undefined)).toBeNull();
  });
});
