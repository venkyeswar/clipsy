import { describe, expect, it } from "vitest";
import { formatBytes } from "./format";

describe("formatBytes", () => {
  it("formats zero as bytes", () => {
    expect(formatBytes(0)).toBe("0 B");
  });

  it("formats kibibytes with one decimal", () => {
    expect(formatBytes(1536)).toBe("1.5 KB");
  });

  it("formats mebibytes with one decimal", () => {
    expect(formatBytes(4404019)).toBe("4.2 MB");
  });

  it("drops a trailing .0", () => {
    expect(formatBytes(1024)).toBe("1 KB");
  });

  it("treats invalid sizes as zero", () => {
    expect(formatBytes(-5)).toBe("0 B");
    expect(formatBytes(Number.NaN)).toBe("0 B");
  });
});
