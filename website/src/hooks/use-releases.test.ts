import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { loadReleases, readFreshCache, resetReleasesRequests, useReleases } from "./use-releases";

// The hook test relies on an empty bundled snapshot.
vi.mock("../data/releases.snapshot.json", () => ({ default: [] }));

const KEY = "clipsy-releases-v1";
const NOW = 1_700_000_000_000;
const FIVE_MINUTES = 5 * 60 * 1000;

const rawRelease = (tag: string) => ({
  tag_name: tag,
  html_url: `https://github.com/venkyeswar/clipsy/releases/tag/${tag}`,
  published_at: "2026-01-01T00:00:00Z",
  draft: false,
  prerelease: false,
  assets: [],
});

const SNAPSHOT = [rawRelease("v0.1.0")];

function memoryStorage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial));
  return {
    data,
    getItem: (key: string) => data.get(key) ?? null,
    setItem: (key: string, value: string) => {
      data.set(key, value);
    },
  };
}

function fakeFetch(status: number, body: unknown) {
  return vi.fn(async () => ({
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  })) as unknown as typeof fetch & ReturnType<typeof vi.fn>;
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

beforeEach(() => {
  window.sessionStorage.clear();
  resetReleasesRequests();
});

describe("loadReleases", () => {
  it("falls_back_to_snapshot_on_403", async () => {
    const fetchImpl = fakeFetch(403, { message: "API rate limit exceeded" });
    const result = await loadReleases(fetchImpl, memoryStorage(), () => NOW, SNAPSHOT);
    expect(result.status).toBe("snapshot");
    expect(result.releases.map((r) => r.version)).toEqual(["0.1.0"]);
    expect(fetchImpl).toHaveBeenCalledTimes(1);
  });

  it("uses_snapshot_when_live_list_is_empty", async () => {
    const result = await loadReleases(fakeFetch(200, []), memoryStorage(), () => NOW, SNAPSHOT);
    expect(result.status).toBe("snapshot");
    expect(result.releases.map((r) => r.version)).toEqual(["0.1.0"]);
  });

  it("falls_back_on_non_array_and_bad_json", async () => {
    const nonArray = await loadReleases(fakeFetch(200, { message: "oops" }), memoryStorage(), () => NOW, SNAPSHOT);
    expect(nonArray.status).toBe("snapshot");
    const badJson = {
      fetch: vi.fn(async () => ({
        ok: true,
        status: 200,
        json: async () => {
          throw new SyntaxError("Unexpected token");
        },
      })) as unknown as typeof fetch,
    };
    const parseFailure = await loadReleases(badJson.fetch, memoryStorage(), () => NOW, SNAPSHOT);
    expect(parseFailure.status).toBe("snapshot");
  });

  it("empty_when_both_empty", async () => {
    const result = await loadReleases(fakeFetch(403, {}), memoryStorage(), () => NOW, []);
    expect(result).toEqual({ releases: [], status: "empty" });
  });

  it("always_revalidates_fresh_cache_and_live_result_wins", async () => {
    const fetchImpl = fakeFetch(200, [rawRelease("v9.9.9")]);
    const storage = memoryStorage({
      [KEY]: JSON.stringify({ t: NOW - 60_000, data: [rawRelease("v0.2.0")] }),
    });
    const result = await loadReleases(fetchImpl, storage, () => NOW, SNAPSHOT);
    expect(fetchImpl).toHaveBeenCalledTimes(1);
    expect(result.status).toBe("live");
    expect(result.releases.map((r) => r.version)).toEqual(["9.9.9"]);
  });

  it("falls_back_to_fresh_cache_when_request_fails", async () => {
    const storage = memoryStorage({
      [KEY]: JSON.stringify({ t: NOW - 60_000, data: [rawRelease("v0.2.0")] }),
    });
    const result = await loadReleases(fakeFetch(403, {}), storage, () => NOW, SNAPSHOT);
    expect(result.status).toBe("live");
    expect(result.releases.map((r) => r.version)).toEqual(["0.2.0"]);
  });

  it("ignores_stale_cache_and_refetches", async () => {
    const fetchImpl = fakeFetch(200, [rawRelease("v0.3.0")]);
    const storage = memoryStorage({
      [KEY]: JSON.stringify({ t: NOW - FIVE_MINUTES - 1, data: [rawRelease("v0.2.0")] }),
    });
    const result = await loadReleases(fetchImpl, storage, () => NOW, SNAPSHOT);
    expect(fetchImpl).toHaveBeenCalledTimes(1);
    expect(result.status).toBe("live");
    expect(result.releases.map((r) => r.version)).toEqual(["0.3.0"]);
    expect(JSON.parse(storage.data.get(KEY) ?? "{}").t).toBe(NOW);
  });

  it("stale_cache_is_not_used_when_request_fails", async () => {
    const storage = memoryStorage({
      [KEY]: JSON.stringify({ t: NOW - FIVE_MINUTES - 1, data: [rawRelease("v0.2.0")] }),
    });
    const result = await loadReleases(fakeFetch(403, {}), storage, () => NOW, SNAPSHOT);
    expect(result.status).toBe("snapshot");
    expect(result.releases.map((r) => r.version)).toEqual(["0.1.0"]);
  });

  it("readFreshCache_honours_the_five_minute_ttl", () => {
    const data = [rawRelease("v0.2.0")];
    const at = (age: number) => memoryStorage({ [KEY]: JSON.stringify({ t: NOW - age, data }) });
    expect(readFreshCache(at(FIVE_MINUTES - 1), () => NOW)?.[0].version).toBe("0.2.0");
    expect(readFreshCache(at(FIVE_MINUTES), () => NOW)).toBeNull();
    expect(readFreshCache(memoryStorage(), () => NOW)).toBeNull();
  });

  it("survives_throwing_sessionStorage", async () => {
    const throwing = {
      getItem: () => {
        throw new Error("storage denied");
      },
      setItem: () => {
        throw new Error("storage denied");
      },
    };
    const live = await loadReleases(fakeFetch(200, [rawRelease("v0.4.0")]), throwing, () => NOW, SNAPSHOT);
    expect(live.status).toBe("live");
    expect(live.releases[0].version).toBe("0.4.0");

    const offline = await loadReleases(
      vi.fn(async () => {
        throw new TypeError("Failed to fetch");
      }) as unknown as typeof fetch,
      throwing,
      () => NOW,
      SNAPSHOT,
    );
    expect(offline.status).toBe("snapshot");
  });

  it("survives_throwing_default_storage", async () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("storage denied");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("storage denied");
    });
    const result = await loadReleases(fakeFetch(200, [rawRelease("v0.5.0")]), undefined, () => NOW, SNAPSHOT);
    expect(result.status).toBe("live");
    expect(result.releases[0].version).toBe("0.5.0");
  });
});

