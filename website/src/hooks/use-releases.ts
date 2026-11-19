import { useEffect, useState } from "react";
import { config } from "../config";
import snapshot from "../data/releases.snapshot.json";
import { parseReleases, type Release } from "../lib/releases";

export type ReleasesStatus = "loading" | "live" | "snapshot" | "empty";
export type ReleasesState = { releases: Release[]; status: ReleasesStatus };

const CACHE_KEY = "clipsy-releases-v1";
const CACHE_TTL_MS = 10 * 60 * 1000;
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

/**
 * Resolves the release list: fresh sessionStorage cache, then the GitHub API,
 * then the bundled snapshot. Never throws.
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

  const cached = readCache(storage, now);
  if (cached) {
    const releases = parseReleases(cached);
    if (releases.length > 0) return { releases, status: "live" };
    return fromSnapshot();
  }

  let body: unknown;
  try {
    const res = await fetchImpl(RELEASES_URL, {
      headers: { Accept: "application/vnd.github+json" },
      signal,
    });
    if (!res.ok) return fromSnapshot();
    body = await res.json();
  } catch {
    return fromSnapshot();
  }
  if (!Array.isArray(body)) return fromSnapshot();

  writeCache(storage, body, now);
  const releases = parseReleases(body);
  if (releases.length > 0) return { releases, status: "live" };
  return fromSnapshot();
}

export function useReleases(): ReleasesState {
  const [state, setState] = useState<ReleasesState>({ releases: [], status: "loading" });

  useEffect(() => {
    let active = true;
    const controller = new AbortController();
    loadReleases(fetch, safeSessionStorage, Date.now, snapshot, controller.signal).then((result) => {
      if (active) setState(result);
    });
    return () => {
      active = false;
      controller.abort();
    };
  }, []);

  return state;
}
