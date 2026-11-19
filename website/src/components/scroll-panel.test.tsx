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

it("makes_the_scroll_region_keyboard_focusable", () => {
  renderPanel();
  const region = screen.getByRole("region", { name: "Panel items, scrollable" });
  expect(region).toHaveAttribute("tabindex", "0");
  expect(region).toHaveClass("overflow-y-auto", "scroll-panel");
});
