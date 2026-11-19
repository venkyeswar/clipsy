import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { toast } from "sonner";
import { CopyButton } from "./copy-button";

vi.mock("sonner", () => ({ toast: vi.fn() }));
const mockedToast = vi.mocked(toast);

function setClipboard(writeText: (text: string) => Promise<void>) {
  Object.defineProperty(navigator, "clipboard", {
    value: { writeText },
    configurable: true,
  });
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

it("shows_fallback_toast_when_clipboard_denied", async () => {
  setClipboard(vi.fn().mockRejectedValue(new Error("denied")));
  render(<CopyButton text="sudo snap install clipsy" />);
  await userEvent.click(screen.getByRole("button", { name: "Copy command" }));
  await waitFor(() => expect(mockedToast).toHaveBeenCalledWith("Press Ctrl+C to copy"));
});

it("copies_text_and_toasts_copied", async () => {
  const writeText = vi.fn().mockResolvedValue(undefined);
  setClipboard(writeText);
  render(<CopyButton text="sudo snap install clipsy" />);
  await userEvent.click(screen.getByRole("button", { name: "Copy command" }));
  await waitFor(() => expect(mockedToast).toHaveBeenCalledWith("Copied"));
  expect(writeText).toHaveBeenCalledWith("sudo snap install clipsy");
});
