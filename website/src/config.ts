import type { Platform } from "./lib/platform";

// Default global toggle shortcut from crates/clipsy-core/src/config.rs: <Super><Shift>v
const LINUX_SHORTCUT = ["Super", "Shift", "V"];
// Windows reserves Win+V, so the Windows app defaults to Ctrl+Shift+V.
const WINDOWS_SHORTCUT = ["Ctrl", "Shift", "V"];

export const config = {
  repo: "venkyeswar/clipsy",
  snap: { channel: "edge" as "stable" | "edge" | null },
  shortcut: LINUX_SHORTCUT as string[],
};

/** The default open-Clipsy shortcut as key labels for the visitor's platform. */
export function shortcutFor(platform: Platform): string[] {
  return platform === "windows" ? [...WINDOWS_SHORTCUT] : [...LINUX_SHORTCUT];
}
