# MaxRAR Help and release updates

Project links, the public update verification key and the fixed HTTPS feed endpoint are recorded in `release.config.json`. The Tauri plugin configuration must match; `scripts/check-release-config.py` and native tests enforce that match. No GitHub tokens or private signing keys are embedded in the app.

## Release configuration

From v0.1.1 onward, every build includes the public verification key. This removes the optional build environment variable that left v0.1.0 installers unable to check for updates. Users must install v0.1.1 manually once; v0.1.0 cannot acquire its missing key remotely.

Windows x64, macOS Intel, macOS Apple Silicon and Linux AppImage use Tauri's updater. DEB/RPM installations instead show **Download latest package** and do not offer misleading in-app installation controls. They must be upgraded by installing a newer package through the system package manager.

Updater signatures are separate from Windows Authenticode and macOS Developer ID signing/notarization. The former establishes which update packages MaxRAR accepts; it does not remove operating-system first-install warnings. OS publisher credentials remain separate release work.

## Preparing a release

1. Increase the version in `Cargo.toml`, `package.json`, `package-lock.json` and `src-tauri/tauri.conf.json`; regenerate `Cargo.lock`. Write `docs/releases/vVERSION.md`.
2. Preserve the existing update signing key. Its encrypted private key and password are GitHub repository secrets named `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Keep a protected recoverable backup outside the repository. Do not regenerate or replace it casually: installed apps trust the existing public key.
3. Merge to `main`, then tag that commit and push the tag: `git tag -a v0.1.2 -m "MaxRAR v0.1.2"` and `git push origin v0.1.2`. That is the whole release step.
4. The **Release** workflow (`.github/workflows/release.yml`) checks that the tag matches the app version and that the notes exist, creates a draft release (or reuses an existing draft for that tag), and builds Windows x64, macOS Apple Silicon, macOS Intel and Linux x64 in parallel. Windows is signed with Azure Artifact Signing; Mac builds are signed and notarized with the Developer ID certificate; updater packages are signed with the Tauri key.
5. After all four builds succeed, the workflow names the Mac DMGs for each chip, writes `latest.json` and `SHA256SUMS.txt`, publishes the release as **latest** with the notes from `docs/releases/vVERSION.md`, and points the README download links at the new version.

Tests run on every push and pull request in the **Desktop and CLI** workflow, not again during the release. If a platform build fails, fix it on `main`, then move the tag (`git tag -fa vVERSION -m "MaxRAR vVERSION"`, `git push -f origin vVERSION`); the workflow reuses the same draft.

The signing key was generated using the official Tauri CLI. Its public counterpart is safe to distribute. The signing private key/password must never enter source control, logs, release assets or application configuration.

## Public availability and acceptance

GitHub's unauthenticated release URLs do not serve private repository assets. Signed artifacts and their complete feed can be prepared and verified while the repository is private, but ordinary installed apps cannot read that feed until it is publicly accessible. A draft release is also inaccessible to the app. Access failures must remain errors, never **You're up to date**.

Before announcing a public release:

- Confirm the published latest release includes `latest.json` and all four platform assets and signatures, and that the feed and each asset can be downloaded without GitHub authentication.
- Confirm Check for updates reports the current version correctly on Windows x64, Intel Mac, Apple Silicon Mac and Linux AppImage.
- Exercise a genuine newer-version upgrade, signature verification, install and restart on each of those platforms. DEB/RPM should open the latest-package page.
- Exercise offline/missing feed, absent platform, invalid signature, interrupted download and retry. Installation must remain blocked while archive jobs are active.

The native staging verifier is also available locally:

```sh
cargo run --locked -p wonrar-desktop --example verify-updates -- /path/to/staged-release
```

It verifies real package bytes and the authenticated version using the same Minisign library used by Tauri, then checks that modified bytes are rejected. It does not execute installers or establish installed upgrade behavior.

## In-app behavior

Configured builds check at startup if the last attempt was at least 24 hours ago, and reconsider hourly while open. Attempts are recorded before network access to avoid repeated requests on every offline startup. Users can disable automatic checks; manual checks remain available.

Downloading and verification happen inside About. Tauri validates signatures and requires the version in the signature's authenticated comment to match the feed. Release notes render as plain text. The verified package remains in native memory until installation, another check or app exit. Closing About does not close an active download. Download interruptions and errors can be retried.

Installation is explicit, blocked by queued/running archive jobs, and holds the archive-job semaphore to prevent concurrent writes. Windows may hand off to the installer; macOS/Linux restart after successful installation. Application code does not accept caller-supplied feed URLs or verification keys.

Reference: https://v2.tauri.app/plugin/updater/
