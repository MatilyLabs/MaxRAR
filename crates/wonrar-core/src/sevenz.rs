//! Maintained 7z adapter. Every output is written by the same scoped policy as ZIP.
use super::*;
use sevenz_rust2::{
    encoder_options::AesEncoderOptions, ArchiveEntry, ArchiveReader, ArchiveWriter, EncoderMethod,
    Password,
};

fn error(e: sevenz_rust2::Error) -> Error {
    invalid(format!("7z error: {e}"))
}
fn password(value: Option<&str>) -> Password {
    value.map(Password::from).unwrap_or_else(Password::empty)
}

pub(super) fn inspect(path: &Path, secret: Option<&str>) -> Result<ArchiveInfo> {
    let mut file = File::open(path)?;
    let identity = fingerprint(&mut file)?;
    let reader = ArchiveReader::new(file, password(secret)).map_err(error)?;
    let a = reader.archive();
    if a.files.len() > Limits::default().max_entries {
        return Err(invalid("7z inventory exceeds 100000 entries"));
    }
    let mut entries = Vec::new();
    let mut findings = Vec::new();
    let mut seen = HashSet::new();
    let mut total = 0u64;
    for (i, f) in a.files.iter().enumerate() {
        let id = i.to_string();
        let mut finding = |code: &str, message: &str, severity: &str| {
            findings.push(Finding {
                severity: severity.into(),
                code: code.into(),
                message: message.into(),
                entry_id: Some(id.clone()),
            })
        };
        if safe_name(&f.name).is_err() {
            finding(
                "unsafe_path",
                "Entry has an unsafe or non-portable path.",
                "error",
            );
        }
        // Unix modes are encoded in the upper half of Windows attributes by 7-Zip.
        let unix_type = (f.windows_attributes >> 16) & 0o170000;
        if f.is_anti_item
            || f.windows_attributes & 0x400 != 0
            || (unix_type != 0 && unix_type != 0o100000 && unix_type != 0o040000)
        {
            finding(
                "special_entry",
                "Links, reparse points, anti-items and special files are blocked.",
                "error",
            );
        }
        if !seen.insert(name_key(f.name.trim_end_matches('/'))) {
            finding(
                "duplicate_path",
                "Path collides under Unicode-normalized case-insensitive comparison.",
                "warning",
            );
        }
        let encrypted = a
            .stream_map
            .file_block_index
            .get(i)
            .and_then(|v| *v)
            .is_some_and(|b| {
                a.blocks[b]
                    .coders
                    .iter()
                    .any(|c| c.encoder_method_id() == EncoderMethod::ID_AES256_SHA256)
            });
        if encrypted {
            finding(
                "encrypted",
                "Content is encrypted and requires a password.",
                "warning",
            );
        }
        let ext = f.name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
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
        total = total
            .checked_add(f.size)
            .ok_or_else(|| invalid("7z declared size overflow"))?;
        entries.push(Entry {
            id,
            name: f.name.clone(),
            size: f.size,
            compressed_size: f.compressed_size,
            is_dir: f.is_directory,
            is_encrypted: encrypted,
            modified: None,
        });
    }
    findings.push(Finding { severity:"info".into(),code:"inspection_coverage".into(),message:"Inventory and path checks only. No malware scan. Solid 7z reads may decode preceding entries; decoded byte limits include skipped data. Per-file compressed sizes may be unavailable.".into(),entry_id:None });
    Ok(ArchiveInfo {
        path: absolute(path)?.to_string_lossy().into(),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        format: "7z".into(),
        identity,
        entries,
        total_size: total,
        compressed_size: fs::metadata(path)?.len(),
        comment: String::new(),
        findings,
        capabilities: Capabilities {
            can_create: true,
            can_extract: true,
            can_test: true,
            can_delete: false,
            can_comment: false,
        },
    })
}

struct Counted<'a> {
    inner: &'a mut dyn Read,
    n: u64,
}

impl Read for Counted<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.n += n as u64;
        Ok(n)
    }
}

