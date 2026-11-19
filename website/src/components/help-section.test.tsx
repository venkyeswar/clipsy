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

it("no_sudo_answer_has_copyable_uninstall_command", async () => {
  render(<HelpSection />);
  await userEvent.click(screen.getByRole("button", { name: "I can't use sudo" }));
  expect(await screen.findByText("./uninstall.sh")).toBeInTheDocument();
  expect(screen.getAllByRole("button", { name: "Copy command" }).length).toBeGreaterThan(0);
});
