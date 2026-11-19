import { expect, it } from "vitest";
import {
  installCommands,
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
