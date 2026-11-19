import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { SiteFooter } from "./site-footer";

it("credits Venkateswarlu Garnimitta as plain text in the footer row", () => {
  const { container } = render(<SiteFooter />);

  const name = screen.getByText("Venkateswarlu Garnimitta");
  expect(name).toBeInTheDocument();

  const credit = name.closest("p");
  expect(credit).not.toBeNull();

  // Accessible text (ignoring the aria-hidden copyright sign) reads as a credit.
  const accessible = credit!.cloneNode(true) as HTMLElement;
  accessible.querySelectorAll('[aria-hidden="true"]').forEach((el) => el.remove());
  expect(accessible.textContent?.replace(/\s+/g, " ").trim()).toBe(
    "Developed by Venkateswarlu Garnimitta",
  );

  const symbol = screen.getByText("©");
  expect(symbol).toHaveAttribute("aria-hidden", "true");

  expect(credit!.closest("a")).toBeNull();

  expect(container.querySelector("svg.lucide-heart")).toBeNull();

  // Credit and footer nav share the same row container.
  const row = container.querySelector("footer > div");
  expect(row).not.toBeNull();
  expect(row).toContainElement(credit as HTMLElement);
  expect(row).toContainElement(screen.getByRole("navigation", { name: "Footer" }));

  expect(container.textContent).not.toContain("Thalvik Labs");
});
