import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { ScrollPanel } from "./scroll-panel";

afterEach(cleanup);

function renderPanel() {
  return render(
    <ScrollPanel
      id="panel"
      headingId="panel-heading"
      heading={<h2 id="panel-heading">Panel title</h2>}
      label="Panel items, scrollable"
    >
      <p>First item</p>
    </ScrollPanel>,
  );
}

it("labels_the_section_by_its_heading", () => {
  renderPanel();
  expect(screen.getByRole("region", { name: "Panel items, scrollable" })).toBeInTheDocument();
  expect(document.getElementById("panel")).toHaveAttribute("aria-labelledby", "panel-heading");
});

it("keeps_the_heading_outside_the_scroll_region", () => {
  renderPanel();
  const region = screen.getByRole("region", { name: "Panel items, scrollable" });
  expect(within(region).queryByRole("heading", { name: "Panel title" })).not.toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "Panel title" })).toBeInTheDocument();
});

it("scrolls_inside_its_own_body_on_large_screens_when_bounded", () => {
  render(
    <ScrollPanel
      id="panel"
      headingId="panel-heading"
      heading={<h2 id="panel-heading">Panel title</h2>}
      label="Panel items, scrollable"
      bounded
    >
      <p>First item</p>
    </ScrollPanel>,
  );
  const region = screen.getByRole("region", { name: "Panel items, scrollable" });
  expect(region).toHaveClass("lg:overflow-y-auto", "lg:max-h-[min(36rem,calc(100dvh-9rem))]", "lg:pr-2");
});

it("is_not_bounded_unless_asked", () => {
  renderPanel();
  const region = screen.getByRole("region", { name: "Panel items, scrollable" });
  expect(region).not.toHaveClass("lg:overflow-y-auto");
});

it("is_keyboard_focusable_only_while_it_can_scroll", () => {
  const originalScroll = Object.getOwnPropertyDescriptor(Element.prototype, "scrollHeight");
  const originalClient = Object.getOwnPropertyDescriptor(Element.prototype, "clientHeight");
  try {
    renderPanel();
    const region = screen.getByRole("region", { name: "Panel items, scrollable" });
    expect(region).not.toHaveAttribute("tabindex");
    cleanup();

    Object.defineProperty(Element.prototype, "scrollHeight", { configurable: true, get: () => 500 });
    Object.defineProperty(Element.prototype, "clientHeight", { configurable: true, get: () => 100 });
    renderPanel();
    expect(screen.getByRole("region", { name: "Panel items, scrollable" })).toHaveAttribute("tabindex", "0");
  } finally {
    if (originalScroll) Object.defineProperty(Element.prototype, "scrollHeight", originalScroll);
    if (originalClient) Object.defineProperty(Element.prototype, "clientHeight", originalClient);
  }
});
