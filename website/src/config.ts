export const config = {
  repo: "venkyeswar/clipsy",
  snap: { channel: "edge" as "stable" | "edge" | null },
  // Default global toggle shortcut from crates/clipsy-core/src/config.rs: <Super><Shift>v
  shortcut: ["Super", "Shift", "V"] as string[],
};
