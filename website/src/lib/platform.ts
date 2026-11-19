import { useState } from "react";

export type Platform = "windows" | "linux" | "mac" | "other";

/**
 * Classifies a User-Agent string. Android also says "Linux", so it is checked first.
 * iOS and iPadOS are "other": Clipsy has no iOS build.
 */
export function detectPlatform(userAgent: string): Platform {
  if (/Android/i.test(userAgent)) return "other";
  if (/Windows|Win64|WOW64|Win32/i.test(userAgent)) return "windows";
  if (/iPhone|iPad|iPod/i.test(userAgent)) return "other";
  if (/Macintosh|Mac OS X/i.test(userAgent)) return "mac";
  if (/Linux|X11/i.test(userAgent) && !/CrOS/i.test(userAgent)) return "linux";
  return "other";
}

/** Reads the visitor's platform; falls back to linux when there is no navigator (tests, SSR). */
function readPlatform(): Platform {
  if (typeof navigator === "undefined" || typeof navigator.userAgent !== "string") return "linux";
  return detectPlatform(navigator.userAgent);
}

/** The detected platform, which the visitor can change with setPlatform. */
export function usePlatform(): { platform: Platform; setPlatform: (platform: Platform) => void } {
  const [platform, setPlatform] = useState<Platform>(readPlatform);
  return { platform, setPlatform };
}
