// Fetches the public release list and writes a fallback snapshot for the site.
// Never fails the build: on any error it warns and leaves the existing file as-is.
import { writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { buildSnapshot } from "./snapshot-lib.mjs";

const REPO = "venkyeswar/clipsy";
const URL_ = `https://api.github.com/repos/${REPO}/releases`;
const OUT = fileURLToPath(new URL("../src/data/releases.snapshot.json", import.meta.url));

async function main() {
  const headers = { Accept: "application/vnd.github+json" };
  if (process.env.GITHUB_TOKEN) headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;

  const res = await fetch(URL_, { headers });
  if (!res.ok) throw new Error(`GitHub API responded ${res.status}`);
  const body = await res.json();
  const snapshot = buildSnapshot(body);
  if (!snapshot) throw new Error("GitHub API returned a non-array body");

  const text = `${JSON.stringify(snapshot, null, 2)}\n`;
  await writeFile(OUT, text, "utf8");
  console.log(`Wrote ${snapshot.length} release(s) to snapshot (drafts excluded).`);
}

try {
  await main();
} catch (error) {
  // Keep the existing snapshot; the build must not fail on network problems.
  console.warn(`Warning: could not refresh release snapshot (${error.message}). Keeping the existing file.`);
}
process.exitCode = 0;
