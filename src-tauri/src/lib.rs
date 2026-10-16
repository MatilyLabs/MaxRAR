mod jobs;
mod promotions;
mod releases;
mod scanner;
pub mod shell;
use jobs::Jobs;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, State};
#[cfg(target_os = "macos")]
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;
#[derive(Default)]
struct LaunchPath(Mutex<Option<String>>);
use wonrar_core::{
    ArchiveInfo, ConflictPolicy, ExtractionPlan, Limits, Preview, Progress, Receipt,
};

#[derive(Deserialize)]
struct PlanRequest {
    path: String,
    archive_identity: String,
    destination: String,
    #[serde(default)]
    entry_ids: Vec<String>,
    #[serde(default)]
    policy: ConflictPolicy,
    #[serde(default)]
    limits: Limits,
    password: Option<String>,
}
#[derive(Deserialize)]
struct CreateRequest {
    output: String,
    inputs: Vec<String>,
    password: Option<String>,
}
#[derive(Deserialize)]
struct TestRequest {
    path: String,
    password: Option<String>,
    archive_identity: String,
}
#[derive(Deserialize)]
struct DeleteRequest {
    path: String,
    archive_identity: String,
    entry_ids: Vec<String>,
}
#[derive(Deserialize)]
struct CommentRequest {
    path: String,
    archive_identity: String,
    comment: String,
}

fn forward(callback: &(dyn Fn(f64, String) + Send + Sync), p: Progress) {
    let fraction = if p.total_bytes > 0 {
        p.bytes_processed as f64 / p.total_bytes as f64
    } else if p.total_entries > 0 {
        p.completed_entries as f64 / p.total_entries as f64
    } else {
        0.0
    };
    callback(fraction, p.entry);
}

