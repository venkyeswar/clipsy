// Pure helpers for generate-snapshot.mjs. Kept separate so they can be unit tested.

/**
 * Turns the GitHub releases body into the snapshot array written to the bundle.
 * Drafts are dropped so they never ship to the client. Returns null if body is not an array.
 */
export function buildSnapshot(body) {
  if (!Array.isArray(body)) return null;
  return body
    .filter((release) => release && typeof release === "object" && release.draft !== true)
    .map((release) => ({
      tag_name: release.tag_name,
      html_url: release.html_url,
      published_at: release.published_at,
      created_at: release.created_at,
      draft: release.draft,
      prerelease: release.prerelease,
      assets: (Array.isArray(release.assets) ? release.assets : []).map((a) => ({
        name: a.name,
        browser_download_url: a.browser_download_url,
        size: a.size,
      })),
    }));
}
