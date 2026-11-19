import type { ReactElement } from "react";
import { render } from "@testing-library/react";
import { PlatformProvider, type SelectedPlatform } from "@/lib/platform-context";

/** Renders inside PlatformProvider with the platform remembered, as an earlier visitor choice would be. */
export function renderWithPlatform(ui: ReactElement, platform: SelectedPlatform = "linux") {
  window.localStorage.setItem("clipsy-platform", platform);
  return render(<PlatformProvider>{ui}</PlatformProvider>);
}
