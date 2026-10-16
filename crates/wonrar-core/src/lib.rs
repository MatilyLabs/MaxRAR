//! Archive operations shared by the desktop and automation clients.
//! No archive content is executed. All extraction paths are capability-relative.
mod mrar;
mod rar;
pub mod rar_worker;
mod sevenz;
mod streams;
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use tempfile::NamedTempFile;
use unicode_normalization::UnicodeNormalization;
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("Operation cancelled; completed files are retained, unfinished files removed.")]
    Cancelled,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub compressed_size: u64,
    pub is_dir: bool,
    pub is_encrypted: bool,
    pub modified: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub entry_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    pub can_create: bool,
    pub can_extract: bool,
    pub can_test: bool,
    pub can_delete: bool,
    pub can_comment: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveInfo {
    pub path: String,
    pub name: String,
    pub format: String,
    pub identity: String,
    pub entries: Vec<Entry>,
    pub total_size: u64,
    pub compressed_size: u64,
    pub comment: String,
    pub findings: Vec<Finding>,
    pub capabilities: Capabilities,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preview {
    pub entry_id: String,
    pub text: String,
    pub truncated: bool,
    pub bytes_read: u64,
}
/// Bounded raw decoded entry bytes. A truncated result has incomplete checksum
/// and authentication coverage and must be reported as partial inspection.
#[derive(Debug, Clone)]
pub struct EntryBytes {
    pub bytes: Vec<u8>,
    pub truncated: bool,
}

fn read_entry_prefix(
    reader: &mut dyn Read,
    max_bytes: u64,
    cancel: &AtomicBool,
) -> Result<EntryBytes> {
    check(cancel)?;
    let limit = max_bytes.min(16 * 1024 * 1024);
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 65536];
    while bytes.len() as u64 <= limit {
        check(cancel)?;
        let remaining = (limit + 1 - bytes.len() as u64).min(buffer.len() as u64) as usize;
        let n = reader.read(&mut buffer[..remaining])?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..n]);
    }
    let truncated = bytes.len() as u64 > limit;
    bytes.truncate(limit as usize);
    Ok(EntryBytes { bytes, truncated })
}

