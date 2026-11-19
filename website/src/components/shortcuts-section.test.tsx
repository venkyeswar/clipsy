import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { ShortcutsSection } from "./shortcuts-section";

afterEach(cleanup);

it("renders_configured_shortcut", () => {
  render(<ShortcutsSection />);
  for (const key of ["Super", "Shift", "V"]) {
    expect(screen.getByText(key, { selector: "kbd" })).toBeInTheDocument();
  }
  expect(screen.getByText("Open Clipsy")).toBeInTheDocument();
});
