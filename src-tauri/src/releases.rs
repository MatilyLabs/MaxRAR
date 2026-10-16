use crate::jobs::Jobs;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::Mutex;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Links {
    repository: String,
    manual: String,
    issues: String,
    organization: String,
    updater_endpoint: String,
}
fn links() -> Links {
    serde_json::from_str(include_str!("../../release.config.json"))
        .expect("Bundled release configuration is invalid")
}
fn public_key() -> Option<&'static str> {
    option_env!("MAXRAR_UPDATER_PUBLIC_KEY")
        .map(str::trim)
        .filter(|key| !key.is_empty())
}
fn supported() -> bool {
    !cfg!(target_os = "linux") || std::env::var_os("APPIMAGE").is_some()
}
fn require_updater() -> Result<(), String> {
    if !supported() {
        return Err("package_manager".into());
    }
    if public_key().is_none() {
        return Err("not_configured".into());
    }
    Ok(())
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    version: String,
    configured: bool,
    unavailable_reason: Option<String>,
    links: Links,
}
#[tauri::command]
pub fn release_info(app: AppHandle) -> ReleaseInfo {
    let reason = require_updater().err();
    ReleaseInfo {
        version: app.package_info().version.to_string(),
        configured: reason.is_none(),
        unavailable_reason: reason,
        links: links(),
    }
}
#[tauri::command]
pub fn open_project_link(app: AppHandle, destination: String) -> Result<(), String> {
    let links = links();
    let url = match destination.as_str() {
        "repository" => links.repository,
        "manual" => links.manual,
        "issues" => links.issues,
        "organization" => links.organization,
        _ => return Err("Unknown project link".into()),
    };
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}
#[derive(Default)]
pub struct Updates(Mutex<Session>);
#[derive(Default)]
struct Session {
    update: Option<Update>,
    bytes: Option<Vec<u8>>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    version: String,
    notes: String,
}
#[derive(Clone, Serialize)]
pub struct UpdateProgress {
    stage: &'static str,
    downloaded: u64,
    total: Option<u64>,
}
#[tauri::command]
pub async fn check_update(
    app: AppHandle,
    state: State<'_, Updates>,
) -> Result<Option<AvailableUpdate>, String> {
    require_updater()?;
    let mut session = state
        .0
        .try_lock()
        .map_err(|_| "An update operation is already running")?;
    // The Rust API is the only update entry point; no caller-supplied feed or key.
    let updater = app
        .updater_builder()
        .pubkey(public_key().unwrap())
        .endpoints(vec![links()
            .updater_endpoint
            .parse()
            .map_err(|e| format!("Invalid update endpoint: {e}"))?])
        .map_err(|e| e.to_string())?
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| e.to_string())?;
    let view = update.as_ref().map(|u| AvailableUpdate {
        version: u.version.clone(),
        notes: u.body.clone().unwrap_or_default(),
    });
    session.update = update;
    session.bytes = None;
    Ok(view)
}
#[tauri::command]
pub async fn download_update(app: AppHandle, state: State<'_, Updates>) -> Result<(), String> {
    require_updater()?;
    let mut session = state
        .0
        .try_lock()
        .map_err(|_| "An update operation is already running")?;
    let update = session.update.as_mut().ok_or("Check for an update first")?;
    update.timeout = Some(Duration::from_secs(600));
    let mut downloaded = 0u64;
    let mut last = Instant::now() - Duration::from_millis(100);
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                if last.elapsed() >= Duration::from_millis(100) {
                    last = Instant::now();
                    let _ = app.emit(
                        "maxrar://update-progress",
                        UpdateProgress {
                            stage: "downloading",
                            downloaded,
                            total,
                        },
                    );
                }
            },
            || {
                let _ = app.emit(
                    "maxrar://update-progress",
                    UpdateProgress {
                        stage: "verifying",
                        downloaded: 0,
                        total: None,
                    },
                );
            },
        )
        .await
        .map_err(|e| e.to_string())?;
    // download() returns bytes only after signature verification succeeds.
    session.bytes = Some(bytes);
    Ok(())
}
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    state: State<'_, Updates>,
    jobs: State<'_, Jobs>,
) -> Result<(), String> {
    require_updater()?;
    let _permit = jobs.idle_permit()?;
    let mut session = state
        .0
        .try_lock()
        .map_err(|_| "An update operation is already running")?;
    let bytes = session
        .bytes
        .as_ref()
        .ok_or("Download and verify the update first")?;
    session
        .update
        .as_ref()
        .ok_or("Check for an update first")?
        .install(bytes)
        .map_err(|e| e.to_string())?;
    session.bytes = None;
    // Windows' updater may already have exited to hand over to its installer.
    app.restart()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_links_are_fixed_to_maxrar() {
        let config = links();
        assert_eq!(config.repository, "https://github.com/MatilyLabs/MaxRAR");
        assert!(config.manual.starts_with(&config.repository));
        assert!(config.issues.starts_with(&config.repository));
        assert!(config
            .updater_endpoint
            .ends_with("/releases/latest/download/latest.json"));
    }
}
