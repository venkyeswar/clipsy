import { cleanup, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it } from "vitest";
import { HelpSection } from "./help-section";
import { renderWithPlatform } from "@/test/render-with-platform";

const questions = [
  "Super+V does nothing",
  "Does it work on Wayland?",
  "I can't use sudo",
  "How do I uninstall?",
  "Where is my data stored?",
];

beforeEach(() => {
  window.localStorage.clear();
});

afterEach(cleanup);

const windowsQuestions = [
  "The shortcut does nothing",
  "Windows says 'unknown publisher'",
  "Can I use it without installing?",
  "How do I uninstall?",
  "Where is my data stored?",
];

it("lists_all_five_questions", () => {
  renderWithPlatform(<HelpSection />, "linux");
  for (const question of questions) {
    expect(screen.getByRole("button", { name: question })).toBeInTheDocument();
  }
});

it("no_sudo_answer_uses_the_installer_and_local_uninstaller", async () => {
  renderWithPlatform(<HelpSection />, "linux");
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
  renderWithPlatform(<HelpSection />, "linux");
  await userEvent.click(screen.getByRole("button", { name: "Where is my data stored?" }));
  expect(await screen.findByText("~/snap/clipsy/")).toBeInTheDocument();
  expect(screen.getByText("rm -r ~/.local/share/clipsy")).toBeInTheDocument();

  await userEvent.click(screen.getByRole("button", { name: "How do I uninstall?" }));
  expect(await screen.findByText(/Snap saves an automatic snapshot/)).toBeInTheDocument();
  expect(screen.getByText(/asks whether to keep or delete your history/)).toBeInTheDocument();
});

it("wayland_answer_mentions_logging_out_once", async () => {
  renderWithPlatform(<HelpSection />, "linux");
  await userEvent.click(screen.getByRole("button", { name: "Does it work on Wayland?" }));
  expect(await screen.findByText(/Log out and back in once/)).toBeInTheDocument();
  expect(screen.getByText("Other Wayland desktops are not supported yet.")).toBeInTheDocument();
});

it("windows_lists_windows_questions_only", () => {
  renderWithPlatform(<HelpSection />, "windows");
  for (const question of windowsQuestions) {
    expect(screen.getByRole("button", { name: question })).toBeInTheDocument();
  }
  expect(screen.queryByRole("button", { name: "Does it work on Wayland?" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "I can't use sudo" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Super+V does nothing" })).not.toBeInTheDocument();
});

it("windows_shortcut_answer_uses_ctrl_shift_v", async () => {
  renderWithPlatform(<HelpSection />, "windows");
  await userEvent.click(screen.getByRole("button", { name: "The shortcut does nothing" }));
  expect(await screen.findByText(/Ctrl\+Shift\+V/)).toBeInTheDocument();
  expect(screen.getByText(/Windows reserves Win\+V/)).toBeInTheDocument();
  expect(screen.getByText('$env:CLIPSY_LOG = "debug"')).toBeInTheDocument();
});

it("windows_data_answer_gives_the_windows_folders", async () => {
  renderWithPlatform(<HelpSection />, "windows");
  await userEvent.click(screen.getByRole("button", { name: "Where is my data stored?" }));
  expect(await screen.findByText("%LOCALAPPDATA%\\clipsy\\data")).toBeInTheDocument();
  expect(screen.getByText("%APPDATA%\\clipsy\\config\\config.toml")).toBeInTheDocument();
});

it("windows_portable_answer_says_no_admin_needed", async () => {
  renderWithPlatform(<HelpSection />, "windows");
  await userEvent.click(screen.getByRole("button", { name: "Can I use it without installing?" }));
  expect(await screen.findByText(/No admin rights are needed/)).toBeInTheDocument();
});

it("windows_smartscreen_answer_mentions_run_anyway", async () => {
  renderWithPlatform(<HelpSection />, "windows");
  await userEvent.click(screen.getByRole("button", { name: "Windows says 'unknown publisher'" }));
  expect(await screen.findByText(/Run anyway/)).toBeInTheDocument();
});
