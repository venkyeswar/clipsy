import { act, cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { SiteHeader } from "./site-header";
import { ThemeProvider } from "./theme-provider";

type Callback = (entries: Partial<IntersectionObserverEntry>[]) => void;

let lastCallback: Callback | undefined;

class FakeIntersectionObserver {
  constructor(callback: Callback) {
    lastCallback = callback;
  }
  observe() {}
  unobserve() {}
  disconnect() {}
  takeRecords() {
    return [];
  }
}

function renderHeader() {
  return render(
    <ThemeProvider>
      <SiteHeader />
    </ThemeProvider>,
  );
}

beforeEach(() => {
  lastCallback = undefined;
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

it("marks_active_section_link", () => {
  renderHeader();
  const downloads = document.createElement("section");
  downloads.id = "downloads";
  document.body.appendChild(downloads);

  act(() => {
    lastCallback?.([{ isIntersecting: true, target: downloads }]);
  });

  const link = screen.getByRole("link", { name: "Downloads" });
  expect(link).toHaveAttribute("aria-current", "location");
  expect(screen.getByRole("link", { name: "Install" })).not.toHaveAttribute("aria-current");

  downloads.remove();
});

it("closes_mobile_menu_when_link_clicked", async () => {
  renderHeader();
  await userEvent.click(screen.getByRole("button", { name: "Open menu" }));
  const dialog = await screen.findByRole("dialog");
  expect(dialog).toBeInTheDocument();

  await userEvent.click(within(dialog).getByRole("link", { name: "Help" }));
  await vi.waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
});
