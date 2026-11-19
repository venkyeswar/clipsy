import { config } from "@/config";
import type { Release } from "@/lib/releases";

/** One numbered step: a short title, a one-line explanation and its commands (if any). */
export type InstallStep = {
  title: string;
  detail: string;
  commands: string[];
  /** The commands come from the release data, so show a placeholder while it loads. */
  needsRelease?: boolean;
};

/** A labelled line under the steps, such as how to update or uninstall. */
export type InstallExtra = {
  label: string;
  detail: string;
  commands: string[];
};

export type InstallTab = {
  id: "snap" | "deb" | "nosudo" | "source";
  title: string;
  /** "When to use this" line shown above the steps. */
  when: string;
  steps: InstallStep[];
  extras: InstallExtra[];
  /** Set when the release has nothing to install with this method. */
  notice?: string;
  /** Every command in the steps, in order. */
  commands: string[];
};

export const SNAP_APP_CENTER_URL = "snap://clipsy";
export const SNAP_REFRESH_COMMAND = "sudo snap refresh clipsy";
export const CHECKSUM_VERIFY_COMMAND = "sha256sum -c --ignore-missing SHA256SUMS";
export const WINDOWS_SHORTCUT_TEXT = "Ctrl+Shift+V";
export const WINDOWS_BUILD_COMMANDS = [
  "git clone https://github.com/venkyeswar/clipsy.git",
  "cd clipsy",
  "cargo build --release -p clipsy-windows",
  ".\\target\\release\\clipsy-windows.exe",
];
export const SOURCE_BUILD_COMMANDS = [
  "sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev libx11-dev",
  "cargo build --release -p clipsy-app",
  "./target/release/clipsy",
];

export function releasesUrl(): string {
  return `https://github.com/${config.repo}/releases`;
}

/** A file from the newest release. GitHub redirects this URL to the latest version. */
export function latestDownloadUrl(file: string): string {
  return `${releasesUrl()}/latest/download/${file}`;
}

export const NOSUDO_INSTALLER_URL = latestDownloadUrl("install.sh");
export const NOSUDO_UNINSTALLER_URL = latestDownloadUrl("uninstall.sh");
export const NOSUDO_DOWNLOAD_COMMAND = `curl -fsSL ${NOSUDO_INSTALLER_URL} -o install.sh`;
export const NOSUDO_RUN_COMMAND = "sh install.sh";
export const NOSUDO_START_COMMAND = "~/.local/bin/clipsy";
/** install.sh keeps a copy of the uninstaller next to its install manifest. */
export const NOSUDO_UNINSTALL_COMMAND = "sh ~/.local/share/clipsy/uninstall.sh";
export const NOSUDO_UNINSTALL_DOWNLOAD_COMMAND = `curl -fsSL ${NOSUDO_UNINSTALLER_URL} -o uninstall.sh && sh uninstall.sh`;

function withCommands(tab: Omit<InstallTab, "commands">): InstallTab {
  return { ...tab, commands: tab.steps.flatMap((step) => step.commands) };
}

function openStep(where: string): InstallStep {
  return { title: "Open Clipsy", detail: `${where} After that it starts when you log in.`, commands: [] };
}

/**
 * Beginner steps for Windows. Without release data the file-name commands stay empty
 * and the steps are marked needsRelease, like the .deb steps.
 */
