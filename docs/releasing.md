# How to publish a Clipsy release

This guide is for the person who makes a new version of Clipsy available to
everyone. You do not need to build anything on your own computer. GitHub does the
building when you push a **tag** (a label such as `v0.4.0`).

## What a release contains

Every release has these files, all attached to one GitHub Release:

| File | For | Built on |
| --- | --- | --- |
| `clipsy_<version>-1_amd64.deb` | Ubuntu / Debian (Linux) | Ubuntu 24.04 runner |
| `clipsy-<version>-windows-x64.zip` | Windows, portable (unzip and run) | Windows runner |
| `clipsy-<version>-setup.exe` | Windows installer | Windows runner (Inno Setup) |
| `SHA256SUMS` | Checksums for the three files above | Release job |
| `install.sh` | Linux per-user installer (no sudo) | copied from `packaging/` |
| `uninstall.sh` | Linux per-user uninstaller | copied from `packaging/` |

Example for version `0.4.0`: `clipsy_0.4.0-1_amd64.deb`, `clipsy-0.4.0-windows-x64.zip`,
`clipsy-0.4.0-setup.exe`.

Release notes are generated automatically from the commit messages since the
previous tag.

## Before you start

1. Make sure `main` (or the branch you release from) is pushed and CI is green.
2. Set the version in the workspace `Cargo.toml` (the `version = "..."` line under
   `[workspace.package]`) to the version you want, for example `0.4.0`. Commit it.
   The release workflow stops with an error if the tag and this version differ.

## Cut a release

```bash
git tag v0.4.0
git push origin v0.4.0
```

That is all. Then open the **Actions** tab on GitHub
(`https://github.com/venkyeswar/clipsy/actions`) and watch the **Release** run.
When it finishes, the new release is at
`https://github.com/venkyeswar/clipsy/releases`.

For a release candidate, use a hyphen, for example `v0.4.0-rc1`. The release is
then marked as a **pre-release**, and the website does not offer it as the latest
version.

To delete a tag you pushed by mistake:

```bash
git tag -d v0.4.0
git push origin :refs/tags/v0.4.0
```

Then delete the draft or the release on GitHub, and push the corrected tag.

## Dry run (build without publishing)

A dry run builds all the files but publishes nothing. Use it to check the build
before you tag.

1. On GitHub, open **Actions**, then **Release**, then **Run workflow**.
2. Pick the branch and click **Run workflow**.
3. When the run is green, open it and download the files from the **Artifacts**
   section at the bottom of the page (`deb`, `windows-zip`, `windows-setup`).

A dry run uses the version in `Cargo.toml`. The release job is skipped, so nothing
is published.

## How the website gets the files

The website does not need to be changed for a new release. It reads the public
list of GitHub Releases (`https://api.github.com/repos/venkyeswar/clipsy/releases`)
in the visitor's browser, and it falls back to a saved copy of that list
(`website/src/data/releases.snapshot.json`) if the live list cannot be read. The
build refreshes that copy. The Downloads section then offers the files by their
names, so the names above must not change.

## Checking the files after a release

Download the files, then check them against `SHA256SUMS`:

```bash
sha256sum -c SHA256SUMS
```

On Windows, PowerShell can show one hash:

```powershell
Get-FileHash .\clipsy-0.4.0-setup.exe -Algorithm SHA256
```

## Known limits

- **The Windows files are not signed.** There is no code-signing certificate yet.
  Windows SmartScreen shows "Windows protected your PC" the first time. Users
  click **More info**, then **Run anyway**. The checksums in `SHA256SUMS` let people
  confirm the download is the one we published.
- **The Windows app is new.** It has been built by CI but not yet tested on a
  real Windows PC by everyone. See `docs/windows-acceptance.md`.
- **Only 64-bit (x64) Windows and x86_64 Linux are built.**
- **No automatic updates.** Users download the new version themselves. On Linux,
  running `install.sh` again updates Clipsy.
- **Re-running a failed tag** does not overwrite a release that already exists.
  Delete the release on GitHub first, then re-run the workflow for that tag (or
  push the tag again after deleting it, as shown above).