/// Read inert entry bytes for built-in inspection, never deserializing content.
/// The returned allocation is capped at 16 MiB. Sequential/solid formats may
/// decode preceding bytes under their separate bounded traversal budget.
pub fn inspect_entry_bytes(
    path: &Path,
    entry_id: &str,
    max_bytes: u64,
    password: Option<&str>,
    cancel: &AtomicBool,
) -> Result<EntryBytes> {
    check(cancel)?;
    if rar::detect(path)? {
        return rar::entry_bytes(path, entry_id, max_bytes, password, cancel);
    }
    if mrar::detect(path)? {
        return mrar::entry_bytes(path, entry_id, max_bytes, cancel);
    }
    if is_sevenz(path)? {
        return sevenz::entry_bytes(path, entry_id, max_bytes, password, cancel);
    }
    if let Some(kind) = streams::detect(path)? {
        return streams::entry_bytes(path, kind, entry_id, max_bytes, cancel);
    }
    let mut archive = open(path)?;
    let index = entry_id.parse().map_err(|_| invalid("Invalid entry ID"))?;
    let mut entry = match password {
        Some(secret) => archive.by_index_decrypt(index, secret.as_bytes())?,
        None => archive.by_index(index)?,
    };
    if entry.is_dir() {
        return Err(invalid("Directories do not contain entry bytes"));
    }
    read_entry_prefix(&mut entry, max_bytes, cancel)
}
/// Bounded byte inspection tied to a reviewed archive identity. MRAR reuses one
/// verified decoded group; other formats use the existing per-entry readers.
/// The caller must still recheck archive identity at the end of its operation.
pub struct EntryInspectionSession<'a> {
    path: &'a Path,
    password: Option<&'a str>,
    cancel: &'a AtomicBool,
    mrar: Option<mrar::InspectionSession>,
}
impl<'a> EntryInspectionSession<'a> {
    pub fn open(
        path: &'a Path,
        password: Option<&'a str>,
        identity: &str,
        cancel: &'a AtomicBool,
    ) -> Result<Self> {
        check(cancel)?;
        let mrar = if mrar::detect(path)? {
            Some(mrar::InspectionSession::open(path, identity, cancel)?)
        } else {
            if inspect_with_password(path, password)?.identity != identity {
                return Err(invalid("Archive changed before byte inspection"));
            }
            None
        };
        Ok(Self {
            path,
            password,
            cancel,
            mrar,
        })
    }
    pub fn read(&mut self, entry_id: &str, max_bytes: u64) -> Result<EntryBytes> {
        check(self.cancel)?;
        match &mut self.mrar {
            Some(reader) => reader.read(entry_id, max_bytes, self.cancel),
            None => inspect_entry_bytes(self.path, entry_id, max_bytes, self.password, self.cancel),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    #[default]
    Skip,
    KeepBoth,
    Replace,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limits {
    pub max_entries: usize,
    pub max_file_bytes: u64,
    pub max_total_bytes: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            max_file_bytes: 2 * 1024 * 1024 * 1024,
            max_total_bytes: 8 * 1024 * 1024 * 1024,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlannedEntry {
    pub entry_id: String,
    pub source_name: String,
    pub output_name: String,
    pub size: u64,
    pub is_dir: bool,
    pub conflict: bool,
    pub action: String,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionPlan {
    pub archive_path: String,
    pub archive_identity: String,
    pub destination: String,
    pub entry_ids: Vec<String>,
    pub policy: ConflictPolicy,
    pub limits: Limits,
    pub entries: Vec<PlannedEntry>,
    pub total_bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    pub operation: String,
    pub entry: String,
    pub completed_entries: usize,
    pub total_entries: usize,
    pub bytes_processed: u64,
    pub total_bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub operation: String,
    pub destination: Option<String>,
    pub written: Vec<String>,
    pub skipped: Vec<String>,
    pub bytes_processed: u64,
    pub message: String,
}

fn check(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}
fn invalid(s: impl Into<String>) -> Error {
    Error::Invalid(s.into())
}
fn absolute(path: &Path) -> Result<PathBuf> {
    Ok(if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    })
}
fn fingerprint(file: &mut File) -> Result<String> {
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(0))?;
    let mut digest = Sha256::new();
    let mut buf = [0; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        digest.update(&buf[..n]);
    }
    file.seek(SeekFrom::Start(0))?;
    Ok(format!("{:x}", digest.finalize()))
}
fn open(path: &Path) -> Result<ZipArchive<File>> {
    let mut file = File::open(path)?;
    let mut magic = [0; 6];
    let n = file.read(&mut magic)?;
    if magic[..n].starts_with(b"Rar!") {
        return Err(invalid(
            "Unsupported RAR signature or version. Use a supported single-volume RAR4 or RAR5 archive.",
        ));
    }
    if magic[..n].starts_with(b"7z\xbc\xaf") {
        return Err(invalid(
            "7z must be opened through the 7z adapter, not the ZIP backend.",
        ));
    }
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(0))?;
    Ok(ZipArchive::new(file)?)
}

/// Portable conservative output names, rejecting traversal, Windows devices and streams.
fn safe_name(name: &str) -> Result<String> {
    // POSIX TAR producers commonly prefix ordinary members with "./".
    // Remove only leading current-directory components; traversal is still rejected.
    let mut name = name;
    while let Some(rest) = name.strip_prefix("./") {
        name = rest;
    }
    if name.is_empty()
        || name.starts_with('/')
        || name.contains('\\')
        || name.contains(':')
        || name.contains('\0')
    {
        return Err(invalid("Absolute, ambiguous or platform-unsafe path"));
    }
    let clean = name.trim_end_matches('/');
    if clean.is_empty() {
        return Err(invalid("Empty path"));
    }
    for c in clean.split('/') {
        let stem = c.split('.').next().unwrap_or("").to_ascii_uppercase();
        if c.is_empty()
            || c == "."
            || c == ".."
            || c.ends_with([' ', '.'])
            || c.chars().any(|v| v.is_control() || "<>\"|?*".contains(v))
            || [
                "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
                "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8",
                "LPT9",
            ]
            .contains(&stem.as_str())
        {
            return Err(invalid(
                "Traversal, reserved or non-portable path component",
            ));
        }
    }
    Ok(clean.to_string())
}
fn metadata_info(
    path: &Path,
    identity: String,
    mut archive: ZipArchive<File>,
) -> Result<ArchiveInfo> {
    if archive.len() > Limits::default().max_entries {
        return Err(invalid("Archive exceeds inventory limit (100000 entries)."));
    }
    let mut entries = Vec::new();
    let mut findings = Vec::new();
    let mut names = HashSet::new();
    let mut total = 0u64;
    for i in 0..archive.len() {
        let f = archive.by_index_raw(i)?;
        let name = f.name()?.to_string();
        let id = i.to_string();
        let mut add = |code: &str, message: &str, severity: &str| {
            findings.push(Finding {
                severity: severity.into(),
                code: code.into(),
                message: message.into(),
                entry_id: Some(id.clone()),
            })
        };
        if safe_name(&name).is_err() {
            add(
                "unsafe_path",
                "Entry cannot be extracted: unsafe or non-portable path.",
                "error",
            );
        }
        if f.is_symlink()
            || f.unix_mode().is_some_and(|m| {
                let t = m & 0o170000;
                t != 0 && t != 0o100000 && t != 0o040000
            })
        {
            add(
                "special_entry",
                "Symbolic links and special files are blocked.",
                "error",
            );
        }
        if !names.insert(name_key(name.trim_end_matches('/'))) {
            add(
                "duplicate_path",
                "Path collides with another entry under case-insensitive comparison.",
                "warning",
            );
        }
        if f.encrypted() {
            add(
                "encrypted",
                "Encrypted contents require a password. Filenames remain visible in ZIP.",
                "warning",
            );
        }
        let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        if [
            "exe", "msi", "sh", "bat", "cmd", "ps1", "app", "scr", "dll", "js", "vbs",
        ]
        .contains(&ext.as_str())
        {
            add("executable_content", "Executable or script entry. This is an inventory signal, not a malware determination.", "warning");
        }
        if f.compressed_size() > 0 && f.size() / f.compressed_size() > 1000 {
            add(
                "high_expansion",
                "Declared expansion ratio exceeds 1000:1.",
                "warning",
            );
        }
        total = total
            .checked_add(f.size())
            .ok_or_else(|| invalid("Archive declared size overflow"))?;
        entries.push(Entry {
            id,
            name,
            size: f.size(),
            compressed_size: f.compressed_size(),
            is_dir: f.is_dir(),
            is_encrypted: f.encrypted(),
            modified: f.last_modified().map(|v| v.to_string()),
        });
    }
    findings.push(Finding { severity: "info".into(), code: "inspection_coverage".into(), message: "Inventory and path checks only. No malware scan or AI safety verdict has been performed.".into(), entry_id: None });
    Ok(ArchiveInfo {
        path: absolute(path)?.to_string_lossy().into(),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        format: "zip".into(),
        identity,
        entries,
        total_size: total,
        compressed_size: fs::metadata(path)?.len(),
        comment: String::from_utf8_lossy(archive.comment()).into(),
        findings,
        capabilities: Capabilities {
            can_create: true,
            can_extract: true,
            can_test: true,
            can_delete: true,
            can_comment: true,
        },
    })
}
fn is_sevenz(path: &Path) -> Result<bool> {
    let mut b = [0; 6];
    let n = File::open(path)?.read(&mut b)?;
    Ok(n == 6 && b == *b"7z\xbc\xaf\x27\x1c")
}
pub fn inspect(path: &Path) -> Result<ArchiveInfo> {
    inspect_with_password(path, None)
}
pub fn inspect_with_password(path: &Path, password: Option<&str>) -> Result<ArchiveInfo> {
    if rar::detect(path)? {
        return rar::inspect(path, password);
    }
    if mrar::detect(path)? {
        return mrar::inspect(path);
    }
    if is_sevenz(path)? {
        return sevenz::inspect(path, password);
    }
    if let Some(kind) = streams::detect(path)? {
        return streams::inspect(path, kind);
    }
    let mut file = File::open(path)?;
    let identity = fingerprint(&mut file)?;
    metadata_info(path, identity, open(path)?)
}

pub fn preview(path: &Path, entry_id: &str, max_bytes: u64) -> Result<Preview> {
    preview_with_password(path, entry_id, max_bytes, None)
}
pub fn preview_with_password(
    path: &Path,
    entry_id: &str,
    max_bytes: u64,
    password: Option<&str>,
) -> Result<Preview> {
    let flag = AtomicBool::new(false);
    let raw = inspect_entry_bytes(path, entry_id, max_bytes.min(1024 * 1024), password, &flag)?;
    let mut bytes = raw.bytes;
    let truncated = raw.truncated;
    if bytes.contains(&0) {
        return Err(invalid("Binary contents: text preview unavailable"));
    }
    if truncated {
        if let Err(e) = std::str::from_utf8(&bytes) {
            if e.error_len().is_none() {
                bytes.truncate(e.valid_up_to());
            }
        }
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| invalid("Preview is not UTF-8 text"))?
        .to_string();
    Ok(Preview {
        entry_id: entry_id.into(),
        text,
        truncated,
        bytes_read: bytes.len() as u64,
    })
}

fn name_key(name: &str) -> String {
    name.nfc().flat_map(char::to_lowercase).collect()
}
fn exists(root: Option<&Dir>, path: &str) -> bool {
    root.is_some_and(|r| r.symlink_metadata(path).is_ok())
}
fn map_directory(name: &str, mappings: &[(String, String)]) -> String {
    mappings
        .iter()
        .rev()
        .find_map(|(old, new)| {
            if name == old {
                Some(new.clone())
            } else {
                name.strip_prefix(&format!("{old}/"))
                    .map(|tail| format!("{new}/{tail}"))
            }
        })
        .unwrap_or_else(|| name.to_string())
}
fn selected(info: &ArchiveInfo, ids: &[String]) -> Result<HashSet<String>> {
    let wanted: HashSet<_> = ids.iter().cloned().collect();
    let known: HashSet<_> = info.entries.iter().map(|e| &e.id).collect();
    if let Some(id) = wanted.iter().find(|v| !known.contains(v)) {
        return Err(invalid(format!("Unknown entry ID: {id}")));
    }
    Ok(if wanted.is_empty() {
        info.entries.iter().map(|e| e.id.clone()).collect()
    } else {
        wanted
    })
}
pub fn plan(
    path: &Path,
    destination: &Path,
    entry_ids: &[String],
    policy: ConflictPolicy,
    limits: Limits,
) -> Result<ExtractionPlan> {
    plan_with_password(path, destination, entry_ids, policy, limits, None)
}
pub fn plan_with_password(
    path: &Path,
    destination: &Path,
    entry_ids: &[String],
    policy: ConflictPolicy,
    limits: Limits,
    password: Option<&str>,
) -> Result<ExtractionPlan> {
    let info = inspect_with_password(path, password)?;
    let wanted = selected(&info, entry_ids)?;
    let special: HashSet<_> = info
        .findings
        .iter()
        .filter(|f| f.code == "special_entry")
        .filter_map(|f| f.entry_id.as_ref())
        .collect();
    if wanted.len() > limits.max_entries {
        return Err(invalid("Selection exceeds entry limit"));
    }
    if streams::detect(path)?.is_some()
        && !wanted.is_empty()
        && (info.total_size > limits.max_total_bytes
            || info.entries.iter().any(|e| e.size > limits.max_file_bytes))
    {
        return Err(invalid("Sequential archive traversal exceeds the selected decoded-byte limits, including skipped contents"));
    }
    let dest = absolute(destination)?;
    let root = if dest.exists() {
        Some(Dir::open_ambient_dir(&dest, ambient_authority())?)
    } else {
        None
    };
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    let mut total = 0u64;
    // Resolve directory collisions first, including parents omitted from ZIP inventory.
    let mut dirs = std::collections::BTreeSet::new();
    for e in info.entries.iter().filter(|e| wanted.contains(&e.id)) {
        if let Ok(name) = safe_name(&e.name) {
            let mut parts: Vec<_> = name.split('/').collect();
            if !e.is_dir {
                parts.pop();
            }
            for n in 1..=parts.len() {
                dirs.insert(parts[..n].join("/"));
            }
        }
    }
    let mut mappings: Vec<(String, String)> = Vec::new();
    for dir in &dirs {
        let mapped = map_directory(dir, &mappings);
        if exists(root.as_ref(), &mapped)
            && !root
                .as_ref()
                .is_some_and(|r| r.symlink_metadata(&mapped).is_ok_and(|m| m.is_dir()))
            && policy == ConflictPolicy::KeepBoth
        {
            let mut n = 2;
            let mut alternative = format!("{mapped} ({n})");
            while exists(root.as_ref(), &alternative)
                || mappings
                    .iter()
                    .any(|(_, v)| name_key(v) == name_key(&alternative))
            {
                n += 1;
                alternative = format!("{mapped} ({n})");
            }
            mappings.push((dir.clone(), alternative));
        }
    }
    let required_dirs: HashSet<_> = dirs
        .iter()
        .map(|v| name_key(&map_directory(v, &mappings)))
        .collect();
    for e in info.entries.iter().filter(|e| wanted.contains(&e.id)) {
        let mut reason = safe_name(&e.name).err().map(|v| v.to_string());
        if e.is_encrypted && password.is_none() {
            reason = Some("Password required for encrypted entry".into());
        }
        if special.contains(&e.id) {
            reason = Some("Links and special files are blocked".into());
        }
        if e.size > limits.max_file_bytes {
            reason = Some("Entry exceeds file size limit".into());
        }
        let normalized =
            safe_name(&e.name).unwrap_or_else(|_| e.name.trim_end_matches('/').to_string());
        let original = normalized.as_str();
        // A directory mapping applies to a file's parent, never to an identically
        // named file that happens to collide with that directory.
        let mut output = if e.is_dir {
            map_directory(original, &mappings)
        } else if let Some((parent, leaf)) = original.rsplit_once('/') {
            format!("{}/{leaf}", map_directory(parent, &mappings))
        } else {
            original.to_string()
        };
        let internal_dir_collision = !e.is_dir && required_dirs.contains(&name_key(&output));
        let mut ancestor_obstruction = None;
        let parts: Vec<_> = output.split('/').collect();
        let parent_count = if e.is_dir {
            parts.len()
        } else {
            parts.len().saturating_sub(1)
        };
        if reason.is_none() {
            for depth in 1..=parent_count {
                let parent = parts[..depth].join("/");
                if let Some(meta) = root.as_ref().and_then(|r| r.symlink_metadata(&parent).ok()) {
                    if !meta.is_dir() {
                        ancestor_obstruction = Some(parent);
                        break;
                    }
                }
            }
        }
        let conflict = exists(root.as_ref(), &output)
            || seen.contains(&name_key(&output))
            || internal_dir_collision
            || ancestor_obstruction.is_some()
            || output != original;
        let mut action = if reason.is_some() {
            "blocked"
        } else {
            "extract"
        }
        .to_string();
        if let Some(parent) = ancestor_obstruction {
            action = if policy == ConflictPolicy::Skip {
                "skip"
            } else {
                "blocked"
            }
            .into();
            reason = Some(format!("Destination ancestor '{parent}' is a file or link; WonRaR will not replace ancestors."));
        }
        if conflict && reason.is_none() {
            if e.is_dir
                && root
                    .as_ref()
                    .is_some_and(|r| r.metadata(&output).is_ok_and(|m| m.is_dir()))
            { /* merge directory */
            } else {
                match policy {
                    ConflictPolicy::Skip => {
                        action = "skip".into();
                        if internal_dir_collision {
                            reason = Some(
                                "Entry path is required as a directory by another selected entry"
                                    .into(),
                            );
                        }
                    }
                    ConflictPolicy::Replace => {
                        if internal_dir_collision {
                            action = "blocked".into();
                            reason = Some(
                                "Entry path is required as a directory by another selected entry"
                                    .into(),
                            );
                        } else if !e.is_dir
                            && root.as_ref().is_some_and(|r| {
                                r.symlink_metadata(&output).is_ok_and(|m| m.is_dir())
                            })
                        {
                            action = "blocked".into();
                            reason = Some("A file cannot replace an existing directory".into());
                        } else if seen.contains(&name_key(&output)) {
                            action = "blocked".into();
                            reason = Some("Duplicate archive output path".into());
                        }
                    }
                    ConflictPolicy::KeepBoth => {
                        let original = output.clone();
                        let mut index = 2;
                        while exists(root.as_ref(), &output)
                            || seen.contains(&name_key(&output))
                            || (!e.is_dir && required_dirs.contains(&name_key(&output)))
                        {
                            output = format!("{original} ({index})");
                            index += 1;
                        }
                    }
                }
            }
        }
        seen.insert(name_key(&output));
        if action == "extract" {
            total = total
                .checked_add(e.size)
                .ok_or_else(|| invalid("Size overflow"))?;
        }
        entries.push(PlannedEntry {
            entry_id: e.id.clone(),
            source_name: e.name.clone(),
            output_name: output,
            size: e.size,
            is_dir: e.is_dir,
            conflict,
            action,
            reason,
        });
    }
    if total > limits.max_total_bytes {
        return Err(invalid("Selection exceeds total extraction limit"));
    }
    Ok(ExtractionPlan {
        archive_path: info.path,
        archive_identity: info.identity,
        destination: dest.to_string_lossy().into(),
        entry_ids: entry_ids.to_vec(),
        policy,
        limits,
        entries,
        total_bytes: total,
    })
}

fn copy_limited(
    reader: &mut (impl Read + ?Sized),
    writer: &mut impl Write,
    limit: u64,
    cancel: &AtomicBool,
    on_bytes: &mut dyn FnMut(u64),
) -> Result<u64> {
    let mut total = 0u64;
    let mut buf = [0; 65536];
    loop {
        check(cancel)?;
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        total = total
            .checked_add(n as u64)
            .ok_or_else(|| invalid("Size overflow"))?;
        if total > limit {
            return Err(invalid("Actual decoded data exceeds extraction limit"));
        }
        writer.write_all(&buf[..n])?;
        on_bytes(total);
    }
    Ok(total)
}

pub fn extract(
    reviewed: &ExtractionPlan,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
) -> Result<Receipt> {
    extract_with_password(reviewed, cancel, progress, None)
}
pub fn extract_with_password(
    reviewed: &ExtractionPlan,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
    password: Option<&str>,
) -> Result<Receipt> {
    check(cancel)?;
    let current = plan_with_password(
        Path::new(&reviewed.archive_path),
        Path::new(&reviewed.destination),
        &reviewed.entry_ids,
        reviewed.policy,
        reviewed.limits.clone(),
        password,
    )?;
    if current.archive_identity != reviewed.archive_identity {
        return Err(invalid(
            "Archive changed after planning. Review a new plan.",
        ));
    }
    if current.entries != reviewed.entries {
        return Err(invalid(
            "Destination or plan changed. Review a new extraction plan.",
        ));
    }
    fs::create_dir_all(&reviewed.destination)?;
    let root = Dir::open_ambient_dir(&reviewed.destination, ambient_authority())?;
    if rar::detect(Path::new(&reviewed.archive_path))? {
        return rar::extract(reviewed, &root, cancel, progress, password);
    }
    if mrar::detect(Path::new(&reviewed.archive_path))? {
        return mrar::extract(reviewed, &root, cancel, progress);
    }
    if is_sevenz(Path::new(&reviewed.archive_path))? {
        return sevenz::extract(reviewed, &root, cancel, progress, password);
    }
    if let Some(kind) = streams::detect(Path::new(&reviewed.archive_path))? {
        return streams::extract(
            Path::new(&reviewed.archive_path),
            kind,
            reviewed,
            &root,
            cancel,
            progress,
        );
    }
    let mut a = open(Path::new(&reviewed.archive_path))?;
    // Pin the actual file descriptor to the reviewed content before decoding.
    let mut pinned = a.into_inner();
    if fingerprint(&mut pinned)? != reviewed.archive_identity {
        return Err(invalid("Archive changed before extraction"));
    }
    a = ZipArchive::new(pinned)?;
    let mut result = Receipt {
        operation: "extract".into(),
        destination: Some(reviewed.destination.clone()),
        written: vec![],
        skipped: vec![],
        bytes_processed: 0,
        message: String::new(),
    };
    for (count, e) in reviewed.entries.iter().enumerate() {
        check(cancel)?;
        if e.action != "extract" {
            result.skipped.push(e.source_name.clone());
            continue;
        }
        let index = e
            .entry_id
            .parse()
            .map_err(|_| invalid("Invalid entry ID"))?;
        let mut f = match password {
            Some(p) => a.by_index_decrypt(index, p.as_bytes())?,
            None => a.by_index(index)?,
        };
        let copied = write_planned(
            &root,
            e,
            &mut f,
            reviewed,
            cancel,
            progress,
            count,
            result.bytes_processed,
        )?;
        result.bytes_processed += copied;
        result.written.push(e.output_name.clone());
    }
    result.message = format!(
        "Extracted {} entries; skipped {}. No malware scan was performed.",
        result.written.len(),
        result.skipped.len()
    );
    Ok(result)
}

// Keep the reviewed policy, capability root and live cancellation/progress
// inputs explicit at this security-sensitive write boundary.
#[allow(clippy::too_many_arguments)]
fn write_planned(
    root: &Dir,
    e: &PlannedEntry,
    reader: &mut dyn Read,
    reviewed: &ExtractionPlan,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
    count: usize,
    before: u64,
) -> Result<u64> {
    let name = safe_name(&e.output_name)?;
    if e.is_dir {
        root.create_dir_all(&name)?;
        return Ok(0);
    }
    if let Some(parent) = Path::new(&name)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        root.create_dir_all(parent)?;
    }
    let temp_name = format!(
        ".wonrar-{}-{}-{}.part",
        std::process::id(),
        count,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut out = root.open_with(&temp_name, &options)?;
    let remaining = reviewed
        .limits
        .max_total_bytes
        .saturating_sub(before)
        .min(reviewed.limits.max_file_bytes);
    let copied = copy_limited(reader, &mut out, remaining, cancel, &mut |bytes| {
        progress(Progress {
            operation: "extract".into(),
            entry: e.source_name.clone(),
            completed_entries: count,
            total_entries: reviewed.entries.len(),
            bytes_processed: before + bytes,
            total_bytes: reviewed.total_bytes,
        })
    });
    let commit = (|| -> Result<u64> {
        let n = copied?;
        if n != e.size {
            return Err(invalid("Decoded size differs from reviewed size"));
        }
        out.sync_all()?;
        drop(out);
        check(cancel)?;
        if reviewed.policy == ConflictPolicy::Replace {
            root.rename(&temp_name, root, &name)?;
        } else {
            root.hard_link(&temp_name, root, &name)?;
        }
        Ok(n)
    })();
    let _ = root.remove_file(&temp_name);
    commit
}

pub fn test(path: &Path, cancel: &AtomicBool, progress: &dyn Fn(Progress)) -> Result<Receipt> {
    test_with_password(path, cancel, progress, None)
}
pub fn test_with_password(
    path: &Path,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
    password: Option<&str>,
) -> Result<Receipt> {
    if rar::detect(path)? {
        return rar::test(path, cancel, progress, password);
    }
    if mrar::detect(path)? {
        return mrar::test(path, cancel, progress);
    }
    if is_sevenz(path)? {
        return sevenz::test(path, cancel, progress, password);
    }
    if let Some(kind) = streams::detect(path)? {
        return streams::test(path, kind, cancel, progress);
    }
    let info = inspect(path)?;
    let mut a = open(path)?;
    let limits = Limits::default();
    let mut total = 0u64;
    let mut written = Vec::new();
    for (i, e) in info.entries.iter().enumerate() {
        check(cancel)?;
        let mut f = match password {
            Some(p) => a.by_index_decrypt(i, p.as_bytes())?,
            None => a.by_index(i)?,
        };
        let before = total;
        let n = copy_limited(
            &mut f,
            &mut std::io::sink(),
            limits
                .max_file_bytes
                .min(limits.max_total_bytes.saturating_sub(total)),
            cancel,
            &mut |n| {
                progress(Progress {
                    operation: "test".into(),
                    entry: e.name.clone(),
                    completed_entries: i,
                    total_entries: info.entries.len(),
                    bytes_processed: before + n,
                    total_bytes: info.total_size,
                })
            },
        )?;
        total += n;
        written.push(e.name.clone());
    }
    Ok(Receipt {
        operation: "test".into(),
        destination: None,
        written,
        skipped: vec![],
        bytes_processed: total,
        message: "ZIP payloads decoded and CRC checked. This is not a malware scan.".into(),
    })
}

pub fn create(
    output: &Path,
    inputs: &[PathBuf],
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
) -> Result<ArchiveInfo> {
    create_with_password(output, inputs, cancel, progress, None)
}
pub fn create_with_password(
    output: &Path,
    inputs: &[PathBuf],
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
    password: Option<&str>,
) -> Result<ArchiveInfo> {
    check(cancel)?;
    if password == Some("") {
        return Err(invalid("Encryption password cannot be empty"));
    }
    if inputs.is_empty() {
        return Err(invalid("Select at least one source file or folder"));
    }
    if output
        .extension()
        .is_some_and(|v| v.eq_ignore_ascii_case("mrar"))
    {
        return mrar::create(output, inputs, cancel, progress, password);
    }
    if let Some(kind) = streams::output_kind(output) {
        return streams::create(output, kind, inputs, cancel, progress, password);
    }
    if output
        .extension()
        .is_some_and(|v| v.eq_ignore_ascii_case("7z"))
    {
        return sevenz::create(output, inputs, cancel, progress, password);
    }
    if output
        .extension()
        .is_none_or(|v| !v.eq_ignore_ascii_case("zip"))
    {
        return Err(invalid(
            "Unsupported output format. Choose ZIP, 7z, TAR, gzip, bzip2, XZ or Zstandard.",
        ));
    }
    let output = absolute(output)?;
    let parent = output
        .parent()
        .ok_or_else(|| invalid("Output has no parent"))?;
    let output = parent.canonicalize()?.join(
        output
            .file_name()
            .ok_or_else(|| invalid("No output filename"))?,
    );
    let parent = output.parent().ok_or_else(|| invalid("No output parent"))?;
    let mut staged = NamedTempFile::new_in(parent)?;
    let mut writer = ZipWriter::new(staged.as_file_mut());
    let mut sources = Vec::new();
    let mut names = HashSet::new();
    for input in inputs {
        let input = absolute(input)?;
        if fs::symlink_metadata(&input)?.file_type().is_symlink() {
            return Err(invalid("Source links are unsupported"));
        }
        let input = input.canonicalize()?;
        if output.starts_with(&input) || output == input {
            return Err(invalid(
                "Output archive cannot be inside an input folder or equal an input",
            ));
        }
        let base = input
            .parent()
            .ok_or_else(|| invalid("Filesystem roots cannot be added"))?;
        for item in walkdir::WalkDir::new(&input).follow_links(false) {
            let item = item.map_err(|e| invalid(e.to_string()))?;
            if item.file_type().is_symlink()
                || (!item.file_type().is_dir() && !item.file_type().is_file())
            {
                return Err(invalid("Source links and special files are not supported"));
            }
            let name = item
                .path()
                .strip_prefix(base)
                .map_err(|e| invalid(e.to_string()))?
                .components()
                .map(|v| {
                    v.as_os_str()
                        .to_str()
                        .ok_or_else(|| invalid("Source filenames must be UTF-8"))
                })
                .collect::<Result<Vec<_>>>()?
                .join("/");
            safe_name(&name)?;
            if !names.insert(name_key(&name)) {
                return Err(invalid(
                    "Input names collide; choose distinct top-level names",
                ));
            }
            sources.push((item.path().to_owned(), name, item.file_type().is_dir()));
            if sources.len() > Limits::default().max_entries {
                return Err(invalid("Too many source entries"));
            }
        }
    }
    let count = sources.len();
    let mut total = 0u64;
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let options = match password {
        Some(p) => options.with_aes_encryption(zip::AesMode::Aes256, p),
        None => options,
    };
    for (i, (path, name, dir)) in sources.into_iter().enumerate() {
        check(cancel)?;
        if dir {
            writer.add_directory(format!("{name}/"), options)?;
        } else {
            writer.start_file(&name, options)?;
            let mut file = File::open(path)?;
            let before = total;
            total += copy_limited(
                &mut file,
                &mut writer,
                Limits::default()
                    .max_file_bytes
                    .min(Limits::default().max_total_bytes.saturating_sub(total)),
                cancel,
                &mut |n| {
                    progress(Progress {
                        operation: "create".into(),
                        entry: name.clone(),
                        completed_entries: i,
                        total_entries: count,
                        bytes_processed: before + n,
                        total_bytes: 0,
                    })
                },
            )?;
        }
    }
    writer.finish()?;
    staged.as_file_mut().sync_all()?;
    check(cancel)?;
    test_with_password(staged.path(), cancel, &|_| {}, password)?;
    staged
        .persist_noclobber(&output)
        .map_err(|e| invalid(format!("Archive was not published: {}", e.error)))?;
    inspect(&output)
}

fn mutation_lock(path: &Path) -> Result<File> {
    let path = absolute(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| invalid("No archive parent"))?
        .canonicalize()?;
    let name = path
        .file_name()
        .ok_or_else(|| invalid("No archive filename"))?
        .to_string_lossy();
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(parent.join(format!(".{name}.wonrar.lock")))?;
    fs2::FileExt::try_lock_exclusive(&file)
        .map_err(|_| invalid("Archive is being modified by another WonRaR operation"))?;
    Ok(file)
}
fn edit(
    path: &Path,
    deleted: &[String],
    comment: Option<&str>,
    expected_identity: Option<&str>,
    cancel: &AtomicBool,
) -> Result<ArchiveInfo> {
    check(cancel)?;
    if mrar::detect(path)? {
        return Err(invalid("Experimental MRAR/0 does not support editing or comments. Create a new archive instead."));
    }
    if streams::detect(path)?.is_some() {
        return Err(invalid("TAR and compression stream editing/comments are unsupported. Create a new archive instead."));
    }
    if is_sevenz(path)? {
        return Err(invalid(
            "7z editing and comments are not supported. Create a new archive instead.",
        ));
    }
    if rar::detect(path)? {
        return Err(invalid(
            "RAR editing and creation are unsupported; extract or create a ZIP/7z archive instead.",
        ));
    }
    let _lock = mutation_lock(path)?;
    let info = inspect(path)?;
    if expected_identity.is_some_and(|expected| expected != info.identity) {
        return Err(invalid(
            "Archive changed since it was opened. Reopen it and review the entries before editing.",
        ));
    }
    let ids = if deleted.is_empty() {
        HashSet::new()
    } else {
        selected(&info, deleted)?
    };
    if comment.is_some_and(|v| v.len() > 65535) {
        return Err(invalid("ZIP comments are limited to 65535 bytes"));
    }
    let path = absolute(path)?;
    let mut staged = NamedTempFile::new_in(
        path.parent()
            .ok_or_else(|| invalid("No parent directory"))?,
    )?;
    let mut source = File::open(&path)?;
    if fingerprint(&mut source)? != info.identity {
        return Err(invalid(
            "Archive changed before editing; original was preserved",
        ));
    }
    let mut a = ZipArchive::new(source)?;
    let mut writer = ZipWriter::new(staged.as_file_mut());
    writer.set_comment(comment.unwrap_or(&info.comment))?;
    for i in 0..a.len() {
        check(cancel)?;
        if !ids.contains(&i.to_string()) {
            writer.raw_copy_file(a.by_index_raw(i)?)?;
        }
    }
    writer.finish()?;
    staged.as_file_mut().sync_all()?;
    check(cancel)?;
    let mut source = a.into_inner();
    if fingerprint(&mut source)? != info.identity {
        return Err(invalid(
            "Archive changed while copying; original was preserved",
        ));
    }
    drop(source);
    // Reopen the staged central directory; raw copying deliberately preserves encrypted data.
    inspect(staged.path())?;
    if inspect(&path)?.identity != info.identity {
        return Err(invalid(
            "Archive changed while editing; original was preserved",
        ));
    }
    staged
        .persist(&path)
        .map_err(|e| invalid(format!("Archive edit was not published: {}", e.error)))?;
    inspect(&path)
}
pub fn delete_entries(
    path: &Path,
    entry_ids: &[String],
    cancel: &AtomicBool,
) -> Result<ArchiveInfo> {
    if entry_ids.is_empty() {
        return Err(invalid("Select entries to delete"));
    }
    edit(path, entry_ids, None, None, cancel)
}
pub fn set_comment(path: &Path, comment: &str, cancel: &AtomicBool) -> Result<ArchiveInfo> {
    edit(path, &[], Some(comment), None, cancel)
}
/// Delete IDs from a specific reviewed inventory. The identity comparison is
/// performed while holding the mutation lock, before writing any staged archive.
pub fn delete_entries_checked(
    path: &Path,
    entry_ids: &[String],
    expected_identity: &str,
    cancel: &AtomicBool,
) -> Result<ArchiveInfo> {
    if entry_ids.is_empty() {
        return Err(invalid("Select entries to delete"));
    }
    edit(path, entry_ids, None, Some(expected_identity), cancel)
}
/// Update the comment only if the archive still matches the reviewed inventory.
pub fn set_comment_checked(
    path: &Path,
    comment: &str,
    expected_identity: &str,
    cancel: &AtomicBool,
) -> Result<ArchiveInfo> {
    edit(path, &[], Some(comment), Some(expected_identity), cancel)
}
