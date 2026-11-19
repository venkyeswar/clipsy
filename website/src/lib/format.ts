const UNITS = ["KB", "MB", "GB", "TB"];

/** Human-readable file size, e.g. 4404019 -> "4.2 MB". Invalid input is treated as 0. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  if (bytes < 1024) return `${bytes} B`;
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${Number(value.toFixed(1))} ${UNITS[unit]}`;
}
