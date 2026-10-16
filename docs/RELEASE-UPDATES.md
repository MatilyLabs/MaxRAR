# MaxRAR Help and release updates

Repository: https://github.com/MatilyLabs/MaxRAR. All project destinations and the update endpoint live in `release.config.json`. The application opens only those fixed project links through its native bridge. Star and Report open GitHub pages; they do not submit anything or upload archive data.

## Current development state

Help/About, shortcuts, daily-check scheduling, download progress, signature verification, installation and active-job guards are implemented locally. The default development build has no verification public key and shows Updates aren’t available yet. It does not make background update requests, pretend to be up to date, or fetch unsigned executables. The build and update-staging workflows do not automatically publish releases.

A real signed release feed and OS upgrade checks are still required before calling the updater operational for users. Updater artifact signatures are separate from macOS code signing/notarization and Windows publisher signing. Those distribution credentials are not configured here.

## Enable release builds later

1. Review the Apache-2.0 LICENSE, project NOTICE and third-party redistribution notices before release.
2. Generate an updater signing key with the official Tauri CLI. Keep the private key outside source control, with a recoverable protected backup. Do not paste it into chat.
3. Put the public verification key in the GitHub repository variable `MAXRAR_UPDATER_PUBLIC_KEY`, and the private key/password in `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secrets. The public key is embedded at compile time using that environment variable; it is not a secret.
4. Increase the version consistently in the workspace Cargo manifest, package metadata and Tauri configuration. The About panel reads the runtime app version. Update comparison uses Tauri’s normal newer-version check.
5. Explicitly run Prepare signed updates. This manual-only workflow has read-only repository permissions and uploads CI artifacts; it contains no release publishing step and is not triggered by a push/tag. Its macOS artifacts need platform signing/notarization before public distribution.
6. Download the four staging artifacts. Each contains a uniquely named platform asset, its signature and a platform JSON. Merge the platform JSON objects into one `latest.json` (same version, actual release notes, all four platforms). Asset URLs target the corresponding `vVERSION` GitHub release. Publish those assets and `latest.json` only as a separate authorized release operation.
7. The app reads `releases/latest/download/latest.json`, selects the matching platform/architecture and verifies the downloaded package before offering installation. Never put GitHub credentials into a public desktop build.

Linux self-updates are for AppImage. DEB/RPM installations show package-manager guidance; they do not try to replace their running application through the AppImage updater. Apple Silicon/Intel macOS and Windows x64 have distinct manifests.

## In-app behavior

Configured builds check after startup if the last attempt was at least 24 hours ago, and reconsider hourly while open. Attempts are recorded before network access to avoid repeated checks on every offline startup. Users can disable automatic checks; manual checks remain available. Errors never become Up to date.

Downloading and verification happen inside About. The signature verifier is Tauri’s official updater. Release notes render as plain text. The verified package remains in native memory until installation, another check or app exit. Closing About does not close an active download. Download interruptions/errors can be retried. Installation is explicit, blocked by queued/running archive jobs, and holds the archive-job semaphore to prevent concurrent writes. Installation may show OS prompts or exit to the Windows installer; macOS/Linux restart after successful installation.

## Validation before publishing

Exercise upgrades between two real signed releases on Windows x64, macOS Apple Silicon/Intel and Linux AppImage. Test offline/missing feed, absent platform asset, invalid signature, interrupted download, retry, active-job installation denial, and restart behavior. A local UI preview or successful compilation does not verify live release distribution.

Reference: https://v2.tauri.app/plugin/updater/
