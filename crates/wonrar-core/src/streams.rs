//! Streaming TAR and self-identifying compression filters.
//! No backend convenience extraction method is used: output stays in the shared
//! capability-scoped writer. TAR extension metadata is parsed with explicit caps.
use super::*;
use std::io::{self, Cursor, Seek, SeekFrom};

const HEADER_BYTES: usize = 512;
const MAX_EXTENSION_BYTES: u64 = 1024 * 1024;
const MAX_PATH_BYTES: usize = 65535;
const DECODER_MEMORY_KIB: u32 = 128 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Codec {
    Plain,
    Gzip,
    Bzip2,
    Xz,
    Zstd,
}
#[derive(Debug, Clone, Copy)]
pub(super) struct Kind {
    codec: Codec,
    tar: bool,
}
impl Kind {
    fn name(self) -> &'static str {
        match (self.codec, self.tar) {
            (Codec::Plain, _) => "tar",
            (Codec::Gzip, true) => "tar.gz",
            (Codec::Gzip, false) => "gzip",
            (Codec::Bzip2, true) => "tar.bz2",
            (Codec::Bzip2, false) => "bzip2",
            (Codec::Xz, true) => "tar.xz",
            (Codec::Xz, false) => "xz",
            (Codec::Zstd, true) => "tar.zst",
            (Codec::Zstd, false) => "zstd",
        }
    }
}

