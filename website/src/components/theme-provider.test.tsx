import { render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ThemeProvider, useTheme } from "./theme-provider";

function ThemeProbe() {
  const { theme } = useTheme();
  return <p data-testid="theme">{theme}</p>;
}

afterEach(() => {
  vi.restoreAllMocks();
  window.localStorage.clear();
  document.documentElement.classList.remove("dark");
});

it("falls_back_when_localStorage_throws", () => {
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
    throw new Error("storage blocked");
  });

  render(
    <ThemeProvider>
      <ThemeProbe />
    </ThemeProvider>,
  );

  expect(screen.getByTestId("theme")).toHaveTextContent("system");
});

it("applies_dark_class_when_dark", () => {
  window.localStorage.setItem("clipsy-theme", "dark");

  render(
    <ThemeProvider>
      <ThemeProbe />
    </ThemeProvider>,
  );

  expect(document.documentElement).toHaveClass("dark");
});