/// Map callback entries back to their central inventory IDs without relying on names.
/// The library passes references to its stable `archive.files` allocation.
fn traverse(
    path: &Path,
    secret: Option<&str>,
    identity: Option<&str>,
    mut each: impl FnMut(usize, &ArchiveEntry, &mut dyn Read) -> Result<()>,
) -> Result<()> {
    let mut file = File::open(path)?;
    if let Some(expected) = identity {
        if fingerprint(&mut file)? != expected {
            return Err(invalid("Archive changed before decoding"));
        }
    }
    let mut reader = ArchiveReader::new(file, password(secret)).map_err(error)?;
    reader.set_thread_count(1);
    let ids: std::collections::HashMap<usize, usize> = reader
        .archive()
        .files
        .iter()
        .enumerate()
        .map(|(i, e)| (e as *const ArchiveEntry as usize, i))
        .collect();
    let mut original = None;
    let result = reader.for_each_entries(|entry, stream| {
        let id = ids
            .get(&(entry as *const ArchiveEntry as usize))
            .copied()
            .ok_or_else(|| sevenz_rust2::Error::Other("7z entry identity unavailable".into()))?;
        // The library checks a CRC only once the declared size has been read, so
        // a wrong key whose garbage stream ends early would otherwise pass.
        let mut counted = Counted { inner: stream, n: 0 };
        let outcome = each(id, entry, &mut counted).and_then(|()| {
            if entry.is_directory || counted.n == entry.size {
                Ok(())
            } else {
                Err(invalid(
                    "Decoded size does not match the archive. The password may be wrong or the archive damaged.",
                ))
            }
        });
        match outcome {
            Ok(()) => Ok(true),
            Err(e) => {
                original = Some(e);
                Err(sevenz_rust2::Error::Other("Operation stopped".into()))
            }
        }
    });
    if let Some(e) = original {
        return Err(e);
    }
    result.map_err(error)
}

pub(super) fn entry_bytes(
    path: &Path,
    id: &str,
    max: u64,
    secret: Option<&str>,
    cancel: &AtomicBool,
) -> Result<EntryBytes> {
    let wanted: usize = id.parse().map_err(|_| invalid("Invalid entry ID"))?;
    let mut decoded = 0u64;
    let mut found = None;
    let limits = Limits::default();
    check(cancel)?;
    let operation = traverse(path, secret, None, |i, entry, stream| {
        check(cancel)?;
        if i == wanted {
            if entry.is_directory {
                return Err(invalid("Directories do not contain bytes"));
            }
            found = Some(read_entry_prefix(stream, max, cancel)?);
            return Err(invalid("Entry read complete"));
        }
        decoded += copy_limited(
            stream,
            &mut std::io::sink(),
            limits
                .max_file_bytes
                .min(limits.max_total_bytes.saturating_sub(decoded)),
            cancel,
            &mut |_| {},
        )?;
        Ok(())
    });
    if let Some(found) = found {
        return Ok(found);
    }
    operation?;
    Err(invalid("Entry ID not found"))
}

pub(super) fn extract(
    p: &ExtractionPlan,
    root: &Dir,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
    secret: Option<&str>,
) -> Result<Receipt> {
    let mut r = Receipt {
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
    let mut decoded = 0u64;
    let selected: std::collections::HashMap<usize, &PlannedEntry> = p
        .entries
        .iter()
        .filter(|e| e.action == "extract")
        .map(|e| {
            Ok((
                e.entry_id
                    .parse()
                    .map_err(|_| invalid("Invalid entry ID"))?,
                e,
            ))
        })
        .collect::<Result<_>>()?;
    traverse(
        Path::new(&p.archive_path),
        secret,
        Some(&p.archive_identity),
        |id, _, stream| {
            check(cancel)?;
            if let Some(e) = selected.get(&id) {
                let n = write_planned(
                    root,
                    e,
                    stream,
                    p,
                    cancel,
                    progress,
                    r.written.len(),
                    decoded,
                )?;
                decoded += n;
                r.bytes_processed += n;
                r.written.push(e.output_name.clone());
            } else {
                decoded += copy_limited(
                    stream,
                    &mut std::io::sink(),
                    p.limits
                        .max_file_bytes
                        .min(p.limits.max_total_bytes.saturating_sub(decoded)),
                    cancel,
                    &mut |_| {},
                )?;
            }
            Ok(())
        },
    )?;
    r.message = format!(
        "Extracted {} entries; skipped {}. No malware scan was performed.",
        r.written.len(),
        r.skipped.len()
    );
    Ok(r)
}

pub(super) fn test(
    path: &Path,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
    secret: Option<&str>,
) -> Result<Receipt> {
    let info = inspect(path, secret)?;
    let limits = Limits::default();
    let mut total = 0u64;
    let mut written = Vec::new();
    traverse(path, secret, Some(&info.identity), |_, entry, stream| {
        let before = total;
        total += copy_limited(
            stream,
            &mut std::io::sink(),
            limits
                .max_file_bytes
                .min(limits.max_total_bytes.saturating_sub(total)),
            cancel,
            &mut |n| {
                progress(Progress {
                    operation: "test".into(),
                    entry: entry.name.clone(),
                    completed_entries: written.len(),
                    total_entries: info.entries.len(),
                    bytes_processed: before + n,
                    total_bytes: info.total_size,
                })
            },
        )?;
        written.push(entry.name.clone());
        Ok(())
    })?;
    Ok(Receipt {
        operation: "test".into(),
        destination: None,
        written,
        skipped: vec![],
        bytes_processed: total,
        message: "7z payloads decoded and available checksums checked. Not a malware scan.".into(),
    })
}

struct ControlledReader<'a> {
    file: File,
    cancel: &'a AtomicBool,
    progress: &'a dyn Fn(Progress),
    name: String,
    count: usize,
    total: usize,
    bytes: u64,
    before: u64,
}
impl Read for ControlledReader<'_> {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::other("Operation cancelled"));
        }
        let n = self.file.read(b)?;
        self.bytes += n as u64;
        let limits = Limits::default();
        if self.bytes > limits.max_file_bytes || self.before + self.bytes > limits.max_total_bytes {
            return Err(std::io::Error::other("Source exceeds compression limit"));
        }
        (self.progress)(Progress {
            operation: "create".into(),
            entry: self.name.clone(),
            completed_entries: self.count,
            total_entries: self.total,
            bytes_processed: self.before + self.bytes,
            total_bytes: 0,
        });
        Ok(n)
    }
}

