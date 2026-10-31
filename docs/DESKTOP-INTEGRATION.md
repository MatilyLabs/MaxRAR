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

### Windows 11 main menu

On Windows 11 the installer also registers a small signed package, `Matily.MaxRAR.ExplorerMenu`, so **MaxRAR** appears in the main right-click menu (no **Show more options** needed). It is a submenu:

- **Open in MaxRAR** and **Extract with MaxRAR…** for one supported archive.
- **Compress with MaxRAR…** for any files or folders (up to 256 items, multi-select works here).

How it works: `crates/maxrar-shell` builds `maxrar_shell.dll`, an `IExplorerCommand` handler that only launches `maxrar.exe` with the same `--compress` / `--extract` arguments as the classic verbs. Windows only shows these handlers for packaged apps, so `src-tauri/shell/windows/package/AppxManifest.xml` describes a *sparse* package whose files stay in the normal install folder. `scripts/build-explorer-menu.ps1` builds the DLL and the `.msix`; in releases it signs both with Azure Artifact Signing and copies the publisher from the signing certificate into the manifest (the two must match). The NSIS hooks run `Add-AppxPackage -ExternalLocation` after install on build 22000+ and `Remove-AppxPackage` before uninstall. If registration fails (for example an unsigned development build), the classic commands under **Show more options** still work.

The classic commands stay registered on purpose, as the fallback and for Windows 10. On Windows 11 they appear again under **Show more options**.

To test an unsigned build locally, turn on Developer Mode and register the unpacked layout instead of the `.msix`: `Add-AppxPackage -Register target\explorer-menu\layout\AppxManifest.xml -ExternalLocation <folder with maxrar.exe and maxrar_shell.dll>`. Windows refuses unsigned packaged apps from the `.msix` itself. Remove it with `Get-AppxPackage Matily.MaxRAR.ExplorerMenu | Remove-AppxPackage`.

### Windows notes

The direct classic context commands intentionally accept one selected item; Send To and the Windows 11 menu are the multi-selection routes. Portable users can enable single-item context commands with Help → Desktop integration. The NSIS installer additionally creates the Send To shortcut. Uninstall removes the app's own matching verb registrations, the menu package and the shortcut; default archive associations are handled by Tauri's existing installer configuration.

No machine-wide registration, administrator privileges or default-handler takeover is added. File associations and context verbs are separate.

## Handoff and testing

The native application accepts `--compress -- <absolute paths…>` and `--extract -- <absolute archive path>`. Each shell command launches a separate review window, avoiding replacement of an existing working window. Ordinary archive Open With behavior remains available.

`src-tauri/src/shell.rs` validates selections and installs the platform registration. Finder templates are generated from `scripts/generate-finder-actions.py`; `src-tauri/shell/windows/hooks.nsh` attaches registration/cleanup to the NSIS package. `scripts/test-windows-shell.ps1` tests installed command strings, Send To and cleanup, and passed in the Windows CI build for v0.1.0.

Local validation covers native argument parsing, template installation/quoting and creation with preselected sources. Installed Windows Explorer UI and macOS Finder menu visibility remain platform manual checks, separate from unit/build success.

RAR file associations and Windows extraction actions now accept `.rar` for supported single-volume RAR4/RAR5 archives. Reinstall the rebuilt application (or rerun desktop integration setup) to update registered actions. Multipart and self-extracting RAR archives remain unsupported.
