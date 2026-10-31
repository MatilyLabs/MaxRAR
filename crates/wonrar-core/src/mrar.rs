//! Experimental MRAR: legacy /0 and bounded grouped /1, with SHA-256 checks.
//! This is an original container implementation, not a renamed existing archive.
use super::*;
use std::io::{self, Cursor, Seek, SeekFrom};

mod v2;

const MAGIC: &[u8; 8] = b"MRAR\r\n\x1a\n";
const HEADER: u64 = 16;
const FOOTER: u64 = 32;
const INDEX_LIMIT: usize = 8 * 1024 * 1024;
const BLOCK_LIMIT: u64 = 1024 * 1024;
const GROUP_LIMIT: u64 = 8 * 1024 * 1024;
const SMALL_FILE_LIMIT: u64 = 256 * 1024;
const PATH_LIMIT: usize = 65535;
const PATH_DEPTH_LIMIT: usize = 256;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    version: u16,
    entries: Vec<Record>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    groups: Vec<Block>,
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Kind {
    File,
    Directory,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    path: String,
    kind: Kind,
    size: u64,
    sha256: String,
    blocks: Vec<Block>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    group: Option<Span>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Span {
    id: usize,
    offset: u64,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Codec {
    Store,
    Zstd,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Block {
    codec: Codec,
    offset: u64,
    encoded_size: u64,
    decoded_size: u64,
    sha256: String,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn json_error(e: serde_json::Error) -> Error {
    invalid(format!("Invalid MRAR index: {e}"))
}
pub(super) fn detect(path: &Path) -> Result<bool> {
    let mut file = File::open(path)?;
    let mut bytes = [0; 8];
    let n = file.read(&mut bytes)?;
    Ok(n == 8 && &bytes == MAGIC)
}

/// The version-independent container: header, decoded index bytes, data range
/// and the verified whole-container SHA-256.
struct Envelope {
    file: File,
    version: u16,
    index: Vec<u8>,
    data_start: u64,
    data_len: u64,
    identity: String,
    length: u64,
}

fn open_envelope(path: &Path, expected: Option<&str>, cancel: &AtomicBool) -> Result<Envelope> {
    check(cancel)?;
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    let limits = Limits::default();
    if length < HEADER + FOOTER
        || length
            > limits
                .max_total_bytes
                .saturating_add(INDEX_LIMIT as u64 + HEADER + FOOTER)
    {
        return Err(invalid("MRAR/0 archive length is outside format limits"));
    }
    let mut header = [0; 16];
    file.read_exact(&mut header)?;
    if &header[..8] != MAGIC {
        return Err(invalid("Invalid MRAR/0 magic"));
    }
    let version = u16::from_le_bytes([header[8], header[9]]);
    if version > v2::VERSION {
        return Err(invalid("Unsupported MRAR version"));
    }
    let flags = u16::from_le_bytes([header[10], header[11]]);
    if flags != version {
        return Err(invalid("Unsupported MRAR version/flags combination"));
    }
    let index_len = u32::from_le_bytes(
        header[12..16]
            .try_into()
            .map_err(|_| invalid("Invalid index length"))?,
    ) as usize;
    if index_len == 0 || index_len > INDEX_LIMIT {
        return Err(invalid("MRAR/0 index exceeds 8 MiB limit or is empty"));
    }
    let data_start = HEADER
        .checked_add(index_len as u64)
        .filter(|n| *n <= length - FOOTER)
        .ok_or_else(|| invalid("MRAR/0 index extends beyond archive"))?;
    let mut bytes = vec![0; index_len];
    file.read_exact(&mut bytes)?;
    if version >= 1 {
        if bytes.len() <= 4 {
            return Err(invalid("Missing compressed MRAR index"));
        }
        let decoded = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        if decoded == 0 || decoded > INDEX_LIMIT {
            return Err(invalid("MRAR decoded index exceeds 8 MiB limit"));
        }
        bytes = decode_frame(bytes[4..].to_vec(), decoded as u64, 23)?;
    }
    // Verify footer before returning any entry data. Identity includes footer.
    file.seek(SeekFrom::Start(0))?;
    let mut hasher = Sha256::new();
    let mut remaining = length - FOOTER;
    let mut buf = vec![0; 1 << 20];
    while remaining > 0 {
        check(cancel)?;
        let n = remaining.min(buf.len() as u64) as usize;
        file.read_exact(&mut buf[..n])?;
        hasher.update(&buf[..n]);
        remaining -= n as u64;
    }
    let mut footer = [0; 32];
    file.read_exact(&mut footer)?;
    let calculated = hasher.clone().finalize();
    if calculated.as_slice() != footer {
        return Err(invalid("MRAR/0 whole-container SHA-256 mismatch"));
    }
    hasher.update(footer);
    let identity = format!("{:x}", hasher.finalize());
    if expected.is_some_and(|v| v != identity) {
        return Err(invalid("Archive changed after review; generate a new plan"));
    }
    if file.metadata()?.len() != length {
        return Err(invalid("Archive length changed while reading"));
    }
    Ok(Envelope {
        file,
        version,
        index: bytes,
        data_start,
        data_len: length - FOOTER - data_start,
        identity,
        length,
    })
}

/// An open archive of any supported version.
enum Opened {
    Legacy(Archive),
    V2(v2::Archive2),
}
fn open_any(path: &Path, expected: Option<&str>, cancel: &AtomicBool) -> Result<Opened> {
    let envelope = open_envelope(path, expected, cancel)?;
    Ok(if envelope.version == v2::VERSION {
        Opened::V2(v2::Archive2::open(envelope, path)?)
    } else {
        Opened::Legacy(Archive::from_envelope(envelope)?)
    })
}

struct Archive {
    file: File,
    index: Index,
    data_start: u64,
    identity: String,
    length: u64,
    group_cache: Option<(usize, Vec<u8>)>,
}
impl Archive {
    fn from_envelope(envelope: Envelope) -> Result<Self> {
        let Envelope {
            file,
            version,
            index: bytes,
            data_start,
            data_len,
            identity,
            length,
        } = envelope;
        let index: Index = serde_json::from_slice(&bytes).map_err(json_error)?;
        if index.version != version {
            return Err(invalid("MRAR header/index version mismatch"));
        }
        if version == 0 {
            // Preserve the original strict schema, even for explicit null/empty fields.
            let raw: serde_json::Value = serde_json::from_slice(&bytes).map_err(json_error)?;
            if raw.get("groups").is_some()
                || raw["entries"]
                    .as_array()
                    .is_some_and(|entries| entries.iter().any(|e| e.get("group").is_some()))
            {
                return Err(invalid("MRAR/0 cannot contain grouped fields"));
            }
        }
        validate(&index, data_len)?;
        Ok(Self {
            file,
            index,
            data_start,
            identity,
            length,
            group_cache: None,
        })
    }
    fn reader<'a>(&'a mut self, id: usize, cancel: &'a AtomicBool) -> Result<EntryReader<'a>> {
        let entry = self
            .index
            .entries
            .get(id)
            .ok_or_else(|| invalid("MRAR entry ID not found"))?;
        if entry.kind != Kind::File {
            return Err(invalid("Directories do not contain bytes"));
        }
        Ok(EntryReader {
            file: &mut self.file,
            entry,
            data_start: self.data_start,
            next: 0,
            buffer: Vec::new(),
            position: 0,
            hasher: Sha256::new(),
            checked: false,
            cancel,
            groups: &self.index.groups,
            group_cache: &mut self.group_cache,
            group_read: false,
        })
    }
}

fn validate(index: &Index, data_len: u64) -> Result<()> {
    let limits = Limits::default();
    if index.version > 1 || (index.version == 0 && !index.groups.is_empty()) {
        return Err(invalid("Unsupported MRAR index version"));
    }
    if index.entries.len() > limits.max_entries {
        return Err(invalid("MRAR entry count exceeds limit"));
    }
    let mut names = std::collections::HashMap::new();
    let mut ranges = Vec::new();
    if index.groups.len() > limits.max_entries {
        return Err(invalid("Too many MRAR groups"));
    }
    let mut spans = vec![Vec::new(); index.groups.len()];
    for group in &index.groups {
        validate_block(group, GROUP_LIMIT, data_len, &mut ranges)?;
    }
    let mut total = 0u64;
    let mut last_group = None;
    for entry in &index.entries {
        if entry.path.len() > PATH_LIMIT
            || entry.path.split('/').count() > PATH_DEPTH_LIMIT
            || safe_name(&entry.path).is_err()
            || safe_name(&entry.path)? != entry.path
        {
            return Err(invalid("MRAR requires canonical portable entry paths"));
        }
        if names.insert(name_key(&entry.path), &entry.kind).is_some() {
            return Err(invalid("MRAR has duplicate or ambiguous entry paths"));
        }
        if entry.kind == Kind::Directory {
            if entry.size != 0
                || !entry.sha256.is_empty()
                || !entry.blocks.is_empty()
                || entry.group.is_some()
            {
                return Err(invalid("MRAR directory must have no data or hash"));
            }
            continue;
        }
        if entry.size > limits.max_file_bytes || !valid_hash(&entry.sha256) {
            return Err(invalid("MRAR file size or SHA-256 is invalid"));
        }
        let mut decoded = 0u64;
        if let Some(span) = &entry.group {
            if index.version != 1
                || !entry.blocks.is_empty()
                || entry.size == 0
                || entry.size > SMALL_FILE_LIMIT
            {
                return Err(invalid("Invalid MRAR grouped file"));
            }
            if last_group.is_some_and(|last| span.id < last) {
                return Err(invalid(
                    "MRAR group references must be in nondecreasing order",
                ));
            }
            last_group = Some(span.id);
            let group = index
                .groups
                .get(span.id)
                .ok_or_else(|| invalid("MRAR group ID outside table"))?;
            let end = span
                .offset
                .checked_add(entry.size)
                .filter(|v| *v <= group.decoded_size)
                .ok_or_else(|| invalid("MRAR file extends outside group"))?;
            spans[span.id].push((span.offset, end));
            decoded = entry.size;
        }
        for block in &entry.blocks {
            validate_block(block, BLOCK_LIMIT, data_len, &mut ranges)?;
            decoded = decoded
                .checked_add(block.decoded_size)
                .filter(|v| *v <= entry.size)
                .ok_or_else(|| invalid("MRAR block decoded lengths exceed file size"))?;
        }
        if decoded != entry.size {
            return Err(invalid("MRAR file size differs from block lengths"));
        }
        if entry.size == 0 && entry.sha256 != digest(&[]) {
            return Err(invalid("Empty MRAR file has incorrect SHA-256"));
        }
        total = total
            .checked_add(entry.size)
            .filter(|v| *v <= limits.max_total_bytes)
            .ok_or_else(|| invalid("MRAR decoded total exceeds limit"))?;
    }
    for (i, members) in spans.iter_mut().enumerate() {
        members.sort_unstable();
        let mut end = 0;
        for &(start, next) in members.iter() {
            if start != end {
                return Err(invalid("MRAR group slices overlap or leave gaps"));
            }
            end = next;
        }
        if end != index.groups[i].decoded_size {
            return Err(invalid("MRAR group has unreferenced bytes"));
        }
    }
    // /0 retains entry/block order; /1 interleaves independently stored groups.
    if index.version == 1 {
        ranges.sort_unstable();
    }
    let mut offset = 0;
    for (start, end) in ranges {
        if start != offset {
            return Err(invalid(
                "MRAR block offsets overlap, have gaps, or are out of order",
            ));
        }
        offset = end;
    }
    if offset != data_len {
        return Err(invalid("MRAR has unreferenced or trailing data bytes"));
    }
    for entry in &index.entries {
        let mut parent = entry.path.as_str();
        while let Some((prefix, _)) = parent.rsplit_once('/') {
            if names
                .get(&name_key(prefix))
                .is_some_and(|kind| **kind == Kind::File)
            {
                return Err(invalid("MRAR file path is also used as a directory"));
            }
            parent = prefix;
        }
    }
    Ok(())
}

fn validate_block(
    block: &Block,
    limit: u64,
    data_len: u64,
    ranges: &mut Vec<(u64, u64)>,
) -> Result<()> {
    if block.decoded_size == 0
        || block.decoded_size > limit
        || block.encoded_size == 0
        || block.encoded_size > block.decoded_size
        || !valid_hash(&block.sha256)
    {
        return Err(invalid("MRAR block size or SHA-256 is invalid"));
    }
    if matches!(block.codec, Codec::Store) && block.encoded_size != block.decoded_size {
        return Err(invalid(
            "Stored MRAR block has unequal encoded/decoded lengths",
        ));
    }
    let end = block
        .offset
        .checked_add(block.encoded_size)
        .filter(|v| *v <= data_len)
        .ok_or_else(|| invalid("MRAR block extends outside data section"))?;
    ranges.push((block.offset, end));
    Ok(())
}

fn decode_frame(encoded: Vec<u8>, decoded_size: u64, window: u32) -> Result<Vec<u8>> {
    if !encoded.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        return Err(invalid("MRAR requires a standard Zstandard frame"));
    }
    let encoded_size = encoded.len() as u64;
    let mut decoder =
        zstd::stream::read::Decoder::with_buffer(Cursor::new(encoded))?.single_frame();
    decoder.window_log_max(window)?;
    let mut bytes = Vec::new();
    Read::take(&mut decoder, decoded_size + 1).read_to_end(&mut bytes)?;
    if decoder.finish().position() != encoded_size {
        return Err(invalid(
            "MRAR Zstandard block has trailing or incomplete frame bytes",
        ));
    }
    if bytes.len() as u64 != decoded_size {
        return Err(invalid(
            "MRAR actual block expansion differs from declared length",
        ));
    }
    Ok(bytes)
}

fn decode_block(file: &mut File, data_start: u64, block: &Block, window: u32) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(
        data_start
            .checked_add(block.offset)
            .ok_or_else(|| invalid("MRAR offset overflow"))?,
    ))?;
    let mut encoded = vec![0; block.encoded_size as usize];
    file.read_exact(&mut encoded)?;
    let bytes = match block.codec {
        Codec::Store => encoded,
        Codec::Zstd => decode_frame(encoded, block.decoded_size, window)?,
    };
    if digest(&bytes) != block.sha256 {
        return Err(invalid("MRAR block SHA-256 mismatch"));
    }
    Ok(bytes)
}

struct EntryReader<'a> {
    file: &'a mut File,
    entry: &'a Record,
    data_start: u64,
    next: usize,
    buffer: Vec<u8>,
    position: usize,
    hasher: Sha256,
    checked: bool,
    cancel: &'a AtomicBool,
    groups: &'a [Block],
    group_cache: &'a mut Option<(usize, Vec<u8>)>,
    group_read: bool,
}
impl EntryReader<'_> {
    fn fill(&mut self) -> Result<bool> {
        check(self.cancel)?;
        if let Some(span) = &self.entry.group {
            if !self.group_read {
                if self
                    .group_cache
                    .as_ref()
                    .is_none_or(|(id, _)| *id != span.id)
                {
                    // Drop the previous group before allocating its replacement.
                    *self.group_cache = None;
                    let bytes =
                        decode_block(self.file, self.data_start, &self.groups[span.id], 23)?;
                    *self.group_cache = Some((span.id, bytes));
                }
                check(self.cancel)?;
                let group = &self.group_cache.as_ref().unwrap().1;
                self.buffer =
                    group[span.offset as usize..(span.offset + self.entry.size) as usize].to_vec();
                self.hasher.update(&self.buffer);
                self.position = 0;
                self.group_read = true;
                return Ok(true);
            }
        }
        let Some(block) = self.entry.blocks.get(self.next) else {
            if !self.checked {
                let actual = format!("{:x}", self.hasher.clone().finalize());
                if actual != self.entry.sha256 {
                    return Err(invalid("MRAR file SHA-256 mismatch"));
                }
                self.checked = true;
            }
            return Ok(false);
        };
        let bytes = decode_block(self.file, self.data_start, block, 20)?;
        self.hasher.update(&bytes);
        self.buffer = bytes;
        self.position = 0;
        self.next += 1;
        Ok(true)
    }
}
impl Read for EntryReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        check(self.cancel).map_err(io::Error::other)?;
        if self.position == self.buffer.len() && !self.fill().map_err(io::Error::other)? {
            return Ok(0);
        }
        let n = buf.len().min(self.buffer.len() - self.position);
        buf[..n].copy_from_slice(&self.buffer[self.position..self.position + n]);
        self.position += n;
        Ok(n)
    }
}

