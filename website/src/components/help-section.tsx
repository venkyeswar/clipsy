import type { ReactNode } from "react";
import { CommandLine } from "@/components/command-line";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { config } from "@/config";
import { NOSUDO_UNINSTALL_COMMAND, releasesUrl } from "@/lib/install-commands";

const shortcutText = config.shortcut.join("+");

function Question({ value, title, children }: { value: string; title: string; children: ReactNode }) {
  return (
    <AccordionItem value={value} className="border-border px-4">
      <AccordionTrigger className="min-h-12 items-center py-3 text-[0.9375rem] font-medium hover:no-underline hover:text-secondary-accent">
        {title}
      </AccordionTrigger>
      <AccordionContent className="pb-4 text-[0.9375rem] text-muted-foreground [&_p:not(:last-child)]:mb-2 [&_a]:text-secondary-accent [&_code]:font-mono [&_code]:text-[0.8125rem] [&_code]:text-foreground">
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

export function HelpSection() {
  return (
    <div>
      <h2 className="font-heading text-xl font-semibold tracking-[-0.02em] sm:text-2xl">Help</h2>
      <p className="mt-1 max-w-[60ch] text-muted-foreground">Answers to the questions people ask most.</p>

      <Accordion className="mt-4 overflow-hidden rounded-xl border border-border bg-card">
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
            The no-sudo install puts Clipsy in your home folder. GTK4 and libadwaita must already
            be installed, which they are on Ubuntu 24.04 GNOME.
          </p>
          <ol className="mb-4 list-decimal space-y-2 pl-5">
            <li>
              Download the <code>.tar.gz</code> file from the{" "}
              <a href={releasesUrl()} target="_blank" rel="noopener">
                releases page
              </a>
              .
            </li>
            <li>
              Extract it and go into the folder:
              <div className="mt-1.5 flex flex-col gap-1.5">
                <CommandLine text="tar -xzf clipsy-*-linux-x86_64.tar.gz" />
                <CommandLine text="cd clipsy-*-linux-x86_64" />
              </div>
            </li>
            <li>
              Run the installer:
              <div className="mt-1.5">
                <CommandLine text="./install.sh" />
              </div>
            </li>
          </ol>
          <p>To remove it later, run this from the same folder:</p>
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
            after="It asks whether to keep or delete your history."
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
      </Accordion>
    </div>
  );
}
