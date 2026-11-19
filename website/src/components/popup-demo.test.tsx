import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it } from "vitest";
import { PopupDemo } from "./popup-demo";

afterEach(cleanup);

const labels = [
  "git clone git@github.com:venkyeswar/clipsy.git",
  "meeting notes: ship the website Friday",
  "screenshot-2026-11-19.png",
  "invoice.pdf",
  "sudo apt install ./clipsy_0.3.1_amd64.deb",
  "logo.svg",
];

it("renders_the_six_demo_items", () => {
  render(<PopupDemo autoplay={false} />);
  for (const label of labels) {
    expect(screen.getByText(label)).toBeInTheDocument();
  }
});

it("trash_dialog_checkbox_unchecked_and_hidden_on_pinned_tab", async () => {
  const user = userEvent.setup();
  render(<PopupDemo autoplay={false} />);

  await user.click(screen.getByRole("button", { name: "Clear everything" }));
  const dialog = await screen.findByRole("alertdialog");
  const checkbox = within(dialog).getByRole("checkbox", { name: "Also remove pinned items" });
  expect(checkbox).not.toBeChecked();

  await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
  await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());

  await user.click(screen.getByRole("tab", { name: "Pinned" }));
  await user.click(screen.getByRole("button", { name: "Clear pinned items" }));
  const pinnedDialog = await screen.findByRole("alertdialog");
  expect(within(pinnedDialog).queryByRole("checkbox")).toBeNull();
});

it("focus_moves_to_search_when_last_row_removed", async () => {
  const user = userEvent.setup();
  render(<PopupDemo autoplay={false} />);
  const search = screen.getByRole("searchbox", { name: "Search clipboard history" });

  await user.type(search, "logo");
  await user.click(screen.getByRole("option", { name: /logo\.svg/ }));
  await user.keyboard("{Backspace}");

  expect(document.activeElement).toBe(search);
});

it("focus_returns_to_search_after_clearing_everything", async () => {
  const user = userEvent.setup();
  render(<PopupDemo autoplay={false} />);
  const search = screen.getByRole("searchbox", { name: "Search clipboard history" });

  await user.click(screen.getByRole("button", { name: "Clear everything" }));
  const dialog = await screen.findByRole("alertdialog");
  await user.click(within(dialog).getByRole("checkbox", { name: "Also remove pinned items" }));
  await user.click(within(dialog).getByRole("button", { name: "Clear" }));

  await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  expect(screen.getByRole("button", { name: "Clear everything" })).toBeDisabled();
  await waitFor(() => expect(document.activeElement).toBe(search));
});