pub(super) fn inspect(path: &Path) -> Result<ArchiveInfo> {
    let flag = AtomicBool::new(false);
    let archive = match open_any(path, None, &flag)? {
        Opened::V2(archive) => return v2::inspect(archive, path),
        Opened::Legacy(archive) => archive,
    };
    let mut total = 0u64;
    let mut findings=vec![Finding{severity:"info".into(),code:"experimental_format".into(),message:format!("Experimental MRAR/{} is for disposable test data. It is not a stable backup or exchange format.",archive.index.version),entry_id:None},Finding{severity:"info".into(),code:"inspection_coverage".into(),message:"Bounded index and whole-container SHA-256 verified. Inventory does not decode every block or establish file integrity, sender authenticity or malware safety. Run Test for payload hashes. Grouped file sizes are proportional payload estimates; reading one member can decode its group (up to 8 MiB).".into(),entry_id:None}];
    let entries = archive
        .index
        .entries
        .iter()
        .enumerate()
        .map(|(id, entry)| {
            total += entry.size;
            let compressed = entry.group.as_ref().map_or_else(
                || entry.blocks.iter().map(|b| b.encoded_size).sum::<u64>(),
                |span| {
                    let group = &archive.index.groups[span.id];
                    group.encoded_size * entry.size / group.decoded_size
                },
            );
            let extension = entry
                .path
                .rsplit('.')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            if [
                "exe", "msi", "sh", "bat", "cmd", "ps1", "app", "scr", "dll", "js", "vbs",
            ]
            .contains(&extension.as_str())
            {
                findings.push(Finding {
                    severity: "warning".into(),
                    code: "executable_content".into(),
                    message: "Executable or script entry; this is not a malware verdict.".into(),
                    entry_id: Some(id.to_string()),
                });
            }
            if compressed > 0 && entry.size / compressed > 1000 {
                findings.push(Finding {
                    severity: "warning".into(),
                    code: "high_expansion".into(),
                    message: "Declared block expansion exceeds 1000:1.".into(),
                    entry_id: Some(id.to_string()),
                });
            }
            Entry {
                id: id.to_string(),
                name: entry.path.clone(),
                size: entry.size,
                compressed_size: compressed,
                is_dir: entry.kind == Kind::Directory,
                is_encrypted: false,
                modified: None,
            }
        })
        .collect();
    Ok(ArchiveInfo {
        path: absolute(path)?.to_string_lossy().into(),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        format: "mrar".into(),
        identity: archive.identity,
        entries,
        total_size: total,
        compressed_size: archive.length,
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
    id: &str,
    max: u64,
    cancel: &AtomicBool,
) -> Result<EntryBytes> {
    let mut archive = match open_any(path, None, cancel)? {
        Opened::V2(mut archive) => return v2::entry_bytes(&mut archive, id, max, cancel),
        Opened::Legacy(archive) => archive,
    };
    let id = id.parse().map_err(|_| invalid("Invalid entry ID"))?;
    read_entry_prefix(&mut archive.reader(id, cancel)?, max, cancel)
}

/// Reuses one verified group across sequential byte-inspection calls.
pub(super) struct InspectionSession(Opened);
impl InspectionSession {
    pub(super) fn open(path: &Path, identity: &str, cancel: &AtomicBool) -> Result<Self> {
        Ok(Self(open_any(path, Some(identity), cancel)?))
    }
    pub(super) fn read(&mut self, id: &str, max: u64, cancel: &AtomicBool) -> Result<EntryBytes> {
        match &mut self.0 {
            Opened::V2(archive) => v2::entry_bytes(archive, id, max, cancel),
            Opened::Legacy(archive) => {
                let id = id.parse().map_err(|_| invalid("Invalid entry ID"))?;
                read_entry_prefix(&mut archive.reader(id, cancel)?, max, cancel)
            }
        }
    }
}

/// Runs `work` over `items` on worker threads and hands each result to
/// `consume` in item order. At most `threads + 2` results are outstanding.
/// Every worker gets its own state from `init` (for example a file handle).
fn ordered_parallel<T: Sync, R: Send, S>(
    items: &[T],
    threads: usize,
    cancel: &AtomicBool,
    init: impl Fn() -> Result<S> + Sync,
    work: impl Fn(&mut S, &T) -> Result<R> + Sync,
    mut consume: impl FnMut(&T, R) -> Result<()>,
) -> Result<()> {
    use std::sync::atomic::AtomicUsize;
    use std::sync::mpsc::{channel, sync_channel};
    use std::sync::Mutex;
    if items.is_empty() {
        return Ok(());
    }
    let window = threads + 2;
    let (permit_tx, permit_rx) = sync_channel::<()>(window);
    for _ in 0..window {
        let _ = permit_tx.send(());
    }
    let permit_rx = Mutex::new(permit_rx);
    let next = AtomicUsize::new(0);
    let (done_tx, done_rx) = channel::<(usize, Result<R>)>();
    std::thread::scope(|scope| {
        for _ in 0..threads.min(items.len()) {
            let done_tx = done_tx.clone();
            let (permit_rx, next, init, work) = (&permit_rx, &next, &init, &work);
            scope.spawn(move || {
                let mut state = None;
                loop {
                    let permit = match permit_rx.lock() {
                        Ok(rx) => rx.recv(),
                        Err(_) => return,
                    };
                    if permit.is_err() {
                        return;
                    }
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if i >= items.len() {
                        return;
                    }
                    let result = (|| {
                        check(cancel)?;
                        if state.is_none() {
                            state = Some(init()?);
                        }
                        work(state.as_mut().expect("state set above"), &items[i])
                    })();
                    if done_tx.send((i, result)).is_err() {
                        return;
                    }
                }
            });
        }
        drop(done_tx);
        let consumed = (|| -> Result<()> {
            let mut pending = std::collections::BTreeMap::new();
            let mut expected = 0;
            for (i, result) in &done_rx {
                pending.insert(i, result);
                while let Some(result) = pending.remove(&expected) {
                    consume(&items[expected], result?)?;
                    expected += 1;
                    let _ = permit_tx.try_send(());
                }
                if expected == items.len() {
                    return Ok(());
                }
            }
            Err(invalid("MRAR verification stopped early"))
        })();
        // Wake and stop every worker before the scope joins them.
        drop(permit_tx);
        drop(done_rx);
        consumed
    })
}

pub(super) fn test(
    path: &Path,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
) -> Result<Receipt> {
    let archive = match open_any(path, None, cancel)? {
        Opened::V2(archive) => return v2::test(archive, cancel, progress),
        Opened::Legacy(archive) => archive,
    };
    let entries = &archive.index.entries;
    let total = entries.iter().map(|e| e.size).sum();
    let count = entries.len();

    // Units in the order a sequential reader meets them. Group IDs are
    // nondecreasing in entry order, so each group's members are contiguous.
    enum Job {
        Group(usize),
        Block(usize, usize),
    }
    let mut jobs = Vec::new();
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); archive.index.groups.len()];
    for (i, entry) in entries.iter().enumerate() {
        if entry.kind != Kind::File {
            continue;
        }
        if let Some(span) = &entry.group {
            if members[span.id].is_empty() {
                jobs.push(Job::Group(span.id));
            }
            members[span.id].push(i);
        } else {
            jobs.extend((0..entry.blocks.len()).map(|b| Job::Block(i, b)));
        }
    }
    let empty_ok = entries
        .iter()
        .filter(|e| e.kind == Kind::File && e.group.is_none() && e.blocks.is_empty())
        .all(|e| e.sha256 == digest(&[]));
    if !empty_ok {
        return Err(invalid("MRAR file SHA-256 mismatch"));
    }

    let mut bytes = 0u64;
    let mut hasher = Sha256::new();
    ordered_parallel(
        &jobs,
        worker_threads(),
        cancel,
        || Ok(File::open(path)?),
        |file, job| match *job {
            Job::Group(id) => decode_block(file, archive.data_start, &archive.index.groups[id], 23),
            Job::Block(i, b) => decode_block(file, archive.data_start, &entries[i].blocks[b], 20),
        },
        |job, decoded| {
            let (last, name) = match *job {
                Job::Group(id) => {
                    for &i in &members[id] {
                        let entry = &entries[i];
                        let span = entry.group.as_ref().expect("grouped entry");
                        let start = span.offset as usize;
                        if digest(&decoded[start..start + entry.size as usize]) != entry.sha256 {
                            return Err(invalid("MRAR file SHA-256 mismatch"));
                        }
                        bytes += entry.size;
                    }
                    let last = *members[id].last().expect("group has members");
                    (last, &entries[last].path)
                }
                Job::Block(i, b) => {
                    let entry = &entries[i];
                    hasher.update(&decoded);
                    bytes += decoded.len() as u64;
                    if b + 1 == entry.blocks.len()
                        && format!("{:x}", std::mem::take(&mut hasher).finalize()) != entry.sha256
                    {
                        return Err(invalid("MRAR file SHA-256 mismatch"));
                    }
                    (i, &entry.path)
                }
            };
            progress(Progress {
                operation: "test".into(),
                entry: name.clone(),
                completed_entries: last,
                total_entries: count,
                bytes_processed: bytes,
                total_bytes: total,
            });
            Ok(())
        },
    )?;
    let written = entries.iter().map(|e| e.path.clone()).collect();
    Ok(Receipt{operation:"test".into(),destination:None,written,skipped:vec![],bytes_processed:bytes,message:format!("Experimental MRAR/{} footer, every block/group and every file SHA-256 verified. Hashes do not authenticate the sender or detect malware.",archive.index.version)})
}