export function windowsInstallSteps(release?: Release): InstallStep[] {
  const installer = release?.windowsInstaller?.name;
  const portable = release?.windowsPortable?.name;
  const sums = release?.sha256sums ?? release?.checksums;
  const version = release ? `Clipsy ${release.version}` : "Clipsy";
  const verifyCommands =
    installer && sums
      ? [
          `Get-FileHash .\\${installer} -Algorithm SHA256`,
          `(Get-Content .\\SHA256SUMS | Select-String "${installer}").Line`,
        ]
      : [];

  return [
    {
      title: "Install with the setup file (recommended)",
      detail: `Download ${installer ?? "the setup .exe"} from the latest release (${version}) and double-click it. Windows SmartScreen may say "Windows protected your PC" because the file is not signed yet. Click More info, then Run anyway.`,
      commands: [],
      needsRelease: true,
    },
    {
      title: "Or use the portable zip",
      detail: `Download ${portable ?? "the windows-x64 .zip"}, right-click it and choose Extract All. Open the new folder and double-click clipsy.exe. Nothing is installed, so you can run it from a USB stick.`,
      commands: [],
      needsRelease: true,
    },
    {
      title: "Check the download (optional)",
      detail: "Makes sure the file arrived complete and unchanged. Download SHA256SUMS from the same release into the same folder. The first command prints a hash, and the second prints the matching line from SHA256SUMS. The two hashes should match.",
      commands: verifyCommands,
      needsRelease: true,
    },
    {
      title: "Or build from source",
      detail: "For developers. You need Rust with the MSVC toolchain (rustup.rs) and the Visual Studio Build Tools C++ workload. Takes a few minutes the first time.",
      commands: [...WINDOWS_BUILD_COMMANDS],
    },
    {
      title: "Open Clipsy",
      detail: `Clipsy runs in the system tray. Press ${WINDOWS_SHORTCUT_TEXT} to open your clipboard history.`,
      commands: [],
    },
  ];
}

export const WINDOWS_COMING_SOON_NOTICE =
  "The Windows download is coming soon. Until then, you can build Clipsy yourself on Windows with the steps below.";

export const SMARTSCREEN_NOTE =
  'Windows may say "unknown publisher" because the app is not code-signed yet. Click More info, then Run anyway.';

export type WindowsInstallTab = {
  id: "windows-installer" | "windows-portable" | "windows-verify" | "windows-source";
  title: string;
  /** "When to use this" line shown above the steps. */
  when: string;
  steps: InstallStep[];
  /** Set while no Windows files are published: the tab shows this note above the build steps. */
  notice?: string;
  /** Every command in the steps, in order. */
  commands: string[];
};

/**
 * The Windows tabs. While no Windows release exists (comingSoon), the download tabs show a
 * note and the build-from-source steps instead, so the page still gives a way to install.
 */
export function windowsInstallTabs(release?: Release, comingSoon = false): WindowsInstallTab[] {
  const [installer, portable, verify, build, open] = windowsInstallSteps(release);
  const buildSteps = [build, open];
  const withNotice = (steps: InstallStep[]) =>
    comingSoon ? { steps: buildSteps, notice: WINDOWS_COMING_SOON_NOTICE } : { steps };

  const tabs: Omit<WindowsInstallTab, "commands">[] = [
    {
      id: "windows-installer",
      title: "Installer",
      when: "Recommended. Double-click the setup file and Clipsy installs for your user account.",
      ...withNotice([installer]),
    },
    {
      id: "windows-portable",
      title: "Portable (.zip)",
      when: "No install. Run Clipsy from a folder or a USB stick.",
      ...withNotice([portable]),
    },
    {
      id: "windows-verify",
      title: "Verify download",
      when: "Checks that the file you downloaded arrived complete and unchanged.",
      ...withNotice([verify]),
    },
    {
      id: "windows-source",
      title: "Build from source",
      when: "For developers. You need Rust with the MSVC toolchain (rustup.rs).",
      steps: buildSteps,
    },
  ];

  return tabs.map((tab) => ({ ...tab, commands: tab.steps.flatMap((step) => step.commands) }));
}

