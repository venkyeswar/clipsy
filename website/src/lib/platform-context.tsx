import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";
import { usePlatform, type Platform } from "./platform";

/** The platform the site shows downloads and steps for. Mac and other visitors get linux. */
export type SelectedPlatform = "linux" | "windows";

export type SelectedPlatformState = {
  platform: SelectedPlatform;
  setPlatform: (platform: SelectedPlatform) => void;
};

const STORAGE_KEY = "clipsy-platform";

const PlatformContext = createContext<SelectedPlatformState | undefined>(undefined);

/** Windows stays Windows; every other detected platform falls back to linux. */
export function toSelectedPlatform(platform: Platform): SelectedPlatform {
  return platform === "windows" ? "windows" : "linux";
}

function readStoredPlatform(): SelectedPlatform | null {
  try {
    const value = window.localStorage.getItem(STORAGE_KEY);
    if (value === "linux" || value === "windows") return value;
  } catch {
    // Storage blocked (private mode, disabled site data): use the detected platform.
  }
  return null;
}

function storePlatform(value: SelectedPlatform) {
  try {
    window.localStorage.setItem(STORAGE_KEY, value);
  } catch {
    // Not remembered; the choice still applies for this visit.
  }
}

/**
 * Shares one platform choice between the hero, header and install/downloads sections.
 * A choice the visitor made earlier (stored in localStorage) wins over detection.
 */
export function PlatformProvider({ children }: { children: ReactNode }) {
  const { platform: detected } = usePlatform();
  const [stored, setStored] = useState<SelectedPlatform | null>(readStoredPlatform);
  const platform = stored ?? toSelectedPlatform(detected);

  const setPlatform = useCallback((next: SelectedPlatform) => {
    storePlatform(next);
    setStored(next);
  }, []);

  const value = useMemo(() => ({ platform, setPlatform }), [platform, setPlatform]);
  return <PlatformContext.Provider value={value}>{children}</PlatformContext.Provider>;
}

export function useSelectedPlatform(): SelectedPlatformState {
  const context = useContext(PlatformContext);
  if (!context) {
    throw new Error("useSelectedPlatform must be used inside <PlatformProvider>");
  }
  return context;
}
