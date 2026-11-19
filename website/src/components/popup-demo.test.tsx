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
