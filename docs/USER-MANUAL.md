# MaxRAR user manual

MaxRAR is a desktop archive workspace for Windows, macOS and Linux. This is a development build; available commands depend on the archive format. The application does not execute archive contents to preview or scan them.

## Open and browse

Use File → Open archive, or Ctrl+O / ⌘O. The file list and folder tree show the archive inventory. Find searches names and paths. Right-click a file or folder for its available commands; Shift+F10 opens the same menu from the keyboard.

Encrypted entries have a red lock badge. Filenames can remain visible while contents are encrypted. Protected previews ask explicitly for the archive password. Passwords are not saved to workspace preferences or copied CLI commands. Cancelling the password prompt discards its draft.

### RAR archives

Open a single-volume RAR4 or RAR5 archive as you would a ZIP. Browse, Extract To,
Test, text preview and local Scan are available for supported content. If headers
are encrypted, MaxRAR asks for the password before showing filenames. Visible
filenames alone do not mean encrypted file contents are accessible.

The decoder is bundled; no separate WinRAR or UnRAR installation is required.
Multipart archives and self-extracting RAR executables are not supported yet.
RAR creation and editing are unavailable. Very large dictionaries or decoding
jobs that exceed resource limits are reported as incomplete. Solid archives may
require decoding earlier files to reach your selection.

## Create an archive

Choose Add, select files/folders, choose a supported format and save location, then Create archive. ZIP and 7z support optional password protection. Raw compression streams accept one file; use TAR packages for folders or multiple files. MRAR is experimental: new archives use /1 and need this updated MaxRAR; legacy /0 archives still open. Keep originals and do not use it as a stable backup/exchange format. RAR creation is not supported.

## Extract

Extract To opens a destination review. Change chooses a folder; Details also permits editing the complete destination path. Existing-file choices appear when conflicts exist. Skip existing is the default; Keep both renames outputs and Replace explicitly overwrites eligible files. Blocked entries prevent extraction. The archive and destination are checked again before writing. Open folder when finished is optional.

## Test and Scan

Test checks readability and the checksums available in that format, without extracting files. Green means the supported integrity checks completed; it does not mean files are safe. Testing stops at the first error. Password, cancellation, resource limits and incomplete checks are shown separately from confirmed corruption. TAR has no per-file content checksum.

Scan runs bundled local structural/static inspection; it is not comprehensive antivirus protection or a safety guarantee. It does not upload archive contents to VirusTotal or require installing ClamAV. Right-click Scan selection limits inspection to the selected files/folder contents. Results identify that scope and unchecked coverage.

## Inspector and previews

The inspector summarizes actual inventory types and folder names, with bounded source hints where available. Explain folders and Find main file help navigate; they do not execute scripts, compile papers or load serialized models. Preview is bounded text, so binary formats may not be previewable.

## Help and updates

Help provides this manual, actual keyboard shortcuts, update checks, issue reporting and About MaxRAR. Third-party notices are available in the repository. About MaxRAR shows the installed version, project links and update status. Star opens the repository; Report an issue opens the issue chooser without submitting a report automatically.

Configured signed release builds check for updates at startup, at most once daily, with periodic checks while the app remains open. Uncheck Check automatically to disable those checks. Manual checks remain available. A failed connection is not reported as up to date.

Updates are downloaded and verified inside MaxRAR. Install & restart waits for active archive tasks to finish. Windows may show an installer or permission prompt. Linux self-updates use AppImage; DEB/RPM installations follow their package distribution. Development builds without an update signing key show that updates are not available yet.

## Keyboard controls

Ctrl / ⌘ O opens an archive, N creates one, F searches, and A selects all filtered files when the file list has focus. Enter opens a folder or previews a file; arrows navigate rows; Home/End jump to the first/last row; Shift+arrows extend selection. Escape closes menus/dialogs. See Help → Keyboard shortcuts for platform-specific key labels.

For problems, use [MaxRAR issues](https://github.com/MatilyLabs/MaxRAR/issues/new/choose). Review any logs or filenames before choosing to include them in a public report.

## Desktop right-click commands

On macOS, open Help → Desktop integration → Enable desktop actions, then use Finder Quick Actions / Services. On Windows, install the NSIS package and use Explorer’s Compress/Extract with MaxRAR commands (Show more options on Windows 11); use Send To for multiple compression sources. Both routes open review windows before writing files. See [Desktop integration](DESKTOP-INTEGRATION.md) for setup and testing.
