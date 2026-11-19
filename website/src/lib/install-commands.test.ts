import { describe, expect, it } from "vitest";
import {
  installCommands,
  windowsInstallSteps,
  latestDownloadUrl,
  NOSUDO_INSTALLER_URL,
  NOSUDO_UNINSTALL_COMMAND,
  NOSUDO_UNINSTALLER_URL,
} from "./install-commands";
import type { Release } from "./releases";

const release: Release = {
  version: "0.4.0",
  tag: "v0.4.0",
  date: "2026-11-18T12:00:00Z",
  prerelease: false,
  notesUrl: "https://example.test/notes",
  deb: {
    name: "clipsy_0.4.0_amd64.deb",
    url: "https://example.test/clipsy_0.4.0_amd64.deb",
    size: 1,
  },
  tarball: {
    name: "clipsy-0.4.0-linux-x86_64.tar.gz",
    url: "https://example.test/clipsy-0.4.0-linux-x86_64.tar.gz",
    size: 1,
  },
  checksums: { name: "SHA256SUMS", url: "https://example.test/SHA256SUMS", size: 1 },
};

const tab = (id: string, list = installCommands("edge", release)) => {
  const found = list.find((item) => item.id === id);
  if (!found) throw new Error(`missing tab ${id}`);
  return found;
};

const allText = (id: string, list = installCommands("edge", release)) => JSON.stringify(tab(id, list));

it("snap_tab_hidden_when_channel_null", () => {
  const ids = installCommands(null, release).map((item) => item.id);
  expect(ids).not.toContain("snap");
  expect(ids).toEqual(["deb", "nosudo", "source"]);
});

it("snap_edge_uses_edge_flag", () => {
  expect(tab("snap").commands).toEqual(["sudo snap install clipsy --edge"]);
});

it("snap_stable_has_no_flag", () => {
  expect(tab("snap", installCommands("stable", release)).commands).toEqual([
    "sudo snap install clipsy",
  ]);
});

it("snap_tab_explains_how_to_update", () => {
  expect(tab("snap").extras.flatMap((extra) => extra.commands)).toEqual(["sudo snap refresh clipsy"]);
});

it("deb_tab_falls_back_without_release", () => {
  const deb = tab("deb", installCommands("edge", undefined));
  expect(deb.commands).toEqual([]);
  expect(deb.notice).toContain("https://github.com/venkyeswar/clipsy/releases");
});

it("deb_tab_uses_release_asset_url_and_name_in_numbered_steps", () => {
  const deb = tab("deb");
  expect(deb.notice).toBeUndefined();
  expect(deb.steps.map((step) => step.title)).toEqual([
    "Download the package",
    "Check the download (optional)",
    "Install it",
    "Open Clipsy",
  ]);
  expect(deb.commands).toEqual([
    "wget https://example.test/clipsy_0.4.0_amd64.deb",
    "wget https://example.test/SHA256SUMS",
    "sha256sum -c --ignore-missing SHA256SUMS",
    "sudo apt install ./clipsy_0.4.0_amd64.deb",
  ]);
});

it("deb_tab_skips_checksum_step_when_release_has_no_sums", () => {
  const deb = tab("deb", installCommands("edge", { ...release, checksums: undefined }));
  expect(deb.steps.map((step) => step.title)).not.toContain("Check the download (optional)");
  expect(deb.commands).not.toContain("sha256sum -c --ignore-missing SHA256SUMS");
});

it("nosudo_tab_uses_latest_download_installer_url_without_release_data", () => {
  const url = "https://github.com/venkyeswar/clipsy/releases/latest/download/install.sh";
  expect(NOSUDO_INSTALLER_URL).toBe(url);
  expect(latestDownloadUrl("uninstall.sh")).toBe(NOSUDO_UNINSTALLER_URL);
  for (const current of [undefined, release, { ...release, tarball: undefined }]) {
    const nosudo = tab("nosudo", installCommands("edge", current));
    expect(nosudo.notice).toBeUndefined();
    expect(nosudo.steps.every((step) => !step.needsRelease)).toBe(true);
    expect(nosudo.commands).toEqual([`curl -fsSL ${url} -o install.sh`, "sh install.sh", "~/.local/bin/clipsy"]);
    expect(nosudo.extras.flatMap((extra) => extra.commands)).toEqual([NOSUDO_UNINSTALL_COMMAND]);
  }
  expect(NOSUDO_UNINSTALL_COMMAND).toBe("sh ~/.local/share/clipsy/uninstall.sh");
});