pub(super) fn extract(
    p: &ExtractionPlan,
    root: &Dir,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
) -> Result<Receipt> {
    let mut archive = match open_any(
        Path::new(&p.archive_path),
        Some(&p.archive_identity),
        cancel,
    )? {
        Opened::V2(archive) => return v2::extract(archive, p, root, cancel, progress),
        Opened::Legacy(archive) => archive,
    };
    let mut r = Receipt {
        operation: "extract".into(),
        destination: Some(p.destination.clone()),
        written: vec![],
        skipped: vec![],
        bytes_processed: 0,
        message: String::new(),
    };
    for (i, entry) in p.entries.iter().enumerate() {
        check(cancel)?;
        if entry.action != "extract" {
            r.skipped.push(entry.source_name.clone());
            continue;
        }
        let id = entry
            .entry_id
            .parse::<usize>()
            .map_err(|_| invalid("Invalid entry ID"))?;
        if entry.is_dir {
            root.create_dir_all(safe_name(&entry.output_name)?)?;
        } else {
            r.bytes_processed += write_planned(
                root,
                entry,
                &mut archive.reader(id, cancel)?,
                p,
                cancel,
                progress,
                i,
                r.bytes_processed,
            )?;
        }
        r.written.push(entry.output_name.clone());
    }
    r.message=format!("Extracted {} experimental MRAR/{} entries; skipped {}. Selected file and block/group hashes verified; no malware scan was performed.",r.written.len(),archive.index.version,r.skipped.len());
    Ok(r)
}

