# Desktop compression and extraction

Shell commands open review windows. They never immediately write, overwrite or delete selected files. Filenames are passed as argument arrays, not interpolated into a shell command. Up to 256 compression sources, or one archive for extraction, are accepted. Missing paths, relative paths and non-regular input objects are rejected. The archive core revalidates content when operations run.

## macOS

1. Move the latest MaxRAR.app into Applications and launch it.
2. Choose **Help → Desktop integration… → Enable desktop actions**.
3. Right-click a file/folder in Finder or on the Desktop, then choose **Quick Actions / Services → Compress with MaxRAR…**. Multiple selected files/folders enter a single Create Archive window.
4. For an archive, choose **Extract with MaxRAR…**. MaxRAR opens the archive and its extraction review. Password-protected archive headers prompt first; cancelling stops the handoff.

The two managed workflows are installed under `~/Library/Services/`. They reference the current app bundle location. After moving the app or replacing it at another path, use **Refresh desktop actions**. macOS controls whether services are enabled: check System Settings → Keyboard → Keyboard Shortcuts → Services if they are absent. Installation refreshes the services database but does not toggle those user preferences or restart Finder.

The service uses Apple's built-in Run Shell Script action and `/usr/bin/open`; no Python, separate archive utility, administrator access or broad filesystem permission is needed. Existing services with the same filenames but without MaxRAR's ownership marker are not overwritten. To remove the actions, remove only `Compress with MaxRAR.workflow` and `Extract with MaxRAR.workflow` from `~/Library/Services/` or disable them in Services settings.

## Windows

Use the NSIS installer. It registers these **per-user** Explorer commands:

- Compress with MaxRAR… for a file or folder.
- Extract with MaxRAR… for supported archive extensions.
- Send to → Compress with MaxRAR for multiple files/folders in one archive.

On Windows 11 the classic commands can appear under **Show more options**. The direct context commands intentionally accept one selected item; Send To is the multi-selection route. Portable users can enable single-item context commands with Help → Desktop integration. The NSIS installer additionally creates the Send To shortcut. Uninstall removes the app's own matching verb registrations and shortcut; default archive associations are handled by Tauri's existing installer configuration.

No COM extension is claimed for the Windows 11 compact menu. No machine-wide registration, administrator privileges or default-handler takeover is added. File associations and context verbs are separate.

## Handoff and testing

The native application accepts `--compress -- <absolute paths…>` and `--extract -- <absolute archive path>`. Each shell command launches a separate review window, avoiding replacement of an existing working window. Ordinary archive Open With behavior remains available.

`src-tauri/src/shell.rs` validates selections and installs the platform registration. Finder templates are generated from `scripts/generate-finder-actions.py`; `src-tauri/shell/windows/hooks.nsh` attaches registration/cleanup to the NSIS package. `scripts/test-windows-shell.ps1` tests installed command strings, Send To and cleanup, and passed in the Windows CI build for v0.1.0.

Local validation covers native argument parsing, template installation/quoting and creation with preselected sources. Installed Windows Explorer UI and macOS Finder menu visibility remain platform manual checks, separate from unit/build success.

RAR file associations and Windows extraction actions now accept `.rar` for supported single-volume RAR4/RAR5 archives. Reinstall the rebuilt application (or rerun desktop integration setup) to update registered actions. Multipart and self-extracting RAR archives remain unsupported.
