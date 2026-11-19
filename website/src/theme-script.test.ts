import { afterEach, beforeEach, expect, it, vi } from "vitest";
import html from "../index.html?raw";

// Runs the inline pre-paint script from index.html against jsdom.
const script = html.match(/<script>([\s\S]*?)<\/script>/)?.[1] ?? "";

function runPrePaint(systemDark: boolean) {
  vi.spyOn(window, "matchMedia").mockImplementation(
    (query: string) => ({ matches: systemDark, media: query }) as MediaQueryList,
  );
  new Function(script)();
}

beforeEach(() => {
  window.localStorage.clear();
  document.documentElement.classList.remove("dark");
});

afterEach(() => {
  vi.restoreAllMocks();
});

it("prepaint_applies_stored_dark_before_react", () => {
  window.localStorage.setItem("clipsy-theme", "dark");
  runPrePaint(false);
  expect(document.documentElement).toHaveClass("dark");
  expect(document.documentElement.style.colorScheme).toBe("dark");
});

it("prepaint_respects_explicit_light_over_os_dark", () => {
  window.localStorage.setItem("clipsy-theme", "light");
  runPrePaint(true);
  expect(document.documentElement).not.toHaveClass("dark");
});

it("prepaint_follows_os_when_system_or_missing", () => {
  runPrePaint(true);
  expect(document.documentElement).toHaveClass("dark");
});
