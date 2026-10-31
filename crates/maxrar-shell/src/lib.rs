//! Windows 11 File Explorer context menu for MaxRAR.
//!
//! Explorer's compact menu only lists `IExplorerCommand` handlers that belong to a
//! packaged app. The sparse package in `src-tauri/shell/windows/package` gives the
//! installed MaxRAR that identity and points Explorer at this DLL. Each command
//! only launches `maxrar.exe` with the same `--compress` / `--extract` handoff the
//! classic registry verbs use, so the review windows still validate everything.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// CLSID registered in the sparse package manifest.
pub const CLSID_TEXT: &str = "94ae9828-d27b-4b7d-9a32-c2e17040f695";

/// Same limit as the desktop handoff in `src-tauri/src/shell.rs`.
pub const MAX_SOURCES: usize = 256;

/// Extensions that get the extract commands, matching the classic registry verbs.
pub const ARCHIVE_EXTENSIONS: &[&str] = &[
    "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "zst", "zstd", "mrar", "tgz", "tbz", "tbz2",
    "txz", "tzst",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Open,
    Extract,
    Compress,
}

impl Action {
    pub const ALL: [Action; 3] = [Action::Open, Action::Extract, Action::Compress];

    pub fn title(self) -> &'static str {
        match self {
            Action::Open => "Open in MaxRAR",
            Action::Extract => "Extract with MaxRAR…",
            Action::Compress => "Compress with MaxRAR…",
        }
    }

    /// Whether the command applies to this selection.
    pub fn applies(self, selection: &[PathBuf]) -> bool {
        match self {
            Action::Open | Action::Extract => {
                selection.len() == 1 && selection[0].is_absolute() && is_archive(&selection[0])
            }
            Action::Compress => {
                !selection.is_empty()
                    && selection.len() <= MAX_SOURCES
                    && selection.iter().all(|p| p.is_absolute())
            }
        }
    }

    /// Arguments passed to `maxrar.exe`. Paths are separate arguments, never a shell string.
    pub fn args(self, selection: &[PathBuf]) -> Vec<OsString> {
        let mut args = Vec::with_capacity(selection.len() + 2);
        match self {
            Action::Open => {}
            Action::Extract => args.extend(["--extract".into(), "--".into()]),
            Action::Compress => args.extend(["--compress".into(), "--".into()]),
        }
        args.extend(selection.iter().map(|p| p.as_os_str().to_owned()));
        args
    }
}

pub fn is_archive(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ARCHIVE_EXTENSIONS.iter().any(|a| a.eq_ignore_ascii_case(e)))
}

/// Commands shown for a selection, in menu order.
pub fn visible_actions(selection: &[PathBuf]) -> Vec<Action> {
    Action::ALL
        .into_iter()
        .filter(|a| a.applies(selection))
        .collect()
}

#[cfg(windows)]
mod com;

#[cfg(test)]
mod tests {
    use super::*;

    fn abs(name: &str) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(format!("C:\\Users\\a\\{name}"))
        } else {
            PathBuf::from(format!("/home/a/{name}"))
        }
    }

    #[test]
    fn archives_get_open_extract_and_compress() {
        let sel = [abs("Photos.ZIP")];
        assert_eq!(
            visible_actions(&sel),
            [Action::Open, Action::Extract, Action::Compress]
        );
    }

    #[test]
    fn folders_and_multiple_items_only_compress() {
        assert_eq!(visible_actions(&[abs("report")]), [Action::Compress]);
        assert_eq!(
            visible_actions(&[abs("a.zip"), abs("b.rar")]),
            [Action::Compress]
        );
    }

    #[test]
    fn rejects_relative_empty_and_oversized_selections() {
        assert!(visible_actions(&[]).is_empty());
        assert!(visible_actions(&[PathBuf::from("a.zip")]).is_empty());
        let many: Vec<_> = (0..=MAX_SOURCES)
            .map(|i| abs(&format!("{i}.txt")))
            .collect();
        assert!(visible_actions(&many).is_empty());
    }

    #[test]
    fn handoff_keeps_paths_as_separate_arguments() {
        let sel = [abs("my \"odd\" file.rar")];
        let args = Action::Extract.args(&sel);
        assert_eq!(args[0], "--extract");
        assert_eq!(args[1], "--");
        assert_eq!(args[2], sel[0].as_os_str());
        assert_eq!(Action::Open.args(&sel), [sel[0].as_os_str()]);
    }
}
