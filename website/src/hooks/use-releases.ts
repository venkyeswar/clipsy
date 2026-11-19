import { useEffect, useState } from "react";
import { config } from "../config";
import snapshot from "../data/releases.snapshot.json";
import { parseReleases, type Release } from "../lib/releases";

export type ReleasesStatus = "loading" | "live" | "snapshot" | "empty";
export type ReleasesState = { releases: Release[]; status: ReleasesStatus };

const CACHE_KEY = "clipsy-releases-v1";
/** Short on purpose: a cached list is only a first paint or a fallback, never the final answer. */
const CACHE_TTL_MS = 5 * 60 * 1000;
const RELEASES_URL = `https://api.github.com/repos/${config.repo}/releases`;

type StorageLike = {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
};

/** sessionStorage wrapper that never throws (private mode, blocked storage). */
export const safeSessionStorage: StorageLike = {
  getItem(key) {
    try {
      return window.sessionStorage.getItem(key);
    } catch {
      return null;
    }
  },
  setItem(key, value) {
    try {
      window.sessionStorage.setItem(key, value);
    } catch {
      // Caching is best effort.
    }
  },
};

function readCache(storage: StorageLike, now: () => number): unknown[] | null {
  try {
    const text = storage.getItem(CACHE_KEY);
    if (!text) return null;
    const parsed: unknown = JSON.parse(text);
    if (typeof parsed !== "object" || parsed === null) return null;
    const { t, data } = parsed as { t?: unknown; data?: unknown };
    if (typeof t !== "number" || !Array.isArray(data)) return null;
    return now() - t < CACHE_TTL_MS ? data : null;
  } catch {
    return null;
  }
}

function writeCache(storage: StorageLike, data: unknown[], now: () => number) {
  try {
    storage.setItem(CACHE_KEY, JSON.stringify({ t: now(), data }));
  } catch {
    // Caching is best effort.
  }
}

/** A recent live list from sessionStorage (under CACHE_TTL_MS old), or null. Never throws. */
export function readFreshCache(
  storage: StorageLike = safeSessionStorage,
  now: () => number = Date.now,
): Release[] | null {
  const data = readCache(storage, now);
  if (!data) return null;
  const releases = parseReleases(data);
  return releases.length > 0 ? releases : null;
}

/**
 * Always asks GitHub for the live list. The result replaces anything shown before.
 * If the request fails, a fresh cached list is used; otherwise the bundled snapshot. Never throws.
 */
export async function loadReleases(
  fetchImpl: typeof fetch = fetch,
  storage: StorageLike = safeSessionStorage,
  now: () => number = Date.now,
  snapshotRaw: unknown = snapshot,
  signal?: AbortSignal,
): Promise<{ releases: Release[]; status: "live" | "snapshot" | "empty" }> {
  const fromSnapshot = () => {
    const releases = parseReleases(snapshotRaw);
    return releases.length > 0
      ? { releases, status: "snapshot" as const }
      : { releases, status: "empty" as const };
  };
  const fallback = () => {
    const cached = readFreshCache(storage, now);
    return cached ? { releases: cached, status: "live" as const } : fromSnapshot();
  };

  let body: unknown;
  try {
    const res = await fetchImpl(RELEASES_URL, {
      headers: { Accept: "application/vnd.github+json" },
      signal,
    });
    if (!res.ok) return fallback();
    body = await res.json();
  } catch {
    return fallback();
  }
  if (!Array.isArray(body)) return fallback();

  writeCache(storage, body, now);
  const releases = parseReleases(body);
  if (releases.length > 0) return { releases, status: "live" };
  return fromSnapshot();
}

type LoadResult = { releases: Release[]; status: "live" | "snapshot" | "empty" };

/** After a failed or rate-limited request, do not ask GitHub again for this long. */
const NEGATIVE_TTL_MS = 2 * 60 * 1000;

// Module-level state shared by every mounted component, so a cold page load makes one request.
let inFlight: Promise<LoadResult> | null = null;
let failedAt: number | null = null;

/** Clears the shared request state. Tests use it to start from a clean slate. */
export function resetReleasesRequests(): void {
  inFlight = null;
  failedAt = null;
}

/**
 * One release request for the whole page: concurrent callers share the in-flight promise,
 * and a failure is remembered briefly so later mounts fall back to the snapshot without retrying.
 */
export function requestReleases(): Promise<LoadResult> {
  if (inFlight) return inFlight;

  const recentlyFailed = failedAt !== null && Date.now() - failedAt < NEGATIVE_TTL_MS;
  let requestFailed = false;
  // A recent failure still uses the fresh sessionStorage cache as fallback, but never calls the network.
  const fetchImpl: typeof fetch = recentlyFailed
    ? () => Promise.reject(new Error("GitHub request failed recently"))
    : async (input, init) => {
        try {
          const response = await fetch(input, init);
          if (!response.ok) requestFailed = true;
          return response;
        } catch (error) {
          requestFailed = true;
          throw error;
        }
      };

  const request = loadReleases(fetchImpl, safeSessionStorage, Date.now, snapshot).then((result) => {
    // Only a real request updates the failure time; blocked requests leave the window alone.
    if (!recentlyFailed) failedAt = requestFailed ? Date.now() : null;
    return result;
  });
  inFlight = request.finally(() => {
    inFlight = null;
  });
  return inFlight;
}

export function useReleases(): ReleasesState {
  // Paint the recent cache at once, then always replace it with the live result.
  const [state, setState] = useState<ReleasesState>(() => {
    const cached = readFreshCache();
    return cached ? { releases: cached, status: "live" } : { releases: [], status: "loading" };
  });

  useEffect(() => {
    let active = true;
    requestReleases().then((result) => {
      if (active) setState(result);
    });
    return () => {
      active = false;
    };
  }, []);

  return state;
}