it("nosudo_tab_has_no_tarball_references", () => {
  for (const current of [undefined, release]) {
    const text = allText("nosudo", installCommands("edge", current));
    expect(text).not.toMatch(/tar\.gz|tarball|tar -x|\.\/install\.sh/i);
  }
});

it("source_tab_clones_builds_and_runs", () => {
  expect(tab("source").commands).toEqual([
    "sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev libx11-dev",
    "git clone https://github.com/venkyeswar/clipsy.git",
    "cd clipsy",
    "cargo build --release -p clipsy-app",
    "./target/release/clipsy",
  ]);
  expect(tab("source").extras.map((extra) => extra.label)).toEqual(["Update"]);
});

const windowsRelease: Release = {
  ...release,
  version: "0.5.0",
  tag: "v0.5.0",
  windowsInstaller: {
    name: "clipsy-0.5.0-setup.exe",
    url: "https://example.test/clipsy-0.5.0-setup.exe",
    size: 1,
  },
  windowsPortable: {
    name: "clipsy-0.5.0-windows-x64.zip",
    url: "https://example.test/clipsy-0.5.0-windows-x64.zip",
    size: 1,
  },
  sha256sums: { name: "SHA256SUMS", url: "https://example.test/SHA256SUMS", size: 1 },
};

const windowsStep = (title: string, release?: Release) => {
  const found = windowsInstallSteps(release).find((step) => step.title === title);
  if (!found) throw new Error(`missing step ${title}`);
  return found;
};

describe("windowsInstallSteps", () => {
  it("lists the beginner steps in order", () => {
    expect(windowsInstallSteps(windowsRelease).map((step) => step.title)).toEqual([
      "Install with the setup file (recommended)",
      "Or use the portable zip",
      "Check the download (optional)",
      "Or build from source",
      "Open Clipsy",
    ]);
  });

  it("gives exact PowerShell verification commands for the fixture version", () => {
    const verify = windowsStep("Check the download (optional)", windowsRelease);
    expect(verify.commands).toEqual([
      "Get-FileHash .\\clipsy-0.5.0-setup.exe -Algorithm SHA256",
      '(Get-Content .\\SHA256SUMS | Select-String "clipsy-0.5.0-setup.exe").Line',
    ]);
    expect(verify.detail).toContain("SHA256SUMS");
  });

  it("gives exact build-from-source commands", () => {
    const source = windowsStep("Or build from source", windowsRelease);
    expect(source.commands).toEqual([
      "git clone https://github.com/venkyeswar/clipsy.git",
      "cd clipsy",
      "cargo build --release -p clipsy-windows",
      ".\\target\\release\\clipsy-windows.exe",
    ]);
    expect(source.detail).toContain("Visual Studio Build Tools");
    expect(source.needsRelease).toBeFalsy();
  });

  it("explains SmartScreen for the installer and the portable zip steps", () => {
    const installer = windowsStep("Install with the setup file (recommended)", windowsRelease);
    expect(installer.detail).toContain("clipsy-0.5.0-setup.exe");
    expect(installer.detail).toContain("More info");
    expect(installer.detail).toContain("Run anyway");
    expect(installer.commands).toEqual([]);
    const portable = windowsStep("Or use the portable zip", windowsRelease);
    expect(portable.detail).toContain("clipsy-0.5.0-windows-x64.zip");
    expect(portable.detail).toContain("Extract All");
    expect(portable.detail).toContain("clipsy.exe");
    expect(portable.commands).toEqual([]);
  });

  it("uses Ctrl+Shift+V in the first-use step", () => {
    const open = windowsStep("Open Clipsy", windowsRelease);
    expect(open.detail).toContain("Ctrl+Shift+V");
    expect(open.detail).not.toContain("Super");
  });

  it("marks release-dependent steps while the release is loading", () => {
    const steps = windowsInstallSteps(undefined);
    expect(steps.filter((step) => step.needsRelease).map((step) => step.title)).toEqual([
      "Install with the setup file (recommended)",
      "Or use the portable zip",
      "Check the download (optional)",
    ]);
    expect(windowsStep("Check the download (optional)").commands).toEqual([]);
    expect(windowsStep("Or build from source").commands).toHaveLength(4);
  });
});