export function installCommands(
  channel: "stable" | "edge" | null,
  release?: Release,
): InstallTab[] {
  const tabs: InstallTab[] = [];

  if (channel !== null) {
    tabs.push(
      withCommands({
        id: "snap",
        title: "Ubuntu App Center / Snap",
        when: "The easiest way on Ubuntu. Updates come automatically.",
        steps: [
          {
            title: "Install Clipsy",
            detail: "Run this, or open Clipsy in the App Center and click Install.",
            commands: [channel === "edge" ? "sudo snap install clipsy --edge" : "sudo snap install clipsy"],
          },
          openStep("Find Clipsy in your app menu and open it once."),
        ],
        extras: [
          { label: "Update now", detail: "Snap updates by itself. To update right away:", commands: [SNAP_REFRESH_COMMAND] },
        ],
      }),
    );
  }

  // Without release data the deb steps keep their titles but have no commands: the page
  // shows placeholders while loading, and the notice once it is known nothing is published.
  const deb = release?.deb;
  const checksums = release?.checksums;
  const loadingCheck = !release;
  tabs.push(
    withCommands({
      id: "deb",
      title: ".deb package",
      when: "For Ubuntu and Debian when you can use sudo.",
      notice: deb
        ? undefined
        : `No .deb package is published for the latest release yet. Find the packages on the GitHub releases page: ${releasesUrl()}`,
      steps: [
        {
          title: "Download the package",
          detail: release ? `Clipsy ${release.version} from the latest release.` : "From the latest release.",
          commands: deb ? [`wget ${deb.url}`] : [],
          needsRelease: true,
        },
        ...(checksums || loadingCheck
          ? [
              {
                title: "Check the download (optional)",
                detail: "Makes sure the file arrived complete and unchanged. It should say OK.",
                commands: checksums && deb ? [`wget ${checksums.url}`, CHECKSUM_VERIFY_COMMAND] : [],
                needsRelease: true,
              },
            ]
          : []),
        {
          title: "Install it",
          detail: "apt also installs anything Clipsy needs.",
          commands: deb ? [`sudo apt install ./${deb.name}`] : [],
          needsRelease: true,
        },
        openStep("Find Clipsy in your app menu and open it once."),
      ],
      extras: [{ label: "Update", detail: "Download the new .deb and install it the same way.", commands: [] }],
    }),
  );

  // Does not depend on release data: the installer URL always points at the latest release.
  tabs.push(
    withCommands({
      id: "nosudo",
      title: "No sudo",
      when: "For computers where you can't use sudo. Clipsy goes into your home folder.",
      steps: [
        {
          title: "Download the installer",
          detail: "A small script from the latest release. You can open it and read it first.",
          commands: [NOSUDO_DOWNLOAD_COMMAND],
        },
        {
          title: "Run it",
          detail:
            "It downloads Clipsy, checks it and installs it into ~/.local. It needs GTK4 and libadwaita, which Ubuntu 24.04 GNOME already has.",
          commands: [NOSUDO_RUN_COMMAND],
        },
        {
          title: "Start Clipsy",
          detail: "Or find Clipsy in your app menu. After you log out and back in, typing clipsy works too.",
          commands: [NOSUDO_START_COMMAND],
        },
      ],
      extras: [
        { label: "Update", detail: "Run steps 1 and 2 again.", commands: [] },
        { label: "Uninstall", detail: "Asks whether to keep your history.", commands: [NOSUDO_UNINSTALL_COMMAND] },
      ],
    }),
  );

  tabs.push(
    withCommands({
      id: "source",
      title: "Build from source",
      when: "For developers. You need Rust installed (get it from rustup.rs).",
      steps: [
        {
          title: "Install the build tools",
          detail: "Only this step needs sudo.",
          commands: [SOURCE_BUILD_COMMANDS[0]],
        },
        {
          title: "Get the code and build it",
          detail: "Takes a few minutes the first time.",
          commands: [`git clone https://github.com/${config.repo}.git`, "cd clipsy", SOURCE_BUILD_COMMANDS[1]],
        },
        {
          title: "Run Clipsy",
          detail: "Starts Clipsy from the folder you built it in.",
          commands: [SOURCE_BUILD_COMMANDS[2]],
        },
      ],
      extras: [{ label: "Update", detail: "Run git pull in the folder, then build again.", commands: [] }],
    }),
  );

  return tabs;
}
