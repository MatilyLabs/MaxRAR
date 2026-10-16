//! Built-in offline scan adapter; process supervision is shared with the CLI.
use serde::Serialize;
use std::{path::Path, sync::atomic::AtomicBool};
use wonrar_core::Progress;
pub use wonrar_scan::ScanReport;
#[derive(Serialize)]
pub struct ScannerStatus {
    pub available: bool,
    pub engine: String,
    pub engine_version: String,
    pub rule_version: String,
    pub message: String,
}
pub fn status() -> ScannerStatus {
    ScannerStatus {available:true,engine:wonrar_scan::ENGINE.into(),engine_version:wonrar_scan::ENGINE_VERSION.into(),rule_version:wonrar_scan::RULE_VERSION.into(),message:"Built-in offline static inspection. No separate scanner installation or online submission.".into()}
}
pub fn scan(
    path: &Path,
    password: Option<&str>,
    entry_ids: Option<&[String]>,
    expected_identity: Option<&str>,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
) -> Result<ScanReport, String> {
    wonrar_scan::worker::scan_reviewed(
        path,
        password,
        entry_ids,
        expected_identity,
        cancel,
        progress,
    )
}