#[tauri::command]
async fn inspect_archive(path: String, password: Option<String>) -> Result<ArchiveInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        wonrar_core::inspect_with_password(Path::new(&path), password.as_deref())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn preview_entry(
    path: String,
    entry_id: String,
    max_bytes: Option<u64>,
    password: Option<String>,
) -> Result<Preview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        wonrar_core::preview_with_password(
            Path::new(&path),
            &entry_id,
            max_bytes.unwrap_or(256 * 1024),
            password.as_deref(),
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn ask_archive(
    path: String,
    question: String,
    model: Option<String>,
) -> Result<wonrar_inspect::Answer, String> {
    tauri::async_runtime::spawn_blocking(move || {
        wonrar_inspect::ask(Path::new(&path), &question, model.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn plan_extraction(request: PlanRequest) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let plan = wonrar_core::plan_with_password(
            Path::new(&request.path),
            Path::new(&request.destination),
            &request.entry_ids,
            request.policy,
            request.limits,
            request.password.as_deref(),
        )
        .map_err(|e| e.to_string())?;
        if plan.archive_identity != request.archive_identity {
            return Err("Archive changed since opening. Reopen it before selecting files.".into());
        }
        let exists = Path::new(&plan.destination).is_dir();
        let mut view = serde_json::to_value(plan).map_err(|e| e.to_string())?;
        view["destination_exists"] = serde_json::json!(exists);
        view["requested_destination"] = serde_json::json!(request.destination);
        Ok(view)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn extract_archive(
    app: AppHandle,
    state: State<'_, Jobs>,
    request: ExtractionPlan,
    job_id: String,
    password: Option<String>,
    open_after: Option<bool>,
) -> Result<Receipt, String> {
    let mut receipt = state
        .run(app.clone(), job_id, "extract", move |cancel, callback| {
            wonrar_core::extract_with_password(
                &request,
                &cancel,
                &|p| forward(&*callback, p),
                password.as_deref(),
            )
            .map_err(|e| e.to_string())
        })
        .await?;
    if open_after.unwrap_or(false) {
        if let Some(destination) = receipt.destination.as_deref() {
            // Open only the completed operation's directory, never an archive member.
            let opened = std::fs::canonicalize(destination)
                .map_err(|e| e.to_string())
                .and_then(|path| {
                    if !path.is_dir() {
                        return Err("Extraction destination is no longer a folder".into());
                    }
                    app.opener()
                        .open_path(path.to_string_lossy(), None::<&str>)
                        .map_err(|e| e.to_string())
                });
            if opened.is_err() {
                receipt
                    .message
                    .push_str(" Folder could not be opened; files were extracted successfully.");
            }
        }
    }
    Ok(receipt)
}
#[tauri::command]
async fn create_archive(
    app: AppHandle,
    state: State<'_, Jobs>,
    request: CreateRequest,
    job_id: String,
) -> Result<ArchiveInfo, String> {
    state
        .run(app, job_id, "create", move |cancel, callback| {
            wonrar_core::create_with_password(
                Path::new(&request.output),
                &request.inputs.iter().map(PathBuf::from).collect::<Vec<_>>(),
                &cancel,
                &|p| forward(&*callback, p),
                request.password.as_deref(),
            )
            .map_err(|e| e.to_string())
        })
        .await
}
#[tauri::command]
async fn test_archive(
    app: AppHandle,
    state: State<'_, Jobs>,
    request: TestRequest,
    job_id: String,
) -> Result<Receipt, String> {
    state
        .run(app, job_id, "test", move |cancel, callback| {
            let identity = wonrar_core::inspect_with_password(
                Path::new(&request.path),
                request.password.as_deref(),
            )
            .map_err(|e| e.to_string())?
            .identity;
            if identity != request.archive_identity {
                return Err("Archive changed. Reopen it before testing.".into());
            }
            let receipt = wonrar_core::test_with_password(
                Path::new(&request.path),
                &cancel,
                &|p| forward(&*callback, p),
                request.password.as_deref(),
            )
            .map_err(|e| e.to_string())?;
            let current = wonrar_core::inspect_with_password(
                Path::new(&request.path),
                request.password.as_deref(),
            )
            .map_err(|e| e.to_string())?
            .identity;
            if current != identity {
                return Err("Archive changed during the test. Reopen it and test again.".into());
            }
            Ok(receipt)
        })
        .await
}
#[tauri::command]
async fn delete_entries(
    app: AppHandle,
    state: State<'_, Jobs>,
    request: DeleteRequest,
    job_id: String,
) -> Result<ArchiveInfo, String> {
    state
        .run(app, job_id, "delete", move |cancel, _| {
            wonrar_core::delete_entries_checked(
                Path::new(&request.path),
                &request.entry_ids,
                &request.archive_identity,
                &cancel,
            )
            .map_err(|e| e.to_string())
        })
        .await
}
#[tauri::command]
async fn update_comment(
    app: AppHandle,
    state: State<'_, Jobs>,
    request: CommentRequest,
    job_id: String,
) -> Result<ArchiveInfo, String> {
    state
        .run(app, job_id, "comment", move |cancel, _| {
            wonrar_core::set_comment_checked(
                Path::new(&request.path),
                &request.comment,
                &request.archive_identity,
                &cancel,
            )
            .map_err(|e| e.to_string())
        })
        .await
}
#[tauri::command]
fn cancel_job(state: State<'_, Jobs>, job_id: String) -> Result<bool, String> {
    state.cancel(&job_id)
}

#[tauri::command]
fn scanner_status() -> scanner::ScannerStatus {
    scanner::status()
}

#[tauri::command]
async fn scan_archive(
    app: AppHandle,
    state: State<'_, Jobs>,
    path: String,
    password: Option<String>,
    job_id: String,
    entry_ids: Option<Vec<String>>,
    expected_identity: Option<String>,
) -> Result<scanner::ScanReport, String> {
    state
        .run(app, job_id, "scan", move |cancel, callback| {
            callback(0.0, "Built-in local static inspection".into());
            scanner::scan(
                Path::new(&path),
                password.as_deref(),
                entry_ids.as_deref(),
                expected_identity.as_deref(),
                &cancel,
                &|p| forward(&*callback, p),
            )
        })
        .await
}

fn launch_file_path(args: impl IntoIterator<Item = std::ffi::OsString>) -> Option<String> {
    args.into_iter()
        .skip(1)
        .map(PathBuf::from)
        .find(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned())
}

#[tauri::command]
fn initial_archive_path(state: State<'_, LaunchPath>) -> Option<String> {
    if let Ok(mut pending) = state.0.lock() {
        if let Some(path) = pending.take() {
            return Some(path);
        }
    }
    launch_file_path(std::env::args_os())
}

#[tauri::command]
async fn research_profile(
    path: String,
    password: Option<String>,
) -> Result<wonrar_inspect::research::ResearchProfile, String> {
    tauri::async_runtime::spawn_blocking(move || {
        wonrar_inspect::research::profile(Path::new(&path), password.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(releases::Updates::default())
        .manage(promotions::Promotions::default())
        .manage(Jobs::default())
        .manage(LaunchPath::default())
        .invoke_handler(tauri::generate_handler![
            releases::release_info,
            promotions::promotion_feed,
            promotions::open_promotion,
            releases::open_project_link,
            releases::check_update,
            releases::download_update,
            releases::install_update,
            inspect_archive,
            preview_entry,
            ask_archive,
            plan_extraction,
            extract_archive,
            create_archive,
            test_archive,
            delete_entries,
            update_comment,
            cancel_job,
            research_profile,
            scanner_status,
            scan_archive,
            initial_archive_path,
            shell::initial_shell_request,
            shell::shell_integration_status,
            shell::install_shell_integration
        ])
        .build(tauri::generate_context!())
        .expect("MaxRAR could not start its desktop runtime")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = event {
                if let Some(path) = urls
                    .into_iter()
                    .filter_map(|url| url.to_file_path().ok())
                    .find(|path| path.is_file())
                {
                    let path = path.to_string_lossy().into_owned();
                    if let Ok(mut pending) = app.state::<LaunchPath>().0.lock() {
                        *pending = Some(path.clone());
                    }
                    let _ = app.emit("wonrar://open-archive", path);
                }
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}

#[cfg(test)]
mod startup_tests {
    use super::*;
    #[test]
    fn launch_accepts_tar_and_extensionless_archive_paths() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["paper.tar", "arxiv-source"] {
            let path = dir.path().join(name);
            std::fs::write(&path, b"archive bytes validated by core later").unwrap();
            assert_eq!(
                launch_file_path([
                    std::ffi::OsString::from("maxrar"),
                    path.clone().into_os_string()
                ]),
                Some(path.to_string_lossy().into_owned())
            );
        }
    }
}
