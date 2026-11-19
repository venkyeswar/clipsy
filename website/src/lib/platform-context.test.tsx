import type { ReactNode } from "react";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PlatformProvider, toSelectedPlatform, useSelectedPlatform } from "./platform-context";

const WINDOWS_UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/124.0.0.0 Safari/537.36";
const LINUX_UA = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/124.0.0.0 Safari/537.36";
const MAC_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Safari/605.1.15";

const wrapper = ({ children }: { children: ReactNode }) => <PlatformProvider>{children}</PlatformProvider>;

// Named as a hook so the rules-of-hooks lint accepts calling useSelectedPlatform inside it.
const useProbe = () => useSelectedPlatform();

function useWithUserAgent(userAgent: string) {
  vi.stubGlobal("navigator", { userAgent });
  return renderHook(useProbe, { wrapper });
}

beforeEach(() => {
  window.localStorage.clear();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  window.localStorage.clear();
});

describe("toSelectedPlatform", () => {
  it("keeps windows and maps every other platform to linux", () => {
    expect(toSelectedPlatform("windows")).toBe("windows");
    expect(toSelectedPlatform("linux")).toBe("linux");
    expect(toSelectedPlatform("mac")).toBe("linux");
    expect(toSelectedPlatform("other")).toBe("linux");
  });
});

describe("PlatformProvider", () => {
  it("defaults a Windows visitor to windows", () => {
    const { result } = useWithUserAgent(WINDOWS_UA);
    expect(result.current.platform).toBe("windows");
  });

  it.each([
    ["linux", LINUX_UA],
    ["mac", MAC_UA],
  ])("defaults a %s visitor to linux", (_name, userAgent) => {
    const { result } = useWithUserAgent(userAgent);
    expect(result.current.platform).toBe("linux");
  });

  it("lets the visitor switch and remembers the choice", () => {
    const { result } = useWithUserAgent(LINUX_UA);
    act(() => result.current.setPlatform("windows"));
    expect(result.current.platform).toBe("windows");
    expect(window.localStorage.getItem("clipsy-platform")).toBe("windows");
  });

  it("uses a stored choice over the detected platform", () => {
    window.localStorage.setItem("clipsy-platform", "linux");
    const { result } = useWithUserAgent(WINDOWS_UA);
    expect(result.current.platform).toBe("linux");
  });

  it("ignores an invalid stored value", () => {
    window.localStorage.setItem("clipsy-platform", "mac");
    const { result } = useWithUserAgent(WINDOWS_UA);
    expect(result.current.platform).toBe("windows");
  });

  it("still switches when localStorage throws", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    const { result } = useWithUserAgent(LINUX_UA);
    expect(result.current.platform).toBe("linux");
    act(() => result.current.setPlatform("windows"));
    expect(result.current.platform).toBe("windows");
  });

  it("throws when used outside the provider", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() => renderHook(useProbe)).toThrow(/PlatformProvider/);
    spy.mockRestore();
  });
});
