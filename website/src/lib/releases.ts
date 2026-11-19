import { config } from "../config";

export type ReleaseAsset = { name: string; url: string; size: number };
export type Release = {
  version: string;
  tag: string;
  date: string;
  prerelease: boolean;
  notesUrl: string;
  deb?: ReleaseAsset;
  tarball?: ReleaseAsset;
  /** Same asset as `sha256sums`; kept so existing callers keep working. */
  checksums?: ReleaseAsset;
  sha256sums?: ReleaseAsset;
  windowsInstaller?: ReleaseAsset;
  windowsPortable?: ReleaseAsset;
};

const DEB_PATTERN = /\.deb$/;
const TARBALL_PATTERN = /^clipsy-.*-linux-x86_64\.tar\.gz$/;
const CHECKSUMS_PATTERN = /^SHA256SUMS$/;
const WINDOWS_INSTALLER_PATTERN = /^clipsy-.+-setup\.exe$/i;
const WINDOWS_PORTABLE_PATTERN = /^clipsy-.+-windows-x64\.zip$/i;

type Obj = Record<string, unknown>;

const isObject = (value: unknown): value is Obj =>
  typeof value === "object" && value !== null && !Array.isArray(value);

function toVersion(tag: string): string {
  return tag.replace(/^v/, "");
}

function splitVersion(version: string): { nums: number[]; pre: string | null } {
  const plain = version.split("+")[0];
  const dash = plain.indexOf("-");
  const core = dash === -1 ? plain : plain.slice(0, dash);
  const pre = dash === -1 ? null : plain.slice(dash + 1);
  const nums = core.split(".").map((segment) => (/^\d+$/.test(segment) ? Number(segment) : 0));
  return { nums, pre };
}

const sign = (n: number): number => (n > 0 ? 1 : n < 0 ? -1 : 0);

/** Semver-style comparison: numeric segments first, then a release beats its pre-release. */
export function compareVersions(a: string, b: string): number {
  const left = splitVersion(a);
  const right = splitVersion(b);
  const length = Math.max(left.nums.length, right.nums.length);
  for (let i = 0; i < length; i++) {
    const diff = (left.nums[i] ?? 0) - (right.nums[i] ?? 0);
    if (diff !== 0) return sign(diff);
  }
  if (left.pre === right.pre) return 0;
  if (left.pre === null) return 1;
  if (right.pre === null) return -1;
  return left.pre < right.pre ? -1 : 1;
}

function toAsset(value: unknown): ReleaseAsset | undefined {
  if (!isObject(value)) return undefined;
  if (typeof value.name !== "string" || typeof value.browser_download_url !== "string") return undefined;
  const size = typeof value.size === "number" && Number.isFinite(value.size) ? value.size : 0;
  return { name: value.name, url: value.browser_download_url, size };
}

/** Converts raw GitHub release JSON into Release records, newest first by semver. */
export function parseReleases(raw: unknown): Release[] {
  if (!Array.isArray(raw)) return [];
  const releases: Release[] = [];
  for (const entry of raw) {
    if (!isObject(entry) || entry.draft === true || typeof entry.tag_name !== "string") continue;
    const tag = entry.tag_name;
    const version = toVersion(tag);
    const assets: ReleaseAsset[] = [];
    if (Array.isArray(entry.assets)) {
      for (const candidate of entry.assets) {
        const asset = toAsset(candidate);
        if (asset) assets.push(asset);
      }
    }
    const pick = (pattern: RegExp) => assets.find((asset) => pattern.test(asset.name));
    const date =
      typeof entry.published_at === "string"
        ? entry.published_at
        : typeof entry.created_at === "string"
          ? entry.created_at
          : "";
    releases.push({
      version,
      tag,
      date,
      prerelease: entry.prerelease === true,
      notesUrl:
        typeof entry.html_url === "string"
          ? entry.html_url
          : `https://github.com/${config.repo}/releases/tag/${tag}`,
      deb: pick(DEB_PATTERN),
      tarball: pick(TARBALL_PATTERN),
      checksums: pick(CHECKSUMS_PATTERN),
      sha256sums: pick(CHECKSUMS_PATTERN),
      windowsInstaller: pick(WINDOWS_INSTALLER_PATTERN),
      windowsPortable: pick(WINDOWS_PORTABLE_PATTERN),
    });
  }
  return releases.sort((a, b) => compareVersions(b.version, a.version));
}

const isStable = (release: Release): boolean => !release.prerelease && !release.version.includes("-");

/** First release that is neither a GitHub pre-release nor a version with a pre-release suffix. */
export function latestStable(releases: Release[]): Release | undefined {
  return releases.find(isStable);
}

/** True when the release has the Windows setup installer or the portable zip. */
export function hasWindows(release: Release): boolean {
  return Boolean(release.windowsInstaller || release.windowsPortable);
}

/** Newest stable release that has a Windows asset, or undefined. */
export function latestWithWindows(releases: Release[]): Release | undefined {
  return releases.find((release) => isStable(release) && hasWindows(release));
}

/** 'available' once a stable release ships Windows files, otherwise 'coming-soon'. */
export function windowsStatus(releases: Release[]): "available" | "coming-soon" {
  return latestWithWindows(releases) ? "available" : "coming-soon";
}
