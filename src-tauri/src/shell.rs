use serde::Serialize;
#[cfg(any(target_os = "macos", test))]
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub path: String,
    pub r#type: &'static str,
}
#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub kind: &'static str,
    pub sources: Vec<Source>,
}
pub fn parse(
    args: impl IntoIterator<Item = std::ffi::OsString>,
) -> Result<Option<Request>, String> {
    let mut args = args.into_iter().skip(1);
    let Some(first) = args.next() else {
        return Ok(None);
    };
    let kind = match first.to_str() {
        Some("--compress") => "compress",
        Some("--extract") => "extract",
        _ => return Ok(None),
    };
    let paths: Vec<_> = args.collect();
    if paths.first().and_then(|v| v.to_str()) != Some("--") {
        return Err("Expected -- before selected paths".into());
    }
    if paths.len() < 2 || paths.len() > 257 || (kind == "extract" && paths.len() != 2) {
        return Err("Select files/folders to compress, or one archive to extract.".into());
    }
    let mut sources = Vec::new();
    for path in paths.into_iter().skip(1).map(PathBuf::from) {
        if !path.is_absolute() {
            return Err("Selected paths must be absolute".into());
        }
        let meta = std::fs::metadata(&path)
            .map_err(|_| format!("Selected item is unavailable: {}", path.display()))?;
        if !meta.is_file() && !meta.is_dir() || kind == "extract" && !meta.is_file() {
            return Err("Choose regular files or folders".into());
        }
        let path = path
            .into_os_string()
            .into_string()
            .map_err(|_| "This filename cannot be represented in the interface")?;
        if !sources.iter().any(|s: &Source| s.path == path) {
            sources.push(Source {
                path,
                r#type: if meta.is_dir() { "folder" } else { "file" },
            });
        }
    }
    Ok(Some(Request { kind, sources }))
}
#[tauri::command]
pub fn initial_shell_request() -> Result<Option<Request>, String> {
    parse(std::env::args_os())
}
#[derive(Serialize)]
pub struct Status {
    platform: &'static str,
    available: bool,
    installed: bool,
    message: String,
}
#[cfg(target_os = "macos")]
fn bundle() -> Result<PathBuf, String> {
    std::env::current_exe()
        .map_err(|e| e.to_string())?
        .ancestors()
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .map(Path::to_path_buf)
        .ok_or("Run the packaged MaxRAR.app to enable Finder actions.".into())
}
#[cfg(target_os = "macos")]
fn services() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(|p| PathBuf::from(p).join("Library/Services"))
        .ok_or("Home folder unavailable".into())
}
#[cfg(any(target_os = "macos", test))]
fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
#[cfg(any(target_os = "macos", test))]
fn install_finder(root: &Path, app: &Path) -> Result<(), String> {
    let app = app.to_str().ok_or("Unsupported application path")?;
    let quoted = format!("'{}'", app.replace('\'', "'\\''"));
    for (name, info, workflow) in [
        (
            "Compress with MaxRAR.workflow",
            include_str!("../shell/macos/compress/Info.plist"),
            include_str!("../shell/macos/compress/document.wflow"),
        ),
        (
            "Extract with MaxRAR.workflow",
            include_str!("../shell/macos/extract/Info.plist"),
            include_str!("../shell/macos/extract/document.wflow"),
        ),
    ] {
        let folder = root.join(name).join("Contents");
        if folder.exists()
            && !std::fs::read_to_string(folder.join("Info.plist"))
                .unwrap_or_default()
                .contains("app.maxrar.desktop.shell.v1")
        {
            return Err(format!("An unmanaged action already exists: {name}"));
        }
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        std::fs::write(folder.join("Info.plist"), info).map_err(|e| e.to_string())?;
        std::fs::write(
            folder.join("document.wflow"),
            workflow.replace("__MAXRAR_APP__", &xml(&quoted)),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[cfg(target_os = "windows")]
fn reg(args: &[&str]) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let exe = PathBuf::from(std::env::var_os("SystemRoot").ok_or("SystemRoot unavailable")?)
        .join("System32/reg.exe");
    let out = std::process::Command::new(exe)
        .args(args)
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}
#[cfg(target_os = "windows")]
fn install_windows() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_str().ok_or("Unsupported application path")?;
    for (class, verb, title, mode) in [
        ("*", "MaxRAR.Compress", "Compress with MaxRAR…", "compress"),
        (
            "Directory",
            "MaxRAR.Compress",
            "Compress with MaxRAR…",
            "compress",
        ),
        (
            "SystemFileAssociations\\.zip",
            "MaxRAR.Extract",
            "Extract with MaxRAR…",
            "extract",
        ),
        (
            "SystemFileAssociations\\.rar",
            "MaxRAR.Extract",
            "Extract with MaxRAR…",
            "extract",
        ),
        (
            "SystemFileAssociations\\.7z",
            "MaxRAR.Extract",
            "Extract with MaxRAR…",
            "extract",
        ),
        (
            "SystemFileAssociations\\.tar",
            "MaxRAR.Extract",
            "Extract with MaxRAR…",
            "extract",
        ),
        (
            "SystemFileAssociations\\.gz",
            "MaxRAR.Extract",
            "Extract with MaxRAR…",
            "extract",
        ),
        (
            "SystemFileAssociations\\.bz2",
            "MaxRAR.Extract",
            "Extract with MaxRAR…",
            "extract",
        ),
        (
            "SystemFileAssociations\\.xz",
            "MaxRAR.Extract",
            "Extract with MaxRAR…",
            "extract",
        ),
        (
            "SystemFileAssociations\\.zst",
            "MaxRAR.Extract",
            "Extract with MaxRAR…",
            "extract",
        ),
        (
            "SystemFileAssociations\\.mrar",
            "MaxRAR.Extract",
            "Extract with MaxRAR…",
            "extract",
        ),
        (
            "SystemFileAssociations\\.tgz",
            "MaxRAR.Extract",
            "Extract with MaxRAR…",
            "extract",
        ),
        (
            "SystemFileAssociations\\.tbz",
            "MaxRAR.Extract",
            "Extract with MaxRAR…",
            "extract",
        ),
        (
            "SystemFileAssociations\\.tbz2",
            "MaxRAR.Extract",
            "Extract with MaxRAR…",
            "extract",
        ),
        (
            "SystemFileAssociations\\.txz",
            "MaxRAR.Extract",
            "Extract with MaxRAR…",
            "extract",
        ),
        (
            "SystemFileAssociations\\.tzst",
            "MaxRAR.Extract",
            "Extract with MaxRAR…",
            "extract",
        ),
        (
            "SystemFileAssociations\\.zstd",
            "MaxRAR.Extract",
            "Extract with MaxRAR…",
            "extract",
        ),
    ] {
        let key = format!("HKCU\\Software\\Classes\\{class}\\shell\\{verb}");
        reg(&["add", &key, "/ve", "/d", title, "/f"])?;
        reg(&[
            "add",
            &key,
            "/v",
            "Icon",
            "/d",
            &format!("\"{exe}\",0"),
            "/f",
        ])?;
        reg(&["add", &key, "/v", "MultiSelectModel", "/d", "Single", "/f"])?;
        reg(&[
            "add",
            &format!("{key}\\command"),
            "/ve",
            "/d",
            &format!("\"{exe}\" --{mode} -- \"%1\""),
            "/f",
        ])?;
    }
    Ok(())
}
#[tauri::command]
pub fn shell_integration_status() -> Status {
    #[cfg(target_os = "macos")]
    {
        Status{platform:"macOS",available:bundle().is_ok(),installed:services().is_ok_and(|p|["Compress with MaxRAR.workflow","Extract with MaxRAR.workflow"].iter().all(|n|p.join(n).join("Contents/document.wflow").is_file())),message:"Finder → Quick Actions / Services. If needed, enable these actions in System Settings → Keyboard → Keyboard Shortcuts → Services.".into()}
    }
    #[cfg(target_os = "windows")]
    {
        Status{platform:"Windows",available:true,installed:reg(&["query","HKCU\\Software\\Classes\\*\\shell\\MaxRAR.Compress"]).is_ok(),message:"Explorer → right-click → MaxRAR. Installed releases add MaxRAR to the Windows 11 main menu; the classic commands are under Show more options.".into()}
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Status {
            platform: "Linux",
            available: false,
            installed: false,
            message: "Desktop menu registration is currently available on macOS and Windows."
                .into(),
        }
    }
}
pub fn install() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        install_finder(&services()?, &bundle()?)?;
        let _ = std::process::Command::new("/System/Library/CoreServices/pbs")
            .arg("-update")
            .status();
        Ok("Finder actions installed. Enable them in Services settings if they do not appear immediately.".into())
    }
    #[cfg(target_os = "windows")]
    {
        install_windows()?;
        Ok("Explorer commands enabled for your user account.".into())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err("Desktop menu registration is unsupported on this platform".into())
    }
}
#[tauri::command]
pub fn install_shell_integration() -> Result<String, String> {
    install()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_selection_without_shell_interpretation() {
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("a ' $(literal).txt");
        std::fs::write(&file, b"sample").unwrap();
        let args = vec![
            "maxrar".into(),
            "--compress".into(),
            "--".into(),
            file.clone().into_os_string(),
            d.path().into(),
        ];
        let r = parse(args).unwrap().unwrap();
        assert_eq!(r.sources.len(), 2);
        assert_eq!(r.sources[0].path, file.to_str().unwrap());
        assert_eq!(r.sources[1].r#type, "folder");
        assert!(parse([
            "maxrar".into(),
            "--extract".into(),
            "--".into(),
            d.path().into()
        ])
        .is_err());
        assert!(parse([
            "maxrar".into(),
            "--compress".into(),
            "--".into(),
            "relative".into()
        ])
        .is_err());
    }
    #[test]
    fn finder_templates_quote_bundle_location() {
        let d = tempfile::tempdir().unwrap();
        install_finder(d.path(), Path::new("/Applications/MaxRAR's & tools.app")).unwrap();
        let text = std::fs::read_to_string(
            d.path()
                .join("Compress with MaxRAR.workflow/Contents/document.wflow"),
        )
        .unwrap();
        assert!(text.contains("--compress --"));
        assert!(!text.contains("__MAXRAR_APP__"));
        assert!(text.contains("&amp;"));
        install_finder(d.path(), Path::new("/Applications/MaxRAR.app")).unwrap();
    }
}
