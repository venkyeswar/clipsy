import { Separator } from "@/components/ui/separator";

const features = [
  {
    name: "Remembers text, images and files",
    description: "Everything you copy stays in your history, so you can paste it again later.",
  },
  {
    name: "Search in a keystroke",
    description: "Type to filter your history as you go.",
  },
  {
    name: "Pin what you reuse",
    description: "Pinned items stay at the top and are kept until you remove them.",
  },
  {
    name: "Clear one tab at a time",
    description:
      "Clear text, images or files separately. Pinned items are kept unless you choose to remove them too.",
  },
  {
    name: "Private by default",
    description:
      "Copies from password managers are skipped, and your history stays on your computer.",
  },
  {
    name: "Works where you work",
    description: "Runs on X11 and on GNOME with Wayland.",
  },
];

export function FeaturesSection() {
  return (
    <div className="grid gap-8 md:grid-cols-[minmax(0,2fr)_minmax(0,3fr)] md:gap-12">
      <div>
        <h2 className="font-heading text-2xl font-semibold tracking-tight">What Clipsy does</h2>
        <p className="mt-2 max-w-[40ch] text-muted-foreground">
          Clipsy watches what you copy, keeps it in order, and stays out of the way until you
          need it again.
        </p>
      </div>

      <dl>
        {features.map((feature, index) => (
          <div key={feature.name}>
            {index > 0 && <Separator className="my-4" />}
            <dt className="font-medium text-foreground">{feature.name}</dt>
            <dd className="mt-1 text-muted-foreground">{feature.description}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}