struct Source {
    path: PathBuf,
    name: String,
    dir: bool,
    size: u64,
}
fn sources(output: &Path, inputs: &[PathBuf], cancel: &AtomicBool) -> Result<Vec<Source>> {
    let mut result = Vec::new();
    let mut names = HashSet::new();
    let limits = Limits::default();
    let mut total = 0u64;
    for input in inputs {
        check(cancel)?;
        let input = absolute(input)?;
        if fs::symlink_metadata(&input)?.file_type().is_symlink() {
            return Err(invalid("MRAR source links are unsupported"));
        }
        let input = input.canonicalize()?;
        if output.starts_with(&input) {
            return Err(invalid(
                "MRAR output cannot be inside a source folder or equal an input",
            ));
        }
        let base = input
            .parent()
            .ok_or_else(|| invalid("Filesystem roots cannot be added"))?;
        for item in walkdir::WalkDir::new(&input)
            .follow_links(false)
            .sort_by_file_name()
        {
            check(cancel)?;
            let item = item.map_err(|e| invalid(e.to_string()))?;
            if !item.file_type().is_file() && !item.file_type().is_dir() {
                return Err(invalid("MRAR only supports ordinary files and directories"));
            }
            let name = item
                .path()
                .strip_prefix(base)
                .map_err(|e| invalid(e.to_string()))?
                .components()
                .map(|v| {
                    v.as_os_str()
                        .to_str()
                        .ok_or_else(|| invalid("MRAR source names must be UTF-8"))
                })
                .collect::<Result<Vec<_>>>()?
                .join("/");
            safe_name(&name)?;
            if name.len() > PATH_LIMIT
                || name.split('/').count() > PATH_DEPTH_LIMIT
                || !names.insert(name_key(&name))
            {
                return Err(invalid("MRAR source names collide or exceed path limits"));
            }
            let size = if item.file_type().is_dir() {
                0
            } else {
                fs::metadata(item.path())?.len()
            };
            if size > limits.max_file_bytes {
                return Err(invalid("MRAR source exceeds file size limit"));
            }
            total = total
                .checked_add(size)
                .filter(|n| *n <= limits.max_total_bytes)
                .ok_or_else(|| invalid("MRAR sources exceed total size limit"))?;
            result.push(Source {
                path: item.path().into(),
                name,
                dir: item.file_type().is_dir(),
                size,
            });
            if result.len() > limits.max_entries {
                return Err(invalid("Too many MRAR source entries"));
            }
        }
    }
    Ok(result)
}
/// Worker threads for MRAR compression and verification. `MAXRAR_THREADS`
/// overrides the default (available cores, at most 16). Output bytes do not
/// depend on the thread count: every unit is compressed independently and
/// written in input order.
fn worker_threads() -> usize {
    std::env::var("MAXRAR_THREADS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
        .min(16)
}

/// Where a compressed unit landed in the data section, with the worker's metadata.
struct Placed<R> {
    offset: u64,
    encoded_size: u64,
    meta: R,
}

type Work<'w, R> = dyn Fn(Vec<u8>) -> Result<(Vec<u8>, R)> + Sync + 'w;

/// Compresses units on worker threads and writes them to the data file in
/// submission order. At most `threads + 2` units are in flight, which bounds
/// memory to that many units plus their encoded copies.
struct Pipeline<'s, R> {
    jobs: Option<std::sync::mpsc::SyncSender<(usize, Vec<u8>)>>,
    permits: std::sync::mpsc::Receiver<()>,
    next: usize,
    writer: std::thread::ScopedJoinHandle<'s, Result<Vec<Placed<R>>>>,
}
impl<'s, R: Send + 's> Pipeline<'s, R> {
    fn start(
        scope: &'s std::thread::Scope<'s, '_>,
        data: &'s mut File,
        cancel: &'s AtomicBool,
        threads: usize,
        work: &'s Work<'s, R>,
    ) -> Self {
        use std::sync::mpsc::{channel, sync_channel};
        use std::sync::{Arc, Mutex};
        let (jobs, job_rx) = sync_channel::<(usize, Vec<u8>)>(threads);
        let job_rx = Arc::new(Mutex::new(job_rx));
        let (done_tx, done_rx) = channel::<(usize, Result<(Vec<u8>, R)>)>();
        let in_flight = threads + 2;
        let (permit_tx, permits) = sync_channel::<()>(in_flight);
        for _ in 0..in_flight {
            let _ = permit_tx.send(());
        }
        for _ in 0..threads {
            let job_rx = Arc::clone(&job_rx);
            let done_tx = done_tx.clone();
            scope.spawn(move || loop {
                let job = match job_rx.lock() {
                    Ok(rx) => rx.recv(),
                    Err(_) => return,
                };
                let Ok((seq, bytes)) = job else { return };
                let result = check(cancel).and_then(|()| work(bytes));
                if done_tx.send((seq, result)).is_err() {
                    return;
                }
            });
        }
        drop(done_tx);
        let writer = scope.spawn(move || -> Result<Vec<Placed<R>>> {
            let mut pending = std::collections::BTreeMap::new();
            let mut placed: Vec<Placed<R>> = Vec::new();
            let mut offset = 0u64;
            for (seq, result) in done_rx {
                pending.insert(seq, result?);
                while let Some((encoded, meta)) = pending.remove(&placed.len()) {
                    check(cancel)?;
                    data.write_all(&encoded)?;
                    let encoded_size = encoded.len() as u64;
                    placed.push(Placed {
                        offset,
                        encoded_size,
                        meta,
                    });
                    offset = offset
                        .checked_add(encoded_size)
                        .ok_or_else(|| invalid("MRAR data offset overflow"))?;
                    // Free a slot for the producer; it may already have finished.
                    let _ = permit_tx.try_send(());
                }
            }
            if !pending.is_empty() {
                return Err(invalid(
                    "MRAR compression stopped before every unit was written",
                ));
            }
            Ok(placed)
        });
        Self {
            jobs: Some(jobs),
            permits,
            next: 0,
            writer,
        }
    }
    /// Queues a unit. Blocks while the pipeline is full.
    fn submit(&mut self, bytes: Vec<u8>) -> Result<()> {
        let stopped = || invalid("MRAR compression stopped");
        self.permits.recv().map_err(|_| stopped())?;
        self.jobs
            .as_ref()
            .ok_or_else(stopped)?
            .send((self.next, bytes))
            .map_err(|_| stopped())?;
        self.next += 1;
        Ok(())
    }
    fn finish(mut self) -> Result<Vec<Placed<R>>> {
        self.jobs = None;
        let placed = self
            .writer
            .join()
            .map_err(|_| invalid("MRAR writer thread failed"))??;
        if placed.len() != self.next {
            return Err(invalid("MRAR compression lost a unit"));
        }
        Ok(placed)
    }
}

