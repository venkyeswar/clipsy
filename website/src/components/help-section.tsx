import { CopyButton } from "@/components/copy-button";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { config } from "@/config";
import { NOSUDO_UNINSTALL_COMMAND, releasesUrl } from "@/lib/install-commands";

/** A single copyable command, styled like the install section. */
function Command({ text }: { text: string }) {
  return (
    <div className="flex items-center gap-2 rounded-lg border border-border bg-muted p-2 pl-3">
      <pre
        data-copy-source
        tabIndex={0}
        className="min-w-0 flex-1 overflow-x-auto rounded-sm font-mono text-sm whitespace-pre text-foreground outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        <code>{text}</code>
      </pre>
      <CopyButton text={text} />
    </div>
  );
}

const shortcutText = config.shortcut.join("+");

export function HelpSection() {
  return (
    <div>
      <h2 className="font-heading text-2xl font-semibold tracking-tight">Help</h2>

      <Accordion className="mt-6">
        <AccordionItem value="super-v">
          <AccordionTrigger className="min-h-11 items-center py-3 text-base">
            Super+V does nothing
          </AccordionTrigger>
          <AccordionContent>
            <p>
              GNOME already uses Super+V to show its notification list. Clipsy&apos;s default
              shortcut is {shortcutText}. You can change it in Settings or on the welcome screen.
            </p>
          </AccordionContent>
        </AccordionItem>

        <AccordionItem value="wayland">
          <AccordionTrigger className="min-h-11 items-center py-3 text-base">
            Does it work on Wayland?
          </AccordionTrigger>
          <AccordionContent>
            <p>
              Yes, on GNOME with Wayland. Clipsy installs a small GNOME Shell extension that
              watches the clipboard and handles the shortcut. On first run, Clipsy offers to set
              it up.
            </p>
          </AccordionContent>
        </AccordionItem>

        <AccordionItem value="sudo">
          <AccordionTrigger className="min-h-11 items-center py-3 text-base">
            I can&apos;t use sudo
          </AccordionTrigger>
          <AccordionContent>
            <p>
              The no-sudo install puts Clipsy in your home folder. GTK4 and libadwaita must already
              be installed, which they are on Ubuntu 24.04 GNOME.
            </p>
            <ol className="mb-4 list-decimal space-y-2 pl-5">
              <li>
                Download the <code className="font-mono">.tar.gz</code> file from the{" "}
                <a href={releasesUrl()} target="_blank" rel="noopener">
                  releases page
                </a>
                .
              </li>
              <li>
                Extract it and go into the folder:
                <div className="mt-2 flex flex-col gap-2">
                  <Command text="tar -xzf clipsy-*-linux-x86_64.tar.gz" />
                  <Command text="cd clipsy-*-linux-x86_64" />
                </div>
              </li>
              <li>
                Run the installer:
                <div className="mt-2">
                  <Command text="./install.sh" />
                </div>
              </li>
            </ol>
            <p>To remove it later, run this from the same folder:</p>
            <div className="mt-2">
              <Command text={NOSUDO_UNINSTALL_COMMAND} />
            </div>
          </AccordionContent>
        </AccordionItem>

        <AccordionItem value="uninstall">
          <AccordionTrigger className="min-h-11 items-center py-3 text-base">
            How do I uninstall?
          </AccordionTrigger>
          <AccordionContent>
            <p>Run the command that matches how you installed Clipsy.</p>
            <p className="mt-2 font-medium text-foreground">Snap</p>
            <div className="mt-1">
              <Command text="sudo snap remove clipsy" />
            </div>
            <p className="mt-3 font-medium text-foreground">.deb package</p>
            <div className="mt-1">
              <Command text="sudo apt remove clipsy" />
            </div>
            <p className="mt-3 font-medium text-foreground">No sudo install</p>
            <div className="mt-1">
              <Command text={NOSUDO_UNINSTALL_COMMAND} />
            </div>
            <p className="mt-3">
              Removing the app keeps your history. To delete it too, see the next question.
            </p>
          </AccordionContent>
        </AccordionItem>

        <AccordionItem value="data">
          <AccordionTrigger className="min-h-11 items-center py-3 text-base">
            Where is my data stored?
          </AccordionTrigger>
          <AccordionContent>
            <p>
              In <code className="font-mono">~/.local/share/clipsy</code> on your computer. That
              folder holds the history database and the image files. Nothing is sent anywhere.
            </p>
            <p className="mt-2">To delete everything Clipsy has stored, remove that folder:</p>
            <div className="mt-2">
              <Command text="rm -r ~/.local/share/clipsy" />
            </div>
          </AccordionContent>
        </AccordionItem>
      </Accordion>
    </div>
  );
}
