import type { ReactNode } from "react";
import { CommandLine } from "@/components/command-line";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { shortcutFor } from "@/config";
import { useSelectedPlatform, type SelectedPlatform } from "@/lib/platform-context";
import {
  NOSUDO_DOWNLOAD_COMMAND,
  NOSUDO_RUN_COMMAND,
  NOSUDO_START_COMMAND,
  NOSUDO_UNINSTALL_COMMAND,
} from "@/lib/install-commands";

function Question({ value, title, children }: { value: string; title: string; children: ReactNode }) {
  return (
    <AccordionItem value={value} className="border-border px-3">
      <AccordionTrigger className="-mx-2 min-h-10 items-center rounded-md px-2 py-2 text-sm font-medium transition-colors duration-150 hover:bg-muted/60 hover:no-underline">
        {title}
      </AccordionTrigger>
      <AccordionContent className="pb-3 text-sm text-muted-foreground [&_p:not(:last-child)]:mb-2 [&_a]:text-secondary-accent [&_code]:font-mono [&_code]:text-[0.8125rem] [&_code]:text-foreground">
        {children}
      </AccordionContent>
    </AccordionItem>
  );
}

/** A label above a command, used where an answer differs by install type. */
function Labelled({ label, command, after }: { label: string; command: string; after?: string }) {
  return (
    <div className="mt-3 first:mt-0">
      <p className="mb-1 text-sm font-medium text-foreground">{label}</p>
      <CommandLine text={command} />
      {after && <p className="mt-1 text-sm">{after}</p>}
    </div>
  );
}

/** Questions for Linux: GNOME, Wayland, the .deb, Snap and the no-sudo installer. */
function LinuxQuestions({ shortcutText }: { shortcutText: string }) {
  return (
    <>
      <Question value="super-v" title="Super+V does nothing">
        <p>
          GNOME already uses Super+V to show its notification list. Clipsy&apos;s default
          shortcut is {shortcutText}. You can change it in Settings or on the welcome screen.
        </p>
      </Question>

      <Question value="wayland" title="Does it work on Wayland?">
        <p>
          Yes, on GNOME with Wayland. Clipsy installs and turns on a small GNOME Shell extension
          by itself where it can. Log out and back in once so the extension starts. If the
          automatic setup fails, Clipsy shows a Terminal command to run instead.
        </p>
        <p>Other Wayland desktops are not supported yet.</p>
      </Question>

      <Question value="sudo" title="I can't use sudo">
        <p>
          Use the no-sudo installer. It puts Clipsy in your home folder (<code>~/.local</code>).
          GTK4 and libadwaita must already be installed, which they are on Ubuntu 24.04 GNOME. If
          they are missing, the installer tells you the one command to ask an admin to run.
        </p>
        <ol className="mb-4 list-decimal space-y-2 pl-5">
          <li>
            Download the installer from the latest release:
            <div className="mt-1.5">
              <CommandLine text={NOSUDO_DOWNLOAD_COMMAND} />
            </div>
          </li>
          <li>
            Run it. It downloads Clipsy, checks it and installs it:
            <div className="mt-1.5">
              <CommandLine text={NOSUDO_RUN_COMMAND} />
            </div>
          </li>
          <li>
            Start Clipsy from your app menu, or with <code>{NOSUDO_START_COMMAND}</code>.
          </li>
        </ol>
        <p>To update later, run the same two commands again. To remove it:</p>
        <div className="mt-1.5">
          <CommandLine text={NOSUDO_UNINSTALL_COMMAND} />
        </div>
      </Question>

      <Question value="uninstall" title="How do I uninstall?">
        <p>Run the command that matches how you installed Clipsy.</p>
        <Labelled
          label="Snap"
          command="sudo snap remove clipsy"
          after="This also deletes your history. Snap saves an automatic snapshot of it first."
        />
        <Labelled
          label=".deb package"
          command="sudo apt remove clipsy"
          after="Your history stays in ~/.local/share/clipsy."
        />
        <Labelled
          label="No sudo install"
          command={NOSUDO_UNINSTALL_COMMAND}
          after="It asks whether to keep or delete your history in ~/.local/share/clipsy."
        />
      </Question>

      <Question value="data" title="Where is my data stored?">
        <p>
          On your computer, and nothing is sent anywhere. The folder holds the history database
          and the image files. Where it is depends on how you installed Clipsy:
        </p>
        <ul className="mb-3 list-disc space-y-1 pl-5">
          <li>
            .deb, no-sudo and source installs: <code>~/.local/share/clipsy</code>
          </li>
          <li>
            Snap: inside the snap&apos;s own folder, under <code>~/snap/clipsy/</code>
          </li>
        </ul>
        <p className="mb-1.5!">
          To delete the history of a .deb, no-sudo or source install, remove that folder:
        </p>
        <CommandLine text="rm -r ~/.local/share/clipsy" />
        <p className="mt-2">
          For the Snap, <code>sudo snap remove clipsy</code> deletes it.
        </p>
      </Question>
    </>
  );
}

