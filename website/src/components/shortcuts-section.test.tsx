import { cleanup, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it } from "vitest";
import { ShortcutsSection } from "./shortcuts-section";
import { renderWithPlatform } from "@/test/render-with-platform";

beforeEach(() => {
  window.localStorage.clear();
});

afterEach(cleanup);

it("renders_linux_shortcut_on_linux", () => {
  renderWithPlatform(<ShortcutsSection />, "linux");
  for (const key of ["Super", "Shift", "V"]) {
    expect(screen.getByText(key, { selector: "kbd" })).toBeInTheDocument();
  }
  expect(screen.getByText("Open Clipsy")).toBeInTheDocument();
});

it("renders_windows_shortcut_on_windows", () => {
  renderWithPlatform(<ShortcutsSection />, "windows");
  const row = screen.getByText("Open Clipsy").closest("div") as HTMLElement;
  const keys = within(row).getAllByText(/^(Ctrl|Shift|V)$/, { selector: "kbd" });
  expect(keys.map((key) => key.textContent)).toEqual(["Ctrl", "Shift", "V"]);
  expect(screen.queryByText("Super", { selector: "kbd" })).not.toBeInTheDocument();
});