/// Writes header, compressed index, data and footer in one pass, self-tests the
/// staged archive and publishes it without replacing an existing file.
#[allow(clippy::too_many_arguments)]
fn write_container(
    parent: &Path,
    output: &Path,
    version: u16,
    index: &[u8],
    encoded_index: &[u8],
    data: &mut File,
    data_len: u64,
    cancel: &AtomicBool,
) -> Result<ArchiveInfo> {
    if encoded_index.len() + 4 > INDEX_LIMIT {
        return Err(invalid("MRAR encoded index exceeds 8 MiB limit"));
    }
    let mut stage = NamedTempFile::new_in(parent)?;
    let mut head = Vec::with_capacity(HEADER as usize + 4 + encoded_index.len());
    head.extend_from_slice(MAGIC);
    head.extend_from_slice(&version.to_le_bytes());
    head.extend_from_slice(&version.to_le_bytes());
    head.extend_from_slice(&((encoded_index.len() + 4) as u32).to_le_bytes());
    head.extend_from_slice(&(index.len() as u32).to_le_bytes());
    head.extend_from_slice(encoded_index);
    let mut hasher = Sha256::new();
    hasher.update(&head);
    stage.write_all(&head)?;
    data.seek(SeekFrom::Start(0))?;
    let mut buffer = vec![0; 1 << 20];
    let mut copied = 0u64;
    loop {
        check(cancel)?;
        let n = data.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        stage.write_all(&buffer[..n])?;
        copied += n as u64;
    }
    if copied != data_len {
        return Err(invalid("MRAR data section length changed while writing"));
    }
    stage.write_all(&hasher.finalize())?;
    stage.as_file_mut().sync_all()?;
    test(stage.path(), cancel, &|_| {})?;
    check(cancel)?;
    stage
        .persist_noclobber(output)
        .map_err(|e| invalid(format!("MRAR archive was not published: {}", e.error)))?;
    inspect(output)
}

