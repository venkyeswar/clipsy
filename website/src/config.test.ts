import { describe, expect, it } from "vitest";
import { shortcutFor } from "./config";

describe("shortcutFor", () => {
  it("uses Ctrl+Shift+V on Windows", () => {
    expect(shortcutFor("windows")).toEqual(["Ctrl", "Shift", "V"]);
  });

  it("keeps the Linux default Super+Shift+V elsewhere", () => {
    expect(shortcutFor("linux")).toEqual(["Super", "Shift", "V"]);
    expect(shortcutFor("other")).toEqual(["Super", "Shift", "V"]);
  });
});
