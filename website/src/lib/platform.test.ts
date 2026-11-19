import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { detectPlatform, usePlatform } from "./platform";

const UA = {
  chromeWindows:
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
  edgeWindows:
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36 Edg/125.0.0.0",
  firefoxWindows32: "Mozilla/5.0 (Windows NT 6.1; WOW64; rv:126.0) Gecko/20100101 Firefox/126.0",
  firefoxUbuntu: "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:126.0) Gecko/20100101 Firefox/126.0",
  chromeLinux: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
  androidChrome:
    "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Mobile Safari/537.36",
  safariMac:
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15",
  iphoneSafari:
    "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Mobile/15E148 Safari/604.1",
  chromeOs:
    "Mozilla/5.0 (X11; CrOS x86_64 14541.0.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
};

describe("detectPlatform", () => {
  it.each([
    [UA.chromeWindows, "windows"],
    [UA.edgeWindows, "windows"],
    [UA.firefoxWindows32, "windows"],
    [UA.firefoxUbuntu, "linux"],
    [UA.chromeLinux, "linux"],
    [UA.androidChrome, "other"],
    [UA.safariMac, "mac"],
    [UA.iphoneSafari, "other"],
    [UA.chromeOs, "other"],
    ["", "other"],
  ])("classifies %s as %s", (userAgent, expected) => {
    expect(detectPlatform(userAgent)).toBe(expected);
  });
});

describe("usePlatform", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("detects the platform from navigator.userAgent", () => {
    vi.stubGlobal("navigator", { userAgent: UA.chromeWindows });
    const { result } = renderHook(() => usePlatform());
    expect(result.current.platform).toBe("windows");
  });

  it("defaults to linux when navigator is unavailable", () => {
    vi.stubGlobal("navigator", undefined);
    const { result } = renderHook(() => usePlatform());
    expect(result.current.platform).toBe("linux");
  });

  it("lets the visitor override the detected platform", () => {
    vi.stubGlobal("navigator", { userAgent: UA.chromeLinux });
    const { result } = renderHook(() => usePlatform());
    expect(result.current.platform).toBe("linux");
    act(() => result.current.setPlatform("windows"));
    expect(result.current.platform).toBe("windows");
  });
});
