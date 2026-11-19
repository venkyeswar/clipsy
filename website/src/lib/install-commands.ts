import { config } from "@/config";
import type { Release } from "@/lib/releases";

export type InstallTab = {
  id: "snap" | "deb" | "nosudo" | "source";
  title: string;
  note: string;
  commands: string[];
};

export const NOSUDO_UNINSTALL_COMMAND = "./uninstall.sh";
export const SNAP_APP_CENTER_URL = "snap://clipsy";
export const SOURCE_BUILD_COMMANDS = [
  "sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev libx11-dev",
  "cargo build --release -p clipsy-app",
  "./target/release/clipsy",
];

export function releasesUrl(): string {
  return `https://github.com/${config.repo}/releases`;
}

const TARBALL_SUFFIX = ".tar.gz";

export function installCommands(
  channel: "stable" | "edge" | null,
  release?: Release,
): InstallTab[] {
  const tabs: InstallTab[] = [];

  if (channel !== null) {
    tabs.push({
      id: "snap",
      title: "Ubuntu App Center / Snap",
      note: "Installs from the Snap Store, and the App Center keeps it up to date.",
      commands: [channel === "edge" ? "sudo snap install clipsy --edge" : "sudo snap install clipsy"],
    });
  }

  const deb = release?.deb;
  tabs.push({
    id: "deb",
    title: ".deb package",
    note: deb
      ? "Downloads the .deb from the latest release and installs it with apt. Needs sudo."
      : `No .deb package is published for the latest release yet. Find the packages on the GitHub releases page: ${releasesUrl()}`,
    commands: deb ? [`wget ${deb.url}`, `sudo apt install ./${deb.name}`] : [],
  });

  const version = release?.version ?? "<version>";
  const tarball = release?.tarball;
  const tarballName = tarball?.name ?? `clipsy-${version}-linux-x86_64${TARBALL_SUFFIX}`;
  const folder = tarballName.endsWith(TARBALL_SUFFIX)
    ? tarballName.slice(0, -TARBALL_SUFFIX.length)
    : tarballName;
  const download = tarball
    ? `wget ${tarball.url}`
    : `wget ${releasesUrl()}/download/v${version}/${tarballName}`;
  tabs.push({
    id: "nosudo",
    title: "No sudo",
    note: `Installs into your home folder, so it needs no sudo. GTK4 and libadwaita must already be installed, which they are on Ubuntu 24.04 GNOME. To remove it later, run ${NOSUDO_UNINSTALL_COMMAND} from the same folder.`,
    commands: [download, `tar -xzf ${tarballName}`, `cd ${folder}`, "./install.sh"],
  });

  tabs.push({
    id: "source",
    title: "Build from source",
    note: "Only the first line needs sudo. You also need Rust installed (get it from rustup.rs).",
    commands: [...SOURCE_BUILD_COMMANDS],
  });

  return tabs;
}