pub(super) fn create(
    output: &Path,
    inputs: &[PathBuf],
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
    secret: Option<&str>,
) -> Result<ArchiveInfo> {
    let output = absolute(output)?;
    let parent = output.parent().ok_or_else(|| invalid("No output parent"))?;
    let output = parent.canonicalize()?.join(
        output
            .file_name()
            .ok_or_else(|| invalid("No output filename"))?,
    );
    let parent = output.parent().ok_or_else(|| invalid("No output parent"))?;
    let mut stage = NamedTempFile::new_in(parent)?;
    let mut writer = ArchiveWriter::new(stage.as_file_mut()).map_err(error)?;
    // Keep inventory readable. Header encryption is deliberately not exposed yet.
    writer.set_encrypt_header(false);
    if let Some(secret) = secret {
        writer.set_content_methods(vec![
            AesEncoderOptions::new(password(Some(secret))).into(),
            EncoderMethod::LZMA2.into(),
        ]);
    }
    let mut sources = Vec::new();
    let mut seen = HashSet::new();
    for input in inputs {
        let input = absolute(input)?;
        if fs::symlink_metadata(&input)?.file_type().is_symlink() {
            return Err(invalid("Source links unsupported"));
        }
        let input = input.canonicalize()?;
        if output.starts_with(&input) {
            return Err(invalid(
                "Output cannot be inside an input folder or equal an input",
            ));
        }
        let base = input
            .parent()
            .ok_or_else(|| invalid("Filesystem roots cannot be added"))?;
        for item in walkdir::WalkDir::new(&input).follow_links(false) {
            let item = item.map_err(|e| invalid(e.to_string()))?;
            if item.file_type().is_symlink()
                || (!item.file_type().is_file() && !item.file_type().is_dir())
            {
                return Err(invalid("Links and special sources are unsupported"));
            }
            let name = item
                .path()
                .strip_prefix(base)
                .map_err(|e| invalid(e.to_string()))?
                .components()
                .map(|v| {
                    v.as_os_str()
                        .to_str()
                        .ok_or_else(|| invalid("Source names must be UTF-8"))
                })
                .collect::<Result<Vec<_>>>()?
                .join("/");
            safe_name(&name)?;
            if !seen.insert(name_key(&name)) {
                return Err(invalid("Source names collide"));
            }
            sources.push((item.path().to_owned(), name, item.file_type().is_dir()));
            if sources.len() > Limits::default().max_entries {
                return Err(invalid("Too many source entries"));
            }
        }
    }
    let count = sources.len();
    let mut bytes = 0u64;
    for (i, (path, name, dir)) in sources.into_iter().enumerate() {
        check(cancel)?;
        if dir {
            writer
                .push_archive_entry(ArchiveEntry::new_directory(&name), None::<File>)
                .map_err(error)?;
        } else {
            let mut reader = ControlledReader {
                file: File::open(path)?,
                cancel,
                progress,
                name: name.clone(),
                count: i,
                total: count,
                bytes: 0,
                before: bytes,
            };
            let result =
                writer.push_archive_entry(ArchiveEntry::new_file(&name), Some(&mut reader));
            check(cancel)?;
            result.map_err(error)?;
            bytes += reader.bytes;
        }
    }
    writer.finish()?;
    stage.as_file_mut().sync_all()?;
    check(cancel)?;
    test(stage.path(), cancel, &|_| {}, secret)?;
    stage
        .persist_noclobber(&output)
        .map_err(|e| invalid(format!("Archive was not published: {}", e.error)))?;
    inspect(&output, secret)
}
