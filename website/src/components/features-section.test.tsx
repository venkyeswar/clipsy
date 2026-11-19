import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { FeaturesSection } from "./features-section";

const features = [
  "Remembers text, images and files",
  "Search in a keystroke",
  "Pin what you reuse",
  "Clear one tab at a time",
  "Private by default",
  "Works where you work",
];

afterEach(cleanup);

it("lists_all_six_features", () => {
  render(<FeaturesSection />);
  for (const feature of features) {
    expect(screen.getByText(feature, { selector: "dt" })).toBeInTheDocument();
  }
});
