import { describe, expect, it } from "vitest";
import { WINDOWS_BUILD_COMMANDS, WINDOWS_COMING_SOON_NOTICE, windowsInstallTabs } from "./install-commands";
import type { Release } from "./releases";

const withWindows: Release = {
  version: "0.5.0",
  tag: "v0.5.0",
  date: "2026-11-19T12:00:00Z",
  prerelease: false,
  notesUrl: "https://example.test/notes",
  windowsInstaller: { name: "clipsy-0.5.0-setup.exe", url: "https://example.test/setup.exe", size: 1 },
  windowsPortable: { name: "clipsy-0.5.0-windows-x64.zip", url: "https://example.test/clipsy.zip", size: 1 },
  sha256sums: { name: "SHA256SUMS", url: "https://example.test/SHA256SUMS", size: 1 },
};

describe("windowsInstallTabs", () => {
  it("has the four Windows tabs in order", () => {
    expect(windowsInstallTabs(withWindows).map((tab) => tab.title)).toEqual([
      "Installer",
      "Portable (.zip)",
      "Verify download",
      "Build from source",
    ]);
  });

  it("shows the release file names and PowerShell checks when Windows is published", () => {
    const [installer, portable, verify] = windowsInstallTabs(withWindows);
    expect(installer.steps[0].detail).toContain("clipsy-0.5.0-setup.exe");
    expect(installer.notice).toBeUndefined();
    expect(portable.steps[0].detail).toContain("clipsy-0.5.0-windows-x64.zip");
    expect(verify.commands).toEqual([
      "Get-FileHash .\\clipsy-0.5.0-setup.exe -Algorithm SHA256",
      '(Get-Content .\\SHA256SUMS | Select-String "clipsy-0.5.0-setup.exe").Line',
    ]);
  });

  it("shows the coming-soon note and build steps on the download tabs when Windows is not published", () => {
    const tabs = windowsInstallTabs(undefined, true);
    for (const tab of tabs.slice(0, 3)) {
      expect(tab.notice).toBe(WINDOWS_COMING_SOON_NOTICE);
      expect(tab.commands).toEqual(WINDOWS_BUILD_COMMANDS);
      expect(tab.steps.map((step) => step.title)).toEqual(["Or build from source", "Open Clipsy"]);
    }
    expect(tabs[3].notice).toBeUndefined();
  });

  it("keeps build-from-source commands on its own tab", () => {
    const source = windowsInstallTabs(withWindows).at(-1);
    expect(source?.commands).toEqual(WINDOWS_BUILD_COMMANDS);
  });
});
