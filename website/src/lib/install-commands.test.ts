import { expect, it } from "vitest";
import { installCommands } from "./install-commands";
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
};

const tab = (id: string, list = installCommands("edge", release)) => {
  const found = list.find((item) => item.id === id);
  if (!found) throw new Error(`missing tab ${id}`);
  return found;
};

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

it("deb_tab_falls_back_without_release", () => {
  const deb = tab("deb", installCommands("edge", undefined));
  expect(deb.commands).toEqual([]);
  expect(deb.note).toContain("https://github.com/venkyeswar/clipsy/releases");
});

it("nosudo_uses_release_version_in_filename", () => {
  expect(tab("nosudo", installCommands("edge", { ...release, tarball: undefined })).commands).toContain(
    "tar -xzf clipsy-0.4.0-linux-x86_64.tar.gz",
  );
  expect(tab("nosudo", installCommands("edge", undefined)).commands).toContain(
    "tar -xzf clipsy-<version>-linux-x86_64.tar.gz",
  );
});

it("deb_tab_uses_release_asset_url_and_name", () => {
  expect(tab("deb").commands).toEqual([
    "wget https://example.test/clipsy_0.4.0_amd64.deb",
    "sudo apt install ./clipsy_0.4.0_amd64.deb",
  ]);
});

it("nosudo_extracted_folder_name_matches_tarball", () => {
  const nosudo = tab("nosudo");
  expect(nosudo.commands).toEqual([
    "wget https://example.test/clipsy-0.4.0-linux-x86_64.tar.gz",
    "tar -xzf clipsy-0.4.0-linux-x86_64.tar.gz",
    "cd clipsy-0.4.0-linux-x86_64",
    "./install.sh",
  ]);
});