describe("useReleases", () => {
  it("empty_when_both_empty", async () => {
    vi.stubGlobal("fetch", fakeFetch(403, { message: "rate limited" }));
    const { result } = renderHook(() => useReleases());
    expect(result.current).toEqual({ releases: [], status: "loading" });
    await waitFor(() => expect(result.current.status).not.toBe("loading"));
    expect(result.current).toEqual({ releases: [], status: "empty" });
  });
});

describe("shared release request", () => {
  it("two_mounts_make_exactly_one_fetch", async () => {
    const fetchMock = fakeFetch(200, [rawRelease("v0.3.0")]);
    vi.stubGlobal("fetch", fetchMock);
    const first = renderHook(() => useReleases());
    const second = renderHook(() => useReleases());
    await waitFor(() => expect(first.result.current.status).toBe("live"));
    await waitFor(() => expect(second.result.current.status).toBe("live"));
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("later_mount_paints_cache_then_revalidates", async () => {
    const fetchMock = fakeFetch(200, [rawRelease("v0.3.0")]);
    vi.stubGlobal("fetch", fetchMock);
    const first = renderHook(() => useReleases());
    await waitFor(() => expect(first.result.current.status).toBe("live"));
    first.unmount();
    const later = renderHook(() => useReleases());
    // The recent cache is shown straight away, before the background request finishes.
    expect(later.result.current.releases.map((r) => r.version)).toEqual(["0.3.0"]);
    await waitFor(() => expect(later.result.current.status).toBe("live"));
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("live_result_replaces_the_cached_list", async () => {
    window.sessionStorage.setItem(KEY, JSON.stringify({ t: Date.now(), data: [rawRelease("v0.2.0")] }));
    vi.stubGlobal("fetch", fakeFetch(200, [rawRelease("v0.3.0")]));
    const { result } = renderHook(() => useReleases());
    expect(result.current.releases.map((r) => r.version)).toEqual(["0.2.0"]);
    await waitFor(() => expect(result.current.releases.map((r) => r.version)).toEqual(["0.3.0"]));
    expect(result.current.status).toBe("live");
  });

  it("failed_fetch_is_not_retried_within_the_negative_window", async () => {
    const fetchMock = fakeFetch(403, { message: "rate limited" });
    vi.stubGlobal("fetch", fetchMock);
    const first = renderHook(() => useReleases());
    await waitFor(() => expect(first.result.current.status).not.toBe("loading"));
    first.unmount();
    const second = renderHook(() => useReleases());
    await waitFor(() => expect(second.result.current.status).not.toBe("loading"));
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});
