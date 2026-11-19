// Fetches the public release list and writes a fallback snapshot for the site.
// Never fails the build: on any error it warns and leaves the existing file as-is.
import { writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const REPO = "venkyeswar/clipsy";
const URL_ = `https://api.github.com/repos/${REPO}/releases`;
const OUT = fileURLToPath(new URL("../src/data/releases.snapshot.json", import.meta.url));

function slim(release) {
  const assets = Array.isArray(release.assets) ? release.assets : [];
  return {
    tag_name: release.tag_name,
    html_url: release.html_url,
    published_at: release.published_at,
    created_at: release.created_at,
    draft: release.draft,
    prerelease: release.prerelease,
    assets: assets.map((a) => ({
      name: a.name,
      browser_download_url: a.browser_download_url,
      size: a.size,
    })),
  };
}

async function main() {
  const headers = { Accept: "application/vnd.github+json" };
  if (process.env.GITHUB_TOKEN) headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;

  const res = await fetch(URL_, { headers });
  if (!res.ok) throw new Error(`GitHub API responded ${res.status}`);
  const body = await res.json();
  if (!Array.isArray(body)) throw new Error("GitHub API returned a non-array body");

  const text = `${JSON.stringify(body.map(slim), null, 2)}\n`;
  await writeFile(OUT, text, "utf8");
  console.log(`Wrote ${body.length} release(s) to snapshot.`);
}

try {
  await main();
} catch (error) {
  // Keep the existing snapshot; the build must not fail on network problems.
  console.warn(`Warning: could not refresh release snapshot (${error.message}). Keeping the existing file.`);
}
process.exitCode = 0;