/// Writes MRAR/2. Readers still accept MRAR/0 and MRAR/1.
pub(super) fn create(
    output: &Path,
    inputs: &[PathBuf],
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
    password: Option<&str>,
) -> Result<ArchiveInfo> {
    check(cancel)?;
    if password.is_some() {
        return Err(invalid(
            "Experimental MRAR does not support encryption. Choose ZIP or 7z.",
        ));
    }
    v2::create(output, inputs, cancel, progress, worker_threads())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_does_not_depend_on_thread_count() {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("src");
        std::fs::create_dir_all(src.join("small")).unwrap();
        // About 21 MiB of mixed small and large files: two MRAR/2 units, with
        // an incompressible tail and an empty file.
        for i in 0..900 {
            let body = format!("record {i} ").repeat(1 + i % 200);
            std::fs::write(src.join("small").join(format!("{i:04}.txt")), body).unwrap();
        }
        let mut large: Vec<u8> = (0..5_000_000u32)
            .flat_map(|i| (i % 251).to_le_bytes())
            .collect();
        let mut x = 0x2545_f491_4f6c_dd1du64;
        large.extend((0..(1 << 20)).map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        }));
        std::fs::write(src.join("large.bin"), &large).unwrap();
        std::fs::write(src.join("empty.txt"), b"").unwrap();
        let cancel = AtomicBool::new(false);
        let mut archives = Vec::new();
        for threads in [1, 3, 7] {
            let out = t.path().join(format!("t{threads}.mrar"));
            v2::create(&out, std::slice::from_ref(&src), &cancel, &|_| {}, threads).unwrap();
            archives.push(std::fs::read(&out).unwrap());
        }
        assert!(archives.iter().all(|a| *a == archives[0]));
        let receipt = test(&t.path().join("t7.mrar"), &cancel, &|_| {}).unwrap();
        assert_eq!(receipt.bytes_processed, input_bytes(&src));
    }

    fn input_bytes(root: &Path) -> u64 {
        walkdir::WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.metadata().unwrap().len())
            .sum()
    }
}