struct BudgetReader<'a, R> {
    inner: R,
    cancel: &'a AtomicBool,
    read: u64,
    limit: u64,
}
impl<R: Read> Read for BudgetReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(io::Error::other("Operation cancelled"));
        }
        if buf.is_empty() {
            return Ok(0);
        }
        // Read at most one byte beyond the budget to distinguish exact EOF.
        let allowed = self
            .limit
            .saturating_sub(self.read)
            .saturating_add(1)
            .min(buf.len() as u64) as usize;
        let n = self.inner.read(&mut buf[..allowed])?;
        self.read = self
            .read
            .checked_add(n as u64)
            .ok_or_else(|| io::Error::other("Decoded size overflow"))?;
        if self.read > self.limit {
            return Err(io::Error::other("Actual decoded stream exceeds byte limit"));
        }
        Ok(n)
    }
}
fn decoded<'a>(
    file: File,
    codec: Codec,
    cancel: &'a AtomicBool,
    limit: u64,
) -> Result<Box<dyn Read + 'a>> {
    let reader: Box<dyn Read> = match codec {
        Codec::Plain => Box::new(file),
        Codec::Gzip => Box::new(flate2::read::MultiGzDecoder::new(file)),
        Codec::Bzip2 => Box::new(bzip2::read::MultiBzDecoder::new(file)),
        Codec::Xz => Box::new(lzma_rust2::XzReader::new_mem_limit(
            file,
            true,
            DECODER_MEMORY_KIB,
        )),
        Codec::Zstd => {
            let mut decoder = zstd::stream::read::Decoder::new(file)?;
            decoder.window_log_max(27)?;
            Box::new(decoder)
        }
    };
    Ok(Box::new(BudgetReader {
        inner: reader,
        cancel,
        read: 0,
        limit,
    }))
}
fn read_some(reader: &mut dyn Read, count: usize) -> Result<Vec<u8>> {
    let mut result = vec![0; count];
    let mut filled = 0;
    while filled < count {
        let n = reader.read(&mut result[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    result.truncate(filled);
    Ok(result)
}
fn valid_header(bytes: &[u8]) -> bool {
    if bytes.len() < HEADER_BYTES || bytes[..HEADER_BYTES].iter().all(|b| *b == 0) {
        return false;
    }
    let header = tar::Header::from_byte_slice(&bytes[..HEADER_BYTES]);
    let calculated = bytes[..148]
        .iter()
        .chain(&bytes[156..512])
        .map(|b| *b as u32)
        .sum::<u32>()
        + 8 * 32;
    header.cksum().is_ok_and(|checksum| checksum == calculated) && header.entry_size().is_ok()
}
fn tar_prefix(bytes: &[u8]) -> bool {
    valid_header(bytes) || bytes.len() >= 1024 && bytes[..1024].iter().all(|b| *b == 0)
}
pub(super) fn detect(path: &Path) -> Result<Option<Kind>> {
    let mut file = File::open(path)?;
    let bytes = read_some(&mut file, 1024)?;
    let codec = if bytes.starts_with(&[0x1f, 0x8b]) {
        Codec::Gzip
    } else if bytes.starts_with(b"BZh") {
        Codec::Bzip2
    } else if bytes.starts_with(b"\xfd7zXZ\0") {
        Codec::Xz
    } else if bytes.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        Codec::Zstd
    } else {
        return Ok(tar_prefix(&bytes).then_some(Kind {
            codec: Codec::Plain,
            tar: true,
        }));
    };
    file.seek(SeekFrom::Start(0))?;
    let flag = AtomicBool::new(false);
    let mut reader = decoded(file, codec, &flag, 1024)?;
    let inner = read_some(&mut reader, 1024)?;
    Ok(Some(Kind {
        codec,
        tar: tar_prefix(&inner),
    }))
}

#[derive(Clone)]
struct Meta {
    id: usize,
    name: String,
    size: u64,
    is_dir: bool,
    special: bool,
    modified: Option<String>,
}
#[derive(Default)]
struct Extensions {
    path: Option<String>,
    size: Option<u64>,
}
fn parse_pax(bytes: &[u8], ext: &mut Extensions) -> Result<()> {
    let mut position = 0;
    while position < bytes.len() {
        let space = bytes[position..]
            .iter()
            .position(|b| *b == b' ')
            .ok_or_else(|| invalid("Malformed PAX record length"))?
            + position;
        let length = std::str::from_utf8(&bytes[position..space])
            .map_err(|_| invalid("Invalid PAX length"))?
            .parse::<usize>()
            .map_err(|_| invalid("Invalid PAX length"))?;
        let end = position
            .checked_add(length)
            .filter(|end| *end <= bytes.len() && *end > space + 1)
            .ok_or_else(|| invalid("PAX length exceeds extension"))?;
        if bytes[end - 1] != b'\n' {
            return Err(invalid("Malformed PAX record terminator"));
        }
        let record = &bytes[space + 1..end - 1];
        let equal = record
            .iter()
            .position(|b| *b == b'=')
            .ok_or_else(|| invalid("Malformed PAX key/value"))?;
        let key = &record[..equal];
        let value = &record[equal + 1..];
        if key.starts_with(b"GNU.sparse.") || key == b"SCHILY.realsize" {
            return Err(invalid(
                "Sparse TAR extensions are unsupported in this build",
            ));
        }
        match key {
            b"path" => {
                if value.len() > MAX_PATH_BYTES {
                    return Err(invalid("TAR path exceeds 65535 bytes"));
                }
                ext.path = if value.is_empty() {
                    None
                } else {
                    Some(
                        std::str::from_utf8(value)
                            .map_err(|_| invalid("TAR paths must be UTF-8"))?
                            .into(),
                    )
                };
            }
            b"size" => {
                ext.size = if value.is_empty() {
                    None
                } else {
                    Some(
                        std::str::from_utf8(value)
                            .map_err(|_| invalid("Invalid PAX size"))?
                            .parse::<u64>()
                            .map_err(|_| invalid("Invalid PAX size"))?,
                    )
                };
            }
            _ => {} // ownership, xattrs and execution metadata are deliberately not restored
        }
        position = end;
    }
    Ok(())
}
fn exact(reader: &mut dyn Read, bytes: &mut [u8], label: &str) -> Result<()> {
    reader
        .read_exact(bytes)
        .map_err(|e| invalid(format!("Truncated or unreadable {label}: {e}")))
}
fn padding(reader: &mut dyn Read, size: u64) -> Result<()> {
    let count = (512 - size % 512) % 512;
    let mut bytes = [0; 512];
    exact(reader, &mut bytes[..count as usize], "TAR padding")
}

/// Parse fixed-size headers ourselves to cap GNU/PAX metadata before allocation.
/// The tar crate supplies checked numeric/header parsing and creation support.
fn traverse_tar(
    reader: &mut dyn Read,
    limits: &Limits,
    cancel: &AtomicBool,
    mut each: impl FnMut(&Meta, &mut dyn Read) -> Result<()>,
) -> Result<()> {
    let mut global = Extensions::default();
    let mut local = Extensions::default();
    let mut longname = None;
    let mut pending = false;
    let mut count = 0usize;
    let mut headers = 0usize;
    let mut total = 0u64;
    loop {
        check(cancel)?;
        let mut bytes = [0; 512];
        exact(reader, &mut bytes, "TAR header/end marker")?;
        if bytes.iter().all(|b| *b == 0) {
            let mut second = [0; 512];
            exact(reader, &mut second, "TAR end marker")?;
            if second.iter().any(|b| *b != 0) {
                return Err(invalid("TAR requires two zero end blocks"));
            }
            if pending {
                return Err(invalid("Dangling TAR extension without a file"));
            }
            let mut tail = [0; 65536];
            let mut padding_bytes = 0u64;
            loop {
                check(cancel)?;
                let n = reader.read(&mut tail)?;
                if n == 0 {
                    break;
                }
                padding_bytes += n as u64;
                if padding_bytes > MAX_EXTENSION_BYTES {
                    return Err(invalid("TAR trailing padding exceeds 1 MiB limit"));
                }
                if tail[..n].iter().any(|b| *b != 0) {
                    return Err(invalid(
                        "Nonzero data after TAR end; concatenated TAR is unsupported",
                    ));
                }
            }
            return Ok(());
        }
        if !valid_header(&bytes) {
            return Err(invalid("TAR header checksum or size is invalid"));
        }
        headers += 1;
        if headers > limits.max_entries.saturating_mul(4).saturating_add(16) {
            return Err(invalid("Too many TAR metadata headers"));
        }
        let header = tar::Header::from_byte_slice(&bytes);
        let kind = header.entry_type();
        let raw_size = header.entry_size()?;
        if kind.is_gnu_sparse() {
            return Err(invalid("GNU sparse TAR files are unsupported"));
        }
        if kind.is_gnu_longname()
            || kind.is_gnu_longlink()
            || kind.is_pax_local_extensions()
            || kind.is_pax_global_extensions()
        {
            if raw_size > MAX_EXTENSION_BYTES {
                return Err(invalid("TAR extension exceeds 1 MiB metadata limit"));
            }
            let mut data = vec![0; raw_size as usize];
            exact(reader, &mut data, "TAR extension")?;
            padding(reader, raw_size)?;
            if kind.is_gnu_longname() {
                let name = data.split(|b| *b == 0).next().unwrap_or_default();
                if name.len() > MAX_PATH_BYTES {
                    return Err(invalid("GNU TAR long path exceeds limit"));
                }
                longname = Some(
                    std::str::from_utf8(name)
                        .map_err(|_| invalid("TAR paths must be UTF-8"))?
                        .into(),
                );
                pending = true;
            } else if kind.is_pax_local_extensions() {
                parse_pax(&data, &mut local)?;
                pending = true;
            } else if kind.is_pax_global_extensions() {
                parse_pax(&data, &mut global)?;
            } else {
                pending = true;
            } // link target is never followed
            continue;
        }
        if count >= limits.max_entries {
            return Err(invalid("TAR inventory exceeds entry limit"));
        }
        let path = header.path_bytes();
        let preferred = local
            .path
            .take()
            .or_else(|| global.path.clone())
            .or(longname.take());
        let name = match preferred {
            Some(name) => name,
            None => std::str::from_utf8(path.as_ref())
                .map_err(|_| invalid("TAR paths must be UTF-8"))?
                .to_string(),
        };
        let size = local.size.take().or(global.size).unwrap_or(raw_size);
        if size > limits.max_file_bytes {
            return Err(invalid("TAR entry exceeds decoded file limit"));
        }
        total = total
            .checked_add(size)
            .filter(|v| *v <= limits.max_total_bytes)
            .ok_or_else(|| invalid("TAR contents exceed total decoded byte limit"))?;
        let meta = Meta {
            id: count,
            name,
            size,
            is_dir: kind.is_dir(),
            special: !kind.is_file() && !kind.is_dir(),
            modified: header.mtime().ok().map(|v| format!("unix:{v}")),
        };
        let mut data = reader.take(size);
        each(&meta, &mut data)?;
        copy_limited(&mut data, &mut io::sink(), size, cancel, &mut |_| {})?;
        if data.limit() != 0 {
            return Err(invalid("Truncated TAR payload"));
        }
        padding(reader, size)?;
        count += 1;
        pending = false;
    }
}
fn stream_limit(limits: &Limits) -> u64 {
    limits
        .max_total_bytes
        .saturating_add((limits.max_entries.min(100000) as u64).saturating_mul(1024))
        .saturating_add(2 * MAX_EXTENSION_BYTES)
}
fn pinned(path: &Path, identity: Option<&str>) -> Result<File> {
    let mut file = File::open(path)?;
    if let Some(expected) = identity {
        if fingerprint(&mut file)? != expected {
            return Err(invalid("Archive changed before decoding"));
        }
    }
    Ok(file)
}
fn single_name(path: &Path, kind: Kind) -> String {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let lower = name.to_ascii_lowercase();
    for suffix in [".gzip", ".gz", ".bz2", ".xz", ".zstd", ".zst"] {
        if lower.ends_with(suffix) && name.len() > suffix.len() {
            return name[..name.len() - suffix.len()].into();
        }
    }
    // Gzip FNAME is untrusted and not used as an extraction path.
    match kind.codec {
        Codec::Plain => name.into(),
        _ => format!("{name}.unpacked"),
    }
}
fn traverse(
    path: &Path,
    kind: Kind,
    identity: Option<&str>,
    limits: &Limits,
    cancel: &AtomicBool,
    mut each: impl FnMut(&Meta, &mut dyn Read) -> Result<()>,
) -> Result<()> {
    check(cancel)?;
    let file = pinned(path, identity)?;
    let mut reader = decoded(
        file,
        kind.codec,
        cancel,
        if kind.tar {
            stream_limit(limits)
        } else {
            limits.max_total_bytes.min(limits.max_file_bytes)
        },
    )?;
    let operation = if kind.tar {
        traverse_tar(&mut reader, limits, cancel, each)
    } else {
        let meta = Meta {
            id: 0,
            name: single_name(path, kind),
            size: 0,
            is_dir: false,
            special: false,
            modified: None,
        };
        each(&meta, &mut reader)
    };
    if cancel.load(Ordering::Relaxed) {
        Err(Error::Cancelled)
    } else {
        operation
    }
}

pub(super) fn inspect(path: &Path, kind: Kind) -> Result<ArchiveInfo> {
    let mut file = File::open(path)?;
    let identity = fingerprint(&mut file)?;
    let flag = AtomicBool::new(false);
    let limits = Limits::default();
    let mut entries = Vec::new();
    let mut findings = Vec::new();
    let mut total = 0u64;
    let mut names = HashSet::new();
    traverse(
        path,
        kind,
        Some(&identity),
        &limits,
        &flag,
        |meta, reader| {
            let id = meta.id.to_string();
            let mut add = |code: &str, message: &str, severity: &str| {
                findings.push(Finding {
                    severity: severity.into(),
                    code: code.into(),
                    message: message.into(),
                    entry_id: Some(id.clone()),
                })
            };
            if safe_name(&meta.name).is_err() {
                add(
                    "unsafe_path",
                    "Unsafe or non-portable entry path; extraction blocked.",
                    "error",
                );
            }
            if meta.special {
                add(
                    "special_entry",
                    "Links, devices and other special TAR entries are blocked.",
                    "error",
                );
            }
            if !names.insert(name_key(meta.name.trim_end_matches('/'))) {
                add(
                    "duplicate_path",
                    "Entry collides under Unicode-normalized lowercase comparison.",
                    "warning",
                );
            }
            let ext = meta
                .name
                .rsplit('.')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            if [
                "exe", "msi", "sh", "bat", "cmd", "ps1", "app", "scr", "dll", "js", "vbs",
            ]
            .contains(&ext.as_str())
            {
                add(
                    "executable_content",
                    "Executable or script entry; this is not a malware verdict.",
                    "warning",
                );
            }
            let size = if kind.tar {
                meta.size
            } else {
                copy_limited(
                    reader,
                    &mut io::sink(),
                    limits.max_file_bytes.min(limits.max_total_bytes),
                    &flag,
                    &mut |_| {},
                )?
            };
            total = total
                .checked_add(size)
                .ok_or_else(|| invalid("Declared size overflow"))?;
            entries.push(Entry {
                id,
                name: meta.name.clone(),
                size,
                compressed_size: if kind.tar {
                    0
                } else {
                    fs::metadata(path)?.len()
                },
                is_dir: meta.is_dir,
                is_encrypted: false,
                modified: meta.modified.clone(),
            });
            Ok(())
        },
    )?;
    findings.push(Finding {severity:"info".into(),code:"inspection_coverage".into(),message:"Stream and TAR header structure checked under decoded-byte limits. TAR has no per-file content checksum. No malware scan or AI safety verdict was performed. Links, sparse files and execution metadata are unsupported; per-entry compressed sizes in TAR are unavailable.".into(),entry_id:None});
    let compressed_size = fs::metadata(path)?.len();
    if compressed_size > 0 && total / compressed_size > 1000 {
        findings.push(Finding { severity:"warning".into(),code:"high_expansion".into(),message:"Observed decoded expansion exceeds 1000:1. This is a resource-use signal, not a malware verdict.".into(),entry_id:None });
    }
    Ok(ArchiveInfo {
        path: absolute(path)?.to_string_lossy().into(),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        format: kind.name().into(),
        identity,
        entries,
        total_size: total,
        compressed_size,
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
pub(super) fn entry_bytes(
    path: &Path,
    kind: Kind,
    id: &str,
    max: u64,
    cancel: &AtomicBool,
) -> Result<EntryBytes> {
    let wanted = id
        .parse::<usize>()
        .map_err(|_| invalid("Invalid entry ID"))?;
    let mut found = None;
    let result = traverse(
        path,
        kind,
        None,
        &Limits::default(),
        cancel,
        |meta, reader| {
            if meta.id == wanted {
                if meta.is_dir || meta.special {
                    return Err(invalid("Only regular entries can be inspected"));
                }
                found = Some(read_entry_prefix(reader, max, cancel)?);
                return Err(invalid("Entry read complete"));
            }
            Ok(())
        },
    );
    if let Some(found) = found {
        return Ok(found);
    }
    result?;
    Err(invalid("Entry ID not found"))
}
pub(super) fn extract(
    path: &Path,
    kind: Kind,
    p: &ExtractionPlan,
    root: &Dir,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
) -> Result<Receipt> {
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
    traverse(
        path,
        kind,
        Some(&p.archive_identity),
        &p.limits,
        cancel,
        |meta, reader| {
            if let Some(entry) = selected.get(&meta.id) {
                if meta.special {
                    return Err(invalid("Special TAR entry cannot be extracted"));
                }
                r.bytes_processed += write_planned(
                    root,
                    entry,
                    reader,
                    p,
                    cancel,
                    progress,
                    r.written.len(),
                    r.bytes_processed,
                )?;
                r.written.push(entry.output_name.clone());
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
    kind: Kind,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
) -> Result<Receipt> {
    let info = inspect(path, kind)?;
    let mut r=Receipt {operation:"test".into(),destination:None,written:vec![],skipped:vec![],bytes_processed:0,message:"Decoded stream and available compression checksums checked. TAR headers checked; TAR has no per-file payload checksum. Not a malware scan.".into()};
    traverse(
        path,
        kind,
        Some(&info.identity),
        &Limits::default(),
        cancel,
        |meta, reader| {
            let before = r.bytes_processed;
            r.bytes_processed += copy_limited(
                reader,
                &mut io::sink(),
                Limits::default().max_file_bytes,
                cancel,
                &mut |n| {
                    progress(Progress {
                        operation: "test".into(),
                        entry: meta.name.clone(),
                        completed_entries: r.written.len(),
                        total_entries: info.entries.len(),
                        bytes_processed: before + n,
                        total_bytes: info.total_size,
                    })
                },
            )?;
            r.written.push(meta.name.clone());
            Ok(())
        },
    )?;
    Ok(r)
}

pub(super) fn output_kind(path: &Path) -> Option<Kind> {
    let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    for (suffix, codec) in [
        (".tar.gz", Codec::Gzip),
        (".tgz", Codec::Gzip),
        (".tar.bz2", Codec::Bzip2),
        (".tbz2", Codec::Bzip2),
        (".tar.xz", Codec::Xz),
        (".txz", Codec::Xz),
        (".tar.zst", Codec::Zstd),
        (".tzst", Codec::Zstd),
    ] {
        if name.ends_with(suffix) {
            return Some(Kind { codec, tar: true });
        }
    }
    if name.ends_with(".tar") {
        return Some(Kind {
            codec: Codec::Plain,
            tar: true,
        });
    }
    for (suffix, codec) in [
        (".gz", Codec::Gzip),
        (".gzip", Codec::Gzip),
        (".bz2", Codec::Bzip2),
        (".xz", Codec::Xz),
        (".zst", Codec::Zstd),
        (".zstd", Codec::Zstd),
    ] {
        if name.ends_with(suffix) {
            return Some(Kind { codec, tar: false });
        }
    }
    None
}
enum Encoder<'a> {
    Plain(&'a mut File),
    Gzip(flate2::write::GzEncoder<&'a mut File>),
    Bzip2(bzip2::write::BzEncoder<&'a mut File>),
    Xz(Box<lzma_rust2::XzWriter<&'a mut File>>),
    Zstd(zstd::stream::write::Encoder<'static, &'a mut File>),
}
impl<'a> Encoder<'a> {
    fn new(file: &'a mut File, codec: Codec) -> Result<Self> {
        Ok(match codec {
            Codec::Plain => Self::Plain(file),
            Codec::Gzip => Self::Gzip(flate2::write::GzEncoder::new(
                file,
                flate2::Compression::default(),
            )),
            Codec::Bzip2 => Self::Bzip2(bzip2::write::BzEncoder::new(
                file,
                bzip2::Compression::default(),
            )),
            Codec::Xz => Self::Xz(Box::new(lzma_rust2::XzWriter::new(
                file,
                lzma_rust2::XzOptions::with_preset(6),
            )?)),
            Codec::Zstd => {
                let mut e = zstd::stream::write::Encoder::new(file, 3)?;
                e.include_checksum(true)?;
                Self::Zstd(e)
            }
        })
    }
    fn finish(self) -> Result<()> {
        match self {
            Self::Plain(f) => f.flush()?,
            Self::Gzip(e) => {
                e.finish()?;
            }
            Self::Bzip2(e) => {
                e.finish()?;
            }
            Self::Xz(e) => {
                e.finish()?;
            }
            Self::Zstd(e) => {
                e.finish()?;
            }
        }
        Ok(())
    }
}
impl Write for Encoder<'_> {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(e) => e.write(b),
            Self::Gzip(e) => e.write(b),
            Self::Bzip2(e) => e.write(b),
            Self::Xz(e) => e.write(b),
            Self::Zstd(e) => e.write(b),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(e) => e.flush(),
            Self::Gzip(e) => e.flush(),
            Self::Bzip2(e) => e.flush(),
            Self::Xz(e) => e.flush(),
            Self::Zstd(e) => e.flush(),
        }
    }
}
struct Source {
    path: PathBuf,
    name: String,
    dir: bool,
    size: u64,
}
fn sources(output: &Path, inputs: &[PathBuf], tar: bool) -> Result<Vec<Source>> {
    if !tar && (inputs.len() != 1 || !inputs[0].is_file()) {
        return Err(invalid("Single-stream compression requires exactly one regular file; choose a TAR compound format for folders or multiple files"));
    }
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    let mut total = 0u64;
    let limits = Limits::default();
    for input in inputs {
        let input = absolute(input)?;
        if fs::symlink_metadata(&input)?.file_type().is_symlink() {
            return Err(invalid("Source links are unsupported"));
        }
        let input = input.canonicalize()?;
        if output.starts_with(&input) {
            return Err(invalid(
                "Output cannot be inside a source folder or equal an input",
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
                return Err(invalid("Source links and special files are unsupported"));
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
            let size = if item.file_type().is_dir() {
                0
            } else {
                fs::metadata(item.path())?.len()
            };
            if size > limits.max_file_bytes {
                return Err(invalid("Source exceeds file byte limit"));
            }
            total = total
                .checked_add(size)
                .filter(|v| *v <= limits.max_total_bytes)
                .ok_or_else(|| invalid("Sources exceed total byte limit"))?;
            result.push(Source {
                path: item.path().into(),
                name,
                dir: item.file_type().is_dir(),
                size,
            });
            if result.len() > limits.max_entries {
                return Err(invalid("Too many source entries"));
            }
        }
    }
    Ok(result)
}
pub(super) fn create(
    output: &Path,
    kind: Kind,
    inputs: &[PathBuf],
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
    password: Option<&str>,
) -> Result<ArchiveInfo> {
    if password.is_some() {
        return Err(invalid(
            "TAR and compression streams do not support encryption. Choose ZIP or 7z.",
        ));
    }
    let output = absolute(output)?;
    let parent = output
        .parent()
        .ok_or_else(|| invalid("No output parent"))?
        .canonicalize()?;
    let output = parent.join(
        output
            .file_name()
            .ok_or_else(|| invalid("No output filename"))?,
    );
    let sources = sources(&output, inputs, kind.tar)?;
    let count = sources.len();
    let total = sources.iter().map(|s| s.size).sum();
    let mut done = 0u64;
    let mut stage = NamedTempFile::new_in(&parent)?;
    let mut encoder = Encoder::new(stage.as_file_mut(), kind.codec)?;
    if kind.tar {
        let mut builder = tar::Builder::new(&mut encoder);
        for (i, source) in sources.into_iter().enumerate() {
            check(cancel)?;
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(if source.dir {
                tar::EntryType::Directory
            } else {
                tar::EntryType::Regular
            });
            header.set_mode(if source.dir { 0o755 } else { 0o644 });
            header.set_size(source.size);
            header.set_mtime(0);
            header.set_cksum();
            let reader: Box<dyn Read> = if source.dir {
                Box::new(Cursor::new(Vec::<u8>::new()))
            } else {
                Box::new(File::open(&source.path)?)
            };
            let mut reader = BudgetReader {
                inner: reader,
                cancel,
                read: 0,
                limit: source.size,
            };
            builder.append_data(&mut header, &source.name, &mut reader)?;
            if reader.read != source.size {
                return Err(invalid("Source size changed while compressing"));
            }
            done += source.size;
            progress(Progress {
                operation: "create".into(),
                entry: source.name,
                completed_entries: i + 1,
                total_entries: count,
                bytes_processed: done,
                total_bytes: total,
            });
        }
        builder.finish()?;
    } else {
        let source = &sources[0];
        done = copy_limited(
            &mut File::open(&source.path)?,
            &mut encoder,
            Limits::default().max_file_bytes,
            cancel,
            &mut |n| {
                progress(Progress {
                    operation: "create".into(),
                    entry: source.name.clone(),
                    completed_entries: 0,
                    total_entries: 1,
                    bytes_processed: n,
                    total_bytes: total,
                })
            },
        )?;
        if done != source.size {
            return Err(invalid("Source size changed while compressing"));
        }
    }
    encoder.finish()?;
    stage.as_file_mut().sync_all()?;
    check(cancel)?;
    test(stage.path(), kind, cancel, &|_| {})?;
    stage
        .persist_noclobber(&output)
        .map_err(|e| invalid(format!("Archive was not published: {}", e.error)))?;
    inspect(&output, kind)
}
