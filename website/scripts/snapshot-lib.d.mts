export type SnapshotAsset = { name: string; browser_download_url: string; size: number };
export type SnapshotRelease = {
  tag_name: string;
  html_url: string;
  published_at: string;
  created_at: string;
  draft: boolean;
  prerelease: boolean;
  assets: SnapshotAsset[];
};
export function buildSnapshot(body: unknown): SnapshotRelease[] | null;