/** Questions for Windows: the tray app, Ctrl+Shift+V, the installer and the portable zip. */
function WindowsQuestions({ shortcutText }: { shortcutText: string }) {
  return (
    <>
      <Question value="win-shortcut" title="The shortcut does nothing">
        <p>
          Clipsy&apos;s default shortcut is {shortcutText}. Windows reserves Win+V for its own
          clipboard history, so Clipsy does not use it. Check that Clipsy is running: its icon
          is in the system tray, near the clock.
        </p>
        <p>
          To look at what Clipsy is doing, close it from the tray, open PowerShell, set the log
          level and start Clipsy from that window:
        </p>
        <CommandLine text={'$env:CLIPSY_LOG = "debug"'} />
        <p>Then start clipsy.exe in the same window. The log messages appear there.</p>
      </Question>

      <Question value="win-smartscreen" title="Windows says 'unknown publisher'">
        <p>
          Clipsy is not code-signed yet, so Windows SmartScreen warns about it. Click{" "}
          <strong>More info</strong>, then <strong>Run anyway</strong>. The release lists SHA256
          checksums, so you can check that your file is complete and unchanged (see the Install
          section).
        </p>
      </Question>

      <Question value="win-portable" title="Can I use it without installing?">
        <p>
          Yes. Download the portable <code>.zip</code>, right-click it, choose Extract All, and
          double-click <code>clipsy.exe</code> in the folder. No admin rights are needed, so you can
          also run it from a USB stick.
        </p>
      </Question>

      <Question value="win-uninstall" title="How do I uninstall?">
        <p>
          <span className="font-medium text-foreground">Installer:</span> open Settings, go to Apps,
          then Installed apps, choose Clipsy and select Uninstall.
        </p>
        <p>
          <span className="font-medium text-foreground">Portable (.zip):</span> quit Clipsy from its
          tray icon, then delete the folder. Nothing else was installed.
        </p>
      </Question>

      <Question value="win-data" title="Where is my data stored?">
        <p>
          On your computer, and nothing is sent anywhere. Clipsy keeps two folders:
        </p>
        <ul className="mb-3 list-disc space-y-1 pl-5">
          <li>
            History database and images: <code>%LOCALAPPDATA%\clipsy\data</code>
          </li>
          <li>
            Settings: <code>%APPDATA%\clipsy\config\config.toml</code>
          </li>
        </ul>
        <p>To delete your history, quit Clipsy and delete the first folder.</p>
      </Question>
    </>
  );
}

/** The question set for the selected platform. The accordion remounts when it changes. */
function questionsFor(platform: SelectedPlatform, shortcutText: string) {
  return platform === "windows" ? (
    <WindowsQuestions shortcutText={shortcutText} />
  ) : (
    <LinuxQuestions shortcutText={shortcutText} />
  );
}

export function HelpSection() {
  const { platform } = useSelectedPlatform();
  const shortcutText = shortcutFor(platform).join("+");

  return (
    <div>
      <h2 className="font-heading text-2xl font-semibold tracking-tight sm:text-3xl">Help</h2>
      <p className="mt-1 max-w-[60ch] text-muted-foreground">Answers to the questions people ask most.</p>

      <Accordion key={platform} className="mt-3 overflow-hidden rounded-xl border border-border/70 bg-background/80 backdrop-blur-sm">
        {questionsFor(platform, shortcutText)}
      </Accordion>
    </div>
  );
}
