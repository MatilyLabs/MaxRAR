//! RAR adapter. Native parsing/decoding happens only in a supervised child.
use super::*;
use crate::rar_worker::{self, Frame, Operation, Request};
use tempfile::TempDir;

pub(super) fn detect(path: &Path) -> Result<bool> {
    let mut signature = [0; 8];
    let n = File::open(path)?.read(&mut signature)?;
    Ok((n >= 7 && signature[..7] == *b"Rar!\x1a\x07\x00")
        || (n == 8 && signature == *b"Rar!\x1a\x07\x01\x00"))
}

struct Snapshot {
    directory: TempDir,
    path: PathBuf,
    identity: String,
}
struct HashingWriter<'a> {
    file: &'a mut File,
    digest: Sha256,
}
impl Write for HashingWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let n = self.file.write(bytes)?;
        self.digest.update(&bytes[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
impl Snapshot {
    fn new(path: &Path, expected: Option<&str>, cancel: &AtomicBool) -> Result<Self> {
        let directory = tempfile::Builder::new().prefix("maxrar-rar-").tempdir()?;
        let snapshot = directory.path().join("archive.rar");
        let mut source = File::open(path)?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .open(&snapshot)?;
        // Pin a private, immutable-to-the-source snapshot; no native decoder opens user paths.
        let mut writer = HashingWriter {
            file: &mut output,
            digest: Sha256::new(),
        };
        copy_limited(
            &mut source,
            &mut writer,
            Limits::default().max_total_bytes,
            cancel,
            &mut |_| {},
        )?;
        let identity = format!("{:x}", writer.digest.finalize());
        if expected.is_some_and(|v| v != identity) {
            return Err(invalid("Archive changed before RAR decoding"));
        }
        Ok(Self {
            directory,
            path: snapshot,
            identity,
        })
    }
    fn request(&self, operation: Operation, password: Option<&str>, limits: Limits) -> Request {
        Request {
            parent_pid: std::process::id(),
            path: self.path.clone(),
            directory: self.directory.path().to_owned(),
            password: password.map(str::to_owned),
            operation,
            limits,
        }
    }
}

pub(super) fn inspect(path: &Path, password: Option<&str>) -> Result<ArchiveInfo> {
    inspect_cancellable(path, password, &AtomicBool::new(false))
}
fn inspect_cancellable(
    path: &Path,
    password: Option<&str>,
    cancel: &AtomicBool,
) -> Result<ArchiveInfo> {
    let snapshot = Snapshot::new(path, None, cancel)?;
    let mut items = None;
    rar_worker::run(
        snapshot.request(Operation::List, password, Limits::default()),
        cancel,
        |frame| {
            if let Frame::Inventory(value) = frame {
                if items.is_some() {
                    return Err(invalid("Duplicate RAR inventory"));
                }
                items = Some(value);
            }
            Ok(())
        },
    )?;
    let items = items.ok_or_else(|| invalid("RAR worker returned no inventory"))?;
    let mut entries = Vec::new();
    let mut findings = Vec::new();
    let mut seen = HashSet::new();
    let mut total_size = 0u64;
    for item in items {
        let e = item.entry;
        let mut finding = |code: &str, message: &str, severity: &str| {
            findings.push(Finding {
                severity: severity.into(),
                code: code.into(),
                message: message.into(),
                entry_id: Some(e.id.clone()),
            })
        };
        if safe_name(&e.name).is_err() {
            finding(
                "unsafe_path",
                "Entry has an unsafe or non-portable path.",
                "error",
            );
        }
        if item.special {
            finding(
                "special_entry",
                "Links, redirections and special files are blocked.",
                "error",
            );
        }
        if !seen.insert(name_key(e.name.trim_end_matches('/'))) {
            finding(
                "duplicate_path",
                "Paths collide under normalized case-insensitive comparison.",
                "warning",
            );
        }
        if e.is_encrypted {
            finding(
                "encrypted",
                "Content is encrypted and requires a password.",
                "warning",
            );
        }
        let ext = e.name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        if [
            "exe", "msi", "sh", "bat", "cmd", "ps1", "app", "scr", "dll", "js", "vbs",
        ]
        .contains(&ext.as_str())
        {
            finding(
                "executable_content",
                "Executable or script entry; this is not a malware determination.",
                "warning",
            );
        }
        total_size = total_size
            .checked_add(e.size)
            .ok_or_else(|| invalid("RAR declared size overflow"))?;
        entries.push(e);
    }
    findings.push(Finding { severity: "info".into(), code: "inspection_coverage".into(), entry_id: None,
        message: "Single-volume RAR4/RAR5 inventory. Payload checks require Test. Solid reads may decode preceding entries. Decoder runs in a supervised worker (120 seconds, sampled 768 MiB RSS threshold, 256 MiB dictionary cap); this is not a filesystem/network sandbox. Multipart and SFX RAR are unsupported.".into() });
    Ok(ArchiveInfo {
        path: absolute(path)?.to_string_lossy().into(),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        format: "rar".into(),
        identity: snapshot.identity.clone(),
        entries,
        total_size,
        compressed_size: fs::metadata(&snapshot.path)?.len(),
        comment: String::new(),
        findings,
        capabilities: Capabilities {
            can_create: false,
            can_extract: true,
            can_test: true,
            can_delete: false,
            can_comment: false,
        },
    })
}

pub(super) fn entry_bytes(
    path: &Path,
    id: &str,
    max: u64,
    password: Option<&str>,
    cancel: &AtomicBool,
) -> Result<EntryBytes> {
    let id = id.parse().map_err(|_| invalid("Invalid RAR entry ID"))?;
    let max = max.min(16 * 1024 * 1024);
    let snapshot = Snapshot::new(path, None, cancel)?;
    let mut result = None;
    rar_worker::run(
        snapshot.request(
            Operation::Prefix { id, limit: max },
            password,
            Limits::default(),
        ),
        cancel,
        |frame| {
            if let Frame::Ready {
                id: found,
                bytes,
                truncated,
            } = frame
            {
                if found != id || result.is_some() || bytes > max {
                    return Err(invalid("Invalid RAR preview result"));
                }
                let mut file = File::open(snapshot.directory.path().join(id.to_string()))?;
                let mut data = Vec::new();
                (&mut file).take(max + 1).read_to_end(&mut data)?;
                if data.len() as u64 != bytes {
                    return Err(invalid("RAR preview size mismatch"));
                }
                result = Some(EntryBytes {
                    bytes: data,
                    truncated,
                });
            }
            Ok(())
        },
    )?;
    result.ok_or_else(|| invalid("RAR entry is a directory or could not be read"))
}

pub(super) fn extract(
    p: &ExtractionPlan,
    root: &Dir,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
    password: Option<&str>,
) -> Result<Receipt> {
    let snapshot = Snapshot::new(
        Path::new(&p.archive_path),
        Some(&p.archive_identity),
        cancel,
    )?;
    let selected: std::collections::HashMap<usize, &PlannedEntry> = p
        .entries
        .iter()
        .filter(|e| e.action == "extract")
        .map(|e| {
            Ok((
                e.entry_id
                    .parse()
                    .map_err(|_| invalid("Invalid RAR entry ID"))?,
                e,
            ))
        })
        .collect::<Result<_>>()?;
    let mut receipt = Receipt {
        operation: "extract".into(),
        destination: Some(p.destination.clone()),
        written: vec![],
        skipped: p
            .entries
            .iter()
            .filter(|e| e.action != "extract")
            .map(|e| e.source_name.clone())
            .collect(),
        bytes_processed: 0,
        message: String::new(),
    };
    if !selected.is_empty() {
        let mut seen = HashSet::new();
        rar_worker::run(
            snapshot.request(
                Operation::Decode {
                    ids: selected.keys().copied().collect(),
                },
                password,
                p.limits.clone(),
            ),
            cancel,
            |frame| {
                match frame {
                    Frame::Progress { id, bytes } => progress(Progress {
                        operation: "extract".into(),
                        entry: selected
                            .get(&id)
                            .map_or("Decoding solid archive", |e| e.source_name.as_str())
                            .into(),
                        completed_entries: receipt.written.len(),
                        total_entries: selected.len(),
                        bytes_processed: bytes,
                        total_bytes: p.total_bytes,
                    }),
                    Frame::Ready {
                        id,
                        bytes,
                        truncated,
                    } => {
                        let e = selected
                            .get(&id)
                            .ok_or_else(|| invalid("Unexpected RAR entry"))?;
                        if !seen.insert(id) || truncated || bytes != e.size {
                            return Err(invalid("RAR output differs from reviewed entry"));
                        }
                        let file = snapshot.directory.path().join(id.to_string());
                        let mut reader: Box<dyn Read> = if e.is_dir {
                            Box::new(std::io::empty())
                        } else {
                            Box::new(File::open(&file)?)
                        };
                        let n = write_planned(
                            root,
                            e,
                            &mut reader,
                            p,
                            cancel,
                            progress,
                            receipt.written.len(),
                            receipt.bytes_processed,
                        )?;
                        drop(reader);
                        if !e.is_dir {
                            fs::remove_file(file)?;
                        }
                        receipt.bytes_processed += n;
                        receipt.written.push(e.output_name.clone());
                    }
                    _ => {}
                }
                Ok(())
            },
        )?;
        if seen.len() != selected.len() {
            return Err(invalid("RAR decoder did not return all selected entries"));
        }
    }
    receipt.message = format!(
        "Extracted {} entries; skipped {}. No malware scan was performed.",
        receipt.written.len(),
        receipt.skipped.len()
    );
    Ok(receipt)
}

pub(super) fn test(
    path: &Path,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
    password: Option<&str>,
) -> Result<Receipt> {
    check(cancel)?;
    progress(Progress {
        operation: "test".into(),
        entry: "Preparing RAR archive".into(),
        completed_entries: 0,
        total_entries: 0,
        bytes_processed: 0,
        total_bytes: 0,
    });
    let info = inspect_cancellable(path, password, cancel)?;
    let snapshot = Snapshot::new(path, Some(&info.identity), cancel)?;
    let mut receipt = Receipt {
        operation: "test".into(),
        destination: None,
        written: vec![],
        skipped: vec![],
        bytes_processed: 0,
        message: "RAR payloads decoded and available checksums checked. Not a malware scan.".into(),
    };
    rar_worker::run(
        snapshot.request(Operation::Test, password, Limits::default()),
        cancel,
        |frame| {
            match frame {
                Frame::Ready {
                    id,
                    bytes,
                    truncated: false,
                } => {
                    let e = info
                        .entries
                        .get(id)
                        .ok_or_else(|| invalid("Unexpected RAR test entry"))?;
                    if bytes != e.size {
                        return Err(invalid("RAR test size mismatch"));
                    }
                    receipt.written.push(e.name.clone());
                    receipt.bytes_processed += bytes;
                    progress(Progress {
                        operation: "test".into(),
                        entry: e.name.clone(),
                        completed_entries: receipt.written.len(),
                        total_entries: info.entries.len(),
                        bytes_processed: receipt.bytes_processed,
                        total_bytes: info.total_size,
                    });
                }
                Frame::Progress { id, bytes } => progress(Progress {
                    operation: "test".into(),
                    entry: info
                        .entries
                        .get(id)
                        .map_or("RAR decoding", |e| e.name.as_str())
                        .into(),
                    completed_entries: receipt.written.len(),
                    total_entries: info.entries.len(),
                    bytes_processed: bytes,
                    total_bytes: info.total_size,
                }),
                _ => {}
            }
            Ok(())
        },
    )?;
    if receipt.written.len() != info.entries.len() {
        return Err(invalid("RAR test coverage incomplete"));
    }
    Ok(receipt)
}
