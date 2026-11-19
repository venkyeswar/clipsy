import { describe, expect, it } from "vitest";
import { clearTab, countClearable, visibleItems, type DemoItem, type DemoTab } from "./demo";

// Fixture: 2 unpinned text, 1 pinned text, 1 unpinned image, 1 unpinned files, 1 pinned image.
const items: DemoItem[] = [
  { id: 1, kind: "text", label: "git clone git@github.com:venkyeswar/clipsy.git", pinned: false },
  { id: 2, kind: "text", label: "meeting notes: ship the website Friday", pinned: false },
  { id: 3, kind: "text", label: "sudo apt install ./clipsy_0.3.1_amd64.deb", pinned: true },
  { id: 4, kind: "image", label: "screenshot-2026-11-19.png", pinned: false },
  { id: 5, kind: "files", label: "invoice.pdf", pinned: false },
  { id: 6, kind: "image", label: "logo.svg", pinned: true },
];

const ids = (list: DemoItem[]) => list.map((item) => item.id);

describe("demo state", () => {
  it("clearTab_all_keeps_pinned_by_default", () => {
    expect(ids(clearTab(items, "all", false))).toEqual([3, 6]);
  });

  it("clearTab_all_include_pinned_removes_everything", () => {
    expect(ids(clearTab(items, "all", true))).toEqual([]);
  });

  it("clearTab_text_only_removes_text_unpinned", () => {
    expect(ids(clearTab(items, "text", false))).toEqual([3, 4, 5, 6]);
  });

  it("clearTab_images_leaves_text", () => {
    expect(ids(clearTab(items, "images", false))).toEqual([1, 2, 3, 5, 6]);
  });

  it("clearTab_pinned_removes_only_pinned_ignoring_flag", () => {
    expect(ids(clearTab(items, "pinned", false))).toEqual([1, 2, 4, 5]);
    expect(ids(clearTab(items, "pinned", true))).toEqual([1, 2, 4, 5]);
  });

  it("countClearable_matches_clearTab", () => {
    const tabs: DemoTab[] = ["all", "text", "images", "files", "pinned"];
    for (const tab of tabs) {
      for (const includePinned of [false, true]) {
        const expected = items.length - clearTab(items, tab, includePinned).length;
        expect(countClearable(items, tab, includePinned)).toBe(expected);
      }
    }
  });

  it("visibleItems_pins_first_and_filters_query", () => {
    expect(ids(visibleItems(items, "all", ""))).toEqual([3, 6, 1, 2, 4, 5]);
    expect(ids(visibleItems(items, "all", "GIT"))).toEqual([1]);
    expect(ids(visibleItems(items, "pinned", ""))).toEqual([3, 6]);
    expect(ids(visibleItems(items, "text", ""))).toEqual([3, 1, 2]);
  });
});
