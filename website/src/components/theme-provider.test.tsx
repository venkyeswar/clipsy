import { act, cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { ThemeProvider, useTheme } from "./theme-provider";

function ThemeProbe() {
  const { theme, resolvedTheme, toggleTheme } = useTheme();
  return (
    <>
      <p data-testid="theme">{theme}</p>
      <p data-testid="resolved">{resolvedTheme}</p>
      <button onClick={toggleTheme}>toggle</button>
    </>
  );
}

// A fake OS colour scheme that tests can flip to simulate a live change.
let osDark = false;
let osListeners: Array<(event: MediaQueryListEvent) => void> = [];

function mockOsScheme(initialDark: boolean) {
  osDark = initialDark;
  osListeners = [];
  vi.spyOn(window, "matchMedia").mockImplementation(
    (query: string) =>
      ({
        get matches() {
          return query === "(prefers-color-scheme: dark)" ? osDark : false;
        },
        media: query,
        addEventListener: (_: string, listener: (event: MediaQueryListEvent) => void) => {
          osListeners.push(listener);
        },
        removeEventListener: (_: string, listener: (event: MediaQueryListEvent) => void) => {
          osListeners = osListeners.filter((l) => l !== listener);
        },
      }) as unknown as MediaQueryList,
  );
}

function changeOsScheme(dark: boolean) {
  osDark = dark;
  act(() => {
    for (const listener of osListeners) {
      listener({ matches: dark } as MediaQueryListEvent);
    }
  });
}

afterEach(() => {
  cleanup();
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
  expect(screen.getByTestId("resolved")).toHaveTextContent("light");
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

it("default_follows_os_dark_and_live_changes", () => {
  mockOsScheme(true);
  render(
    <ThemeProvider>
      <ThemeProbe />
    </ThemeProvider>,
  );
  expect(screen.getByTestId("resolved")).toHaveTextContent("dark");
  expect(document.documentElement).toHaveClass("dark");

  changeOsScheme(false);
  expect(screen.getByTestId("resolved")).toHaveTextContent("light");
  expect(document.documentElement).not.toHaveClass("dark");
});

it("toggle_switches_to_opposite_of_applied_and_stores_it", async () => {
  mockOsScheme(false);
  render(
    <ThemeProvider>
      <ThemeProbe />
    </ThemeProvider>,
  );

  await userEvent.click(screen.getByRole("button", { name: "toggle" }));
  expect(screen.getByTestId("resolved")).toHaveTextContent("dark");
  expect(window.localStorage.getItem("clipsy-theme")).toBe("dark");
  expect(document.documentElement).toHaveClass("dark");

  await userEvent.click(screen.getByRole("button", { name: "toggle" }));
  expect(screen.getByTestId("resolved")).toHaveTextContent("light");
  expect(window.localStorage.getItem("clipsy-theme")).toBe("light");
});

it("explicit_choice_ignores_later_os_changes", async () => {
  mockOsScheme(false);
  render(
    <ThemeProvider>
      <ThemeProbe />
    </ThemeProvider>,
  );

  await userEvent.click(screen.getByRole("button", { name: "toggle" }));
  changeOsScheme(false);
  expect(screen.getByTestId("resolved")).toHaveTextContent("dark");
  changeOsScheme(true);
  expect(screen.getByTestId("resolved")).toHaveTextContent("dark");
  expect(screen.getByTestId("theme")).toHaveTextContent("dark");
});
