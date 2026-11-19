import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it } from "vitest";
import { HelpSection } from "./help-section";

const questions = [
  "Super+V does nothing",
  "Does it work on Wayland?",
  "I can't use sudo",
  "How do I uninstall?",
  "Where is my data stored?",
];

afterEach(cleanup);

it("lists_all_five_questions", () => {
  render(<HelpSection />);
  for (const question of questions) {
    expect(screen.getByRole("button", { name: question })).toBeInTheDocument();
  }
});

it("no_sudo_answer_uses_the_installer_and_local_uninstaller", async () => {
  render(<HelpSection />);
  await userEvent.click(screen.getByRole("button", { name: "I can't use sudo" }));
  expect(
    await screen.findByText(
      "curl -fsSL https://github.com/venkyeswar/clipsy/releases/latest/download/install.sh -o install.sh",
    ),
  ).toBeInTheDocument();
  expect(screen.getByText("sh install.sh")).toBeInTheDocument();
  expect(screen.getAllByText("sh ~/.local/share/clipsy/uninstall.sh").length).toBeGreaterThan(0);
  expect(screen.queryByText(/tar -xzf|\.tar\.gz|^\.\/install\.sh$|^\.\/uninstall\.sh$/)).not.toBeInTheDocument();
  expect(screen.getAllByRole("button", { name: "Copy command" }).length).toBeGreaterThan(0);
});

it("data_and_uninstall_answers_distinguish_snap_installs", async () => {
  render(<HelpSection />);
  await userEvent.click(screen.getByRole("button", { name: "Where is my data stored?" }));
  expect(await screen.findByText("~/snap/clipsy/")).toBeInTheDocument();
  expect(screen.getByText("rm -r ~/.local/share/clipsy")).toBeInTheDocument();

  await userEvent.click(screen.getByRole("button", { name: "How do I uninstall?" }));
  expect(await screen.findByText(/Snap saves an automatic snapshot/)).toBeInTheDocument();
  expect(screen.getByText(/asks whether to keep or delete your history/)).toBeInTheDocument();
});

it("wayland_answer_mentions_logging_out_once", async () => {
  render(<HelpSection />);
  await userEvent.click(screen.getByRole("button", { name: "Does it work on Wayland?" }));
  expect(await screen.findByText(/Log out and back in once/)).toBeInTheDocument();
  expect(screen.getByText("Other Wayland desktops are not supported yet.")).toBeInTheDocument();
});
