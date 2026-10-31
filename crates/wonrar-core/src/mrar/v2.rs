//! MRAR/2: files packed into one solid stream, cut into independent units of
//! up to 16 MiB. Each unit is stored, LZMA2 or PPMd, chosen per unit from a
//! sample, and carries a SHA-256 of its decoded bytes. The index is a compact
//! binary record list (shared path prefixes, sizes; positions are implied by
//! entry order). See docs/MRAR-2-SPEC.md.
use super::*;

pub(super) const VERSION: u16 = 2;
const UNIT_SIZE: usize = 16 * 1024 * 1024;
const PPMD_ORDER: u32 = 6;
const PPMD_MEM_LOG: u8 = 26; // 64 MiB model
const PPMD_MEM_LOG_MAX: u8 = 26;
const LZMA_PRESET: u32 = 6;
const LZMA_DICT_LOG_MIN: u8 = 12;
const LZMA_DICT_LOG_MAX: u8 = 24; // 16 MiB
/// Upper bound on MRAR/2 worker threads.
const MAX_THREADS: usize = 12;
/// Rough peak per worker: the larger of an LZMA2 encoder (about 11x its
/// 16 MiB dictionary) or a 64 MiB PPMd model, plus three unit buffers.
const PER_THREAD_BYTES: u64 = 240 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Codec {
    Store,
    Lzma2 { dict_log: u8 },
    Ppmd { order: u8, mem_log: u8 },
}

#[derive(Debug)]
pub(super) struct Unit {
    codec: Codec,
    offset: u64,
    encoded_size: u64,
    decoded_size: u64,
    /// Offset of this unit's first byte in the solid stream.
    stream_start: u64,
    sha256: [u8; 32],
}

#[derive(Debug)]
pub(super) struct Entry2 {
    path: String,
    dir: bool,
    size: u64,
    /// Offset of the file's first byte in the solid stream.
    start: u64,
}

pub(super) struct Index2 {
    units: Vec<Unit>,
    entries: Vec<Entry2>,
}

// ---------- binary index ----------

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

struct Cursor2<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl Cursor2<'_> {
    fn byte(&mut self) -> Result<u8> {
        let b = *self
            .bytes
            .get(self.pos)
            .ok_or_else(|| invalid("MRAR/2 index is truncated"))?;
        self.pos += 1;
        Ok(b)
    }
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|e| *e <= self.bytes.len())
            .ok_or_else(|| invalid("MRAR/2 index is truncated"))?;
        let slice = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(slice)
    }
    /// LEB128, at most 10 bytes, minimal encoding only.
    fn varint(&mut self) -> Result<u64> {
        let mut v = 0u64;
        for i in 0..10 {
            let b = self.byte()?;
            if i == 9 && b > 1 {
                return Err(invalid("MRAR/2 index number overflows"));
            }
            v |= u64::from(b & 0x7f) << (7 * i);
            if b & 0x80 == 0 {
                if i > 0 && b == 0 {
                    return Err(invalid("MRAR/2 index number is not minimally encoded"));
                }
                return Ok(v);
            }
        }
        Err(invalid("MRAR/2 index number overflows"))
    }
    fn size(&mut self) -> Result<usize> {
        usize::try_from(self.varint()?).map_err(|_| invalid("MRAR/2 index count too large"))
    }
}

fn encode_index(index: &Index2) -> Vec<u8> {
    let mut out = Vec::new();
    put_varint(&mut out, index.units.len() as u64);
    for unit in &index.units {
        match unit.codec {
            Codec::Store => out.push(0),
            Codec::Lzma2 { dict_log } => out.extend([1, dict_log]),
            Codec::Ppmd { order, mem_log } => out.extend([2, order, mem_log]),
        }
        put_varint(&mut out, unit.encoded_size);
        put_varint(&mut out, unit.decoded_size);
        out.extend_from_slice(&unit.sha256);
    }
    put_varint(&mut out, index.entries.len() as u64);
    let mut previous: &[u8] = &[];
    for entry in &index.entries {
        out.push(u8::from(entry.dir));
        let path = entry.path.as_bytes();
        let shared = previous
            .iter()
            .zip(path)
            .take_while(|(a, b)| a == b)
            .count();
        put_varint(&mut out, shared as u64);
        put_varint(&mut out, (path.len() - shared) as u64);
        out.extend_from_slice(&path[shared..]);
        if !entry.dir {
            put_varint(&mut out, entry.size);
        }
        previous = path;
    }
    out
}

/// Parses and fully validates a decoded index against the data section length.
fn decode_index(bytes: &[u8], data_len: u64) -> Result<Index2> {
    let limits = Limits::default();
    let mut c = Cursor2 { bytes, pos: 0 };
    let unit_count = c.size()?;
    // Every unit needs at least 35 index bytes, so this also bounds allocation.
    if unit_count > bytes.len() / 35 + 1 {
        return Err(invalid("MRAR/2 unit count exceeds the index"));
    }
    let mut units = Vec::with_capacity(unit_count);
    let (mut offset, mut stream) = (0u64, 0u64);
    for _ in 0..unit_count {
        let codec = match c.byte()? {
            0 => Codec::Store,
            1 => {
                let dict_log = c.byte()?;
                if !(LZMA_DICT_LOG_MIN..=LZMA_DICT_LOG_MAX).contains(&dict_log) {
                    return Err(invalid("MRAR/2 LZMA2 dictionary outside limits"));
                }
                Codec::Lzma2 { dict_log }
            }
            2 => {
                let (order, mem_log) = (c.byte()?, c.byte()?);
                if !(2..=16).contains(&order) || !(11..=PPMD_MEM_LOG_MAX).contains(&mem_log) {
                    return Err(invalid("MRAR/2 PPMd model outside limits"));
                }
                Codec::Ppmd { order, mem_log }
            }
            _ => return Err(invalid("Unknown MRAR/2 codec")),
        };
        let encoded_size = c.varint()?;
        let decoded_size = c.varint()?;
        if decoded_size == 0
            || decoded_size > UNIT_SIZE as u64
            || encoded_size == 0
            || encoded_size > decoded_size
            || (codec == Codec::Store && encoded_size != decoded_size)
        {
            return Err(invalid("MRAR/2 unit sizes outside limits"));
        }
        let sha256: [u8; 32] = c.take(32)?.try_into().expect("32 bytes");
        units.push(Unit {
            codec,
            offset,
            encoded_size,
            decoded_size,
            stream_start: stream,
            sha256,
        });
        offset = offset
            .checked_add(encoded_size)
            .filter(|o| *o <= data_len)
            .ok_or_else(|| invalid("MRAR/2 units extend beyond the data section"))?;
        stream = stream
            .checked_add(decoded_size)
            .filter(|s| *s <= limits.max_total_bytes)
            .ok_or_else(|| invalid("MRAR/2 total size exceeds limit"))?;
    }
    if offset != data_len {
        return Err(invalid(
            "MRAR/2 units do not cover the data section exactly",
        ));
    }
    let entry_count = c.size()?;
    if entry_count > limits.max_entries || entry_count > bytes.len() {
        return Err(invalid("MRAR entry count exceeds limit"));
    }
    let mut entries = Vec::with_capacity(entry_count);
    let mut names = std::collections::HashSet::new();
    let mut previous: Vec<u8> = Vec::new();
    let mut position = 0u64;
    for _ in 0..entry_count {
        let dir = match c.byte()? {
            0 => false,
            1 => true,
            _ => return Err(invalid("Unknown MRAR/2 entry kind")),
        };
        let shared = c.size()?;
        let suffix = c.size()?;
        if shared > previous.len() || shared.saturating_add(suffix) > PATH_LIMIT {
            return Err(invalid("MRAR/2 path outside limits"));
        }
        let mut path_bytes = previous[..shared].to_vec();
        path_bytes.extend_from_slice(c.take(suffix)?);
        let path = String::from_utf8(path_bytes.clone())
            .map_err(|_| invalid("MRAR/2 path is not UTF-8"))?;
        if path.split('/').count() > PATH_DEPTH_LIMIT
            || safe_name(&path).is_err()
            || safe_name(&path)? != path
        {
            return Err(invalid("MRAR requires canonical portable entry paths"));
        }
        if !names.insert(name_key(&path)) {
            return Err(invalid("MRAR has duplicate or ambiguous entry paths"));
        }
        let size = if dir { 0 } else { c.varint()? };
        if size > limits.max_file_bytes {
            return Err(invalid("MRAR file size exceeds limit"));
        }
        entries.push(Entry2 {
            path,
            dir,
            size,
            start: position,
        });
        position = position
            .checked_add(size)
            .filter(|p| *p <= stream)
            .ok_or_else(|| invalid("MRAR/2 files exceed the decoded units"))?;
        previous = path_bytes;
    }
    if position != stream {
        return Err(invalid(
            "MRAR/2 files do not cover the decoded units exactly",
        ));
    }
    if c.pos != bytes.len() {
        return Err(invalid("MRAR/2 index has trailing bytes"));
    }
    Ok(Index2 { units, entries })
}

// ---------- codecs ----------

fn ppmd_mem(mem_log: u8) -> u32 {
    1u32 << mem_log
}

fn encode_with(codec: Codec, bytes: &[u8]) -> Result<Vec<u8>> {
    Ok(match codec {
        Codec::Store => bytes.to_vec(),
        Codec::Lzma2 { dict_log } => {
            let mut options = lzma_rust2::Lzma2Options::with_preset(LZMA_PRESET);
            options.lzma_options.dict_size = 1 << dict_log;
            let mut w = lzma_rust2::Lzma2Writer::new(Vec::new(), options);
            w.write_all(bytes)?;
            w.finish()?
        }
        Codec::Ppmd { order, mem_log } => {
            let mut w = ppmd_rust::Ppmd7Encoder::new(Vec::new(), order.into(), ppmd_mem(mem_log))
                .map_err(|e| invalid(format!("PPMd encoder: {e}")))?;
            w.write_all(bytes)?;
            w.finish(false)?
        }
    })
}

fn lzma_for(len: usize) -> Codec {
    let log = (len.max(1).next_power_of_two().trailing_zeros() as u8)
        .clamp(LZMA_DICT_LOG_MIN, LZMA_DICT_LOG_MAX);
    Codec::Lzma2 { dict_log: log }
}
const PPMD: Codec = Codec::Ppmd {
    order: PPMD_ORDER as u8,
    mem_log: PPMD_MEM_LOG,
};

/// Chooses and applies a codec. Deterministic: depends only on the unit bytes.
/// Deterministic, evenly spread probe position: a golden-ratio sequence, so
/// probes never line up with regular layouts (for example alternating 1 MiB
/// text and binary).
fn probe_at(i: u64, len: usize, width: usize) -> std::ops::Range<usize> {
    let fraction = i.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32;
    let start = ((fraction * len as u64) >> 32) as usize;
    start.min(len)..(start + width).min(len)
}

/// Picks a codec from probes of the unit:
/// - 64 probes of 4 KiB, all incompressible: store, unless a fast
///   long-window pass over the whole unit finds repeats (then LZMA2);
/// - some incompressible: LZMA2 (PPMd is very slow on random data);
/// - on eight 64 KiB slices, PPMd at least 10 % smaller: PPMd (typical for text);
/// - otherwise both run on the whole unit and the smaller wins, because a
///   sample cannot show LZMA2's long-range matches between similar files.
fn compress_unit(bytes: &[u8]) -> Result<(Codec, Vec<u8>)> {
    const PROBES: u64 = 64;
    let mut incompressible = 0;
    for i in 1..=PROBES {
        let probe = &bytes[probe_at(i, bytes.len(), 4 << 10)];
        if probe.is_empty()
            || zstd::bulk::compress(probe, 1)?.len() as f64 > probe.len() as f64 * 0.97
        {
            incompressible += 1;
        }
    }
    let lzma = lzma_for(bytes.len());
    let candidates = match incompressible {
        PROBES => {
            // Random-looking probes can still repeat each other (duplicate
            // files): one fast long-window pass over the whole unit decides.
            let mut quick = zstd::bulk::Compressor::new(1)?;
            quick.set_parameter(zstd::zstd_safe::CParameter::WindowLog(24))?;
            if (quick.compress(bytes)?.len() as f64) < bytes.len() as f64 * 0.97 {
                vec![lzma]
            } else {
                vec![]
            }
        }
        1.. => vec![lzma],
        _ => {
            let mut sample = Vec::with_capacity(8 << 16);
            for i in 1..=8 {
                sample.extend_from_slice(&bytes[probe_at(i, bytes.len(), 64 << 10)]);
            }
            let p = encode_with(PPMD, &sample)?.len() as f64;
            let l = encode_with(lzma_for(sample.len()), &sample)?.len() as f64;
            if p < l * 0.9 {
                vec![PPMD]
            } else if l <= p {
                vec![lzma]
            } else {
                vec![lzma, PPMD]
            }
        }
    };
    let mut best = (Codec::Store, bytes.to_vec());
    for codec in candidates {
        let encoded = encode_with(codec, bytes)?;
        if encoded.len() < best.1.len() {
            best = (codec, encoded);
        }
    }
    Ok(best)
}

fn decode_unit_bytes(unit: &Unit, encoded: Vec<u8>) -> Result<Vec<u8>> {
    let size = unit.decoded_size as usize;
    let bytes = match unit.codec {
        Codec::Store => encoded,
        Codec::Lzma2 { dict_log } => {
            let encoded_len = encoded.len() as u64;
            let mut reader =
                lzma_rust2::Lzma2Reader::new(Cursor::new(encoded), 1 << dict_log, None);
            let mut out = Vec::with_capacity(size);
            Read::take(&mut reader, unit.decoded_size + 1).read_to_end(&mut out)?;
            if reader.into_inner().position() != encoded_len {
                return Err(invalid("MRAR/2 LZMA2 unit has trailing bytes"));
            }
            out
        }
        Codec::Ppmd { order, mem_log } => {
            let mut decoder =
                ppmd_rust::Ppmd7Decoder::new(Cursor::new(encoded), order.into(), ppmd_mem(mem_log))
                    .map_err(|e| invalid(format!("PPMd decoder: {e}")))?;
            let mut out = vec![0; size];
            decoder.read_exact(&mut out)?;
            out
        }
    };
    if bytes.len() != size {
        return Err(invalid(
            "MRAR actual block expansion differs from declared length",
        ));
    }
    if Sha256::digest(&bytes).as_slice() != unit.sha256 {
        return Err(invalid("MRAR block SHA-256 mismatch"));
    }
    Ok(bytes)
}

fn read_unit(file: &mut File, data_start: u64, unit: &Unit) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(
        data_start
            .checked_add(unit.offset)
            .ok_or_else(|| invalid("MRAR offset overflow"))?,
    ))?;
    let mut encoded = vec![0; unit.encoded_size as usize];
    file.read_exact(&mut encoded)?;
    decode_unit_bytes(unit, encoded)
}

/// Worker count limited by cores, `MAX_THREADS` and a quarter of available
/// memory. Archive bytes never depend on it.
fn threads_for(requested: usize) -> usize {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    let budget = system.available_memory() / 4;
    let by_memory = (budget / PER_THREAD_BYTES).max(1) as usize;
    requested.clamp(1, MAX_THREADS).min(by_memory)
}
fn threads() -> usize {
    threads_for(worker_threads())
}

// ---------- reader ----------

pub(super) struct Archive2 {
    file: File,
    path: PathBuf,
    index: Index2,
    data_start: u64,
    identity: String,
    length: u64,
    cache: Option<(usize, Vec<u8>)>,
}
impl Archive2 {
    pub(super) fn open(envelope: Envelope, path: &Path) -> Result<Self> {
        let index = decode_index(&envelope.index, envelope.data_len)?;
        Ok(Self {
            file: envelope.file,
            path: path.to_path_buf(),
            index,
            data_start: envelope.data_start,
            identity: envelope.identity,
            length: envelope.length,
            cache: None,
        })
    }
    /// Index of the unit holding stream offset `pos` (pos < total).
    fn unit_at(&self, pos: u64) -> usize {
        self.index
            .units
            .partition_point(|u| u.stream_start + u.decoded_size <= pos)
    }
    fn unit(&mut self, id: usize) -> Result<&[u8]> {
        if self.cache.as_ref().is_none_or(|(c, _)| *c != id) {
            self.cache = None;
            let bytes = read_unit(&mut self.file, self.data_start, &self.index.units[id])?;
            self.cache = Some((id, bytes));
        }
        Ok(&self.cache.as_ref().expect("cached above").1)
    }
    /// Reads up to `max` bytes of entry `id`.
    fn prefix(&mut self, id: usize, max: u64, cancel: &AtomicBool) -> Result<EntryBytes> {
        let entry = self
            .index
            .entries
            .get(id)
            .ok_or_else(|| invalid("MRAR entry ID not found"))?;
        if entry.dir {
            return Err(invalid("Directories do not contain bytes"));
        }
        let (start, size) = (entry.start, entry.size);
        let want = size.min(max);
        let mut bytes = Vec::with_capacity(want as usize);
        let mut pos = start;
        while (bytes.len() as u64) < want {
            check(cancel)?;
            let id = self.unit_at(pos);
            let unit_start = self.index.units[id].stream_start;
            let unit = self.unit(id)?;
            let from = (pos - unit_start) as usize;
            let n = (unit.len() - from).min((want - bytes.len() as u64) as usize);
            bytes.extend_from_slice(&unit[from..from + n]);
            pos += n as u64;
        }
        Ok(EntryBytes {
            bytes,
            truncated: size > max,
        })
    }
}

pub(super) fn entry_bytes(
    archive: &mut Archive2,
    id: &str,
    max: u64,
    cancel: &AtomicBool,
) -> Result<EntryBytes> {
    let id = id.parse().map_err(|_| invalid("Invalid entry ID"))?;
    archive.prefix(id, max, cancel)
}

pub(super) fn inspect(archive: Archive2, path: &Path) -> Result<ArchiveInfo> {
    let mut findings = vec![
        Finding{severity:"info".into(),code:"experimental_format".into(),message:"Experimental MRAR/2 is for disposable test data. It is not a stable backup or exchange format.".into(),entry_id:None},
        Finding{severity:"info".into(),code:"inspection_coverage".into(),message:"Bounded index and whole-container SHA-256 verified. Inventory does not decode units or establish file integrity, sender authenticity or malware safety. Run Test to verify every unit. Compressed sizes are proportional estimates; reading one file decodes the units it spans (up to 16 MiB each).".into(),entry_id:None},
    ];
    let units = &archive.index.units;
    let mut total = 0u64;
    let entries = archive
        .index
        .entries
        .iter()
        .enumerate()
        .map(|(id, e)| {
            total += e.size;
            // Proportional share of every unit the file overlaps.
            let mut compressed = 0u64;
            if e.size > 0 {
                let end = e.start + e.size;
                let first = units.partition_point(|u| u.stream_start + u.decoded_size <= e.start);
                for u in units[first..].iter().take_while(|u| u.stream_start < end) {
                    let overlap =
                        end.min(u.stream_start + u.decoded_size) - e.start.max(u.stream_start);
                    compressed += (u128::from(u.encoded_size) * u128::from(overlap)
                        / u128::from(u.decoded_size)) as u64;
                }
            }
            let extension = e.path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
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
            Entry {
                id: id.to_string(),
                name: e.path.clone(),
                size: e.size,
                compressed_size: compressed,
                is_dir: e.dir,
                is_encrypted: false,
                modified: None,
            }
        })
        .collect();
    if units
        .iter()
        .any(|u| u.encoded_size > 0 && u.decoded_size / u.encoded_size > 1000)
    {
        findings.push(Finding {
            severity: "warning".into(),
            code: "high_expansion".into(),
            message: "Declared unit expansion exceeds 1000:1.".into(),
            entry_id: None,
        });
    }
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

/// Decodes the given units on worker threads and passes them on in order.
fn decode_in_order(
    archive: &Archive2,
    ids: &[usize],
    cancel: &AtomicBool,
    consume: impl FnMut(&usize, Vec<u8>) -> Result<()>,
) -> Result<()> {
    let (path, start, units) = (&archive.path, archive.data_start, &archive.index.units);
    ordered_parallel(
        ids,
        threads(),
        cancel,
        || Ok(File::open(path)?),
        |file, &id| read_unit(file, start, &units[id]),
        consume,
    )
}

pub(super) fn test(
    archive: Archive2,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
) -> Result<Receipt> {
    let total: u64 = archive.index.units.iter().map(|u| u.decoded_size).sum();
    let count = archive.index.entries.len();
    let ids: Vec<usize> = (0..archive.index.units.len()).collect();
    let mut bytes = 0u64;
    decode_in_order(&archive, &ids, cancel, |&id, decoded| {
        bytes += decoded.len() as u64;
        let unit = &archive.index.units[id];
        let last = archive
            .index
            .entries
            .partition_point(|e| e.start < unit.stream_start + unit.decoded_size)
            .saturating_sub(1);
        progress(Progress {
            operation: "test".into(),
            entry: archive
                .index
                .entries
                .get(last)
                .map(|e| e.path.clone())
                .unwrap_or_default(),
            completed_entries: last,
            total_entries: count,
            bytes_processed: bytes,
            total_bytes: total,
        });
        Ok(())
    })?;
    let written = archive
        .index
        .entries
        .iter()
        .map(|e| e.path.clone())
        .collect();
    Ok(Receipt{operation:"test".into(),destination:None,written,skipped:vec![],bytes_processed:bytes,message:"Experimental MRAR/2 footer and every unit SHA-256 verified. Hashes do not authenticate the sender or detect malware.".into()})
}

/// Pulls decoded units from a channel and serves byte ranges of the solid stream
/// in increasing order, skipping bytes nobody asked for.
struct Feed<'a> {
    rx: std::sync::mpsc::Receiver<Result<(usize, Vec<u8>)>>,
    units: &'a [Unit],
    current: Option<(u64, Vec<u8>)>,
}
impl Feed<'_> {
    fn bytes_at(&mut self, pos: u64) -> Result<&[u8]> {
        loop {
            if let Some((start, bytes)) = &self.current {
                if pos >= *start && pos < start + bytes.len() as u64 {
                    let (start, bytes) = self.current.as_ref().expect("checked");
                    return Ok(&bytes[(pos - start) as usize..]);
                }
            }
            let (id, bytes) = self
                .rx
                .recv()
                .map_err(|_| invalid("MRAR/2 decoding stopped early"))??;
            self.current = Some((self.units[id].stream_start, bytes));
        }
    }
}
struct RangeReader<'f, 'a> {
    feed: &'f mut Feed<'a>,
    pos: u64,
    end: u64,
}
impl Read for RangeReader<'_, '_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.end || buf.is_empty() {
            return Ok(0);
        }
        let available = self.feed.bytes_at(self.pos).map_err(io::Error::other)?;
        let n = available
            .len()
            .min(buf.len())
            .min((self.end - self.pos) as usize);
        buf[..n].copy_from_slice(&available[..n]);
        self.pos += n as u64;
        Ok(n)
    }
}

pub(super) fn extract(
    archive: Archive2,
    p: &ExtractionPlan,
    root: &Dir,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
) -> Result<Receipt> {
    let mut r = Receipt {
        operation: "extract".into(),
        destination: Some(p.destination.clone()),
        written: vec![],
        skipped: vec![],
        bytes_processed: 0,
        message: String::new(),
    };
    // Files are written in stream order so every unit is decoded once.
    let mut work = Vec::new();
    for (i, planned) in p.entries.iter().enumerate() {
        check(cancel)?;
        if planned.action != "extract" {
            r.skipped.push(planned.source_name.clone());
            continue;
        }
        let id = planned
            .entry_id
            .parse::<usize>()
            .map_err(|_| invalid("Invalid entry ID"))?;
        let entry = archive
            .index
            .entries
            .get(id)
            .ok_or_else(|| invalid("MRAR entry ID not found"))?;
        if planned.is_dir {
            root.create_dir_all(safe_name(&planned.output_name)?)?;
            r.written.push(planned.output_name.clone());
        } else {
            work.push((entry.start, entry.size, i));
        }
    }
    work.sort_by_key(|w| w.0);
    let mut needed: Vec<usize> = Vec::new();
    for &(start, size, _) in &work {
        if size == 0 {
            continue;
        }
        let first = archive.unit_at(start);
        let last = archive.unit_at(start + size - 1);
        for id in first..=last {
            if needed.last().is_none_or(|l| *l < id) {
                needed.push(id);
            }
        }
    }
    std::thread::scope(|scope| -> Result<()> {
        let (tx, rx) = std::sync::mpsc::sync_channel(2);
        let archive = &archive;
        let needed = &needed;
        scope.spawn(move || {
            let result = decode_in_order(archive, needed, cancel, |&id, bytes| {
                tx.send(Ok((id, bytes)))
                    .map_err(|_| invalid("MRAR/2 extraction stopped"))
            });
            if let Err(e) = result {
                let _ = tx.send(Err(e));
            }
        });
        let mut feed = Feed {
            rx,
            units: &archive.index.units,
            current: None,
        };
        for &(start, size, i) in &work {
            check(cancel)?;
            let planned = &p.entries[i];
            let mut reader = RangeReader {
                feed: &mut feed,
                pos: start,
                end: start + size,
            };
            r.bytes_processed += write_planned(
                root,
                planned,
                &mut reader,
                p,
                cancel,
                progress,
                i,
                r.bytes_processed,
            )?;
            r.written.push(planned.output_name.clone());
        }
        Ok(())
    })?;
    r.message = format!(
        "Extracted {} experimental MRAR/2 entries; skipped {}. Every unit read was SHA-256 verified; no malware scan was performed.",
        r.written.len(),
        r.skipped.len()
    );
    Ok(r)
}

// ---------- writer ----------

/// What a worker reports for a unit: codec, decoded size and SHA-256.
type UnitMeta = (Codec, u64, [u8; 32]);

pub(super) fn create(
    output: &Path,
    inputs: &[PathBuf],
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
    threads: usize,
) -> Result<ArchiveInfo> {
    let output = absolute(output)?;
    let parent = output
        .parent()
        .ok_or_else(|| invalid("No output directory"))?
        .canonicalize()?;
    let output = parent.join(
        output
            .file_name()
            .ok_or_else(|| invalid("No output filename"))?,
    );
    let sources = sources(&output, inputs, cancel)?;
    let count = sources.len();
    let total: u64 = sources.iter().map(|s| s.size).sum();
    let mut data = NamedTempFile::new_in(&parent)?;
    let threads = threads_for(threads);
    let mut entries = Vec::with_capacity(count);

    let work = |bytes: Vec<u8>| -> Result<(Vec<u8>, UnitMeta)> {
        let sha256: [u8; 32] = Sha256::digest(&bytes).into();
        let decoded = bytes.len() as u64;
        let (codec, encoded) = compress_unit(&bytes)?;
        Ok((encoded, (codec, decoded, sha256)))
    };
    let placed = std::thread::scope(|scope| -> Result<Vec<Placed<UnitMeta>>> {
        let mut pipeline = Pipeline::start(scope, data.as_file_mut(), cancel, threads, &work);
        let produced = (|| -> Result<()> {
            let mut unit = Vec::with_capacity(UNIT_SIZE);
            let mut position = 0u64;
            for (i, source) in sources.into_iter().enumerate() {
                check(cancel)?;
                entries.push(Entry2 {
                    path: source.name.clone(),
                    dir: source.dir,
                    size: source.size,
                    start: position,
                });
                if source.dir || source.size == 0 {
                    continue;
                }
                let mut file = File::open(&source.path)?;
                let mut remaining = source.size;
                while remaining > 0 {
                    check(cancel)?;
                    let room = (UNIT_SIZE - unit.len()) as u64;
                    let want = remaining.min(room) as usize;
                    let at = unit.len();
                    unit.resize(at + want, 0);
                    file.read_exact(&mut unit[at..])
                        .map_err(|_| invalid("MRAR source size changed while compressing"))?;
                    remaining -= want as u64;
                    position += want as u64;
                    if unit.len() == UNIT_SIZE {
                        pipeline
                            .submit(std::mem::replace(&mut unit, Vec::with_capacity(UNIT_SIZE)))?;
                    }
                    progress(Progress {
                        operation: "create".into(),
                        entry: source.name.clone(),
                        completed_entries: i,
                        total_entries: count,
                        bytes_processed: position,
                        total_bytes: total,
                    });
                }
                if file.read(&mut [0])? != 0 {
                    return Err(invalid("MRAR source grew while compressing"));
                }
            }
            if !unit.is_empty() {
                pipeline.submit(unit)?;
            }
            Ok(())
        })();
        let finished = pipeline.finish();
        finished.and_then(|placed| produced.map(|()| placed))
    })?;

    let mut stream = 0u64;
    let units = placed
        .iter()
        .map(|p| {
            let (codec, decoded_size, sha256) = p.meta;
            let unit = Unit {
                codec,
                offset: p.offset,
                encoded_size: p.encoded_size,
                decoded_size,
                stream_start: stream,
                sha256,
            };
            stream += decoded_size;
            unit
        })
        .collect();
    let index = Index2 { units, entries };
    let data_len = placed.last().map_or(0, |p| p.offset + p.encoded_size);
    let bytes = encode_index(&index);
    if bytes.len() > INDEX_LIMIT {
        return Err(invalid("MRAR index exceeds 8 MiB limit"));
    }
    decode_index(&bytes, data_len)?; // the writer must produce what the reader accepts
    let mut index_compressor = zstd::bulk::Compressor::new(19)?;
    index_compressor.set_parameter(zstd::zstd_safe::CParameter::WindowLog(23))?;
    let encoded_index = index_compressor.compress(&bytes)?;
    write_container(
        &parent,
        &output,
        VERSION,
        &bytes,
        &encoded_index,
        data.as_file_mut(),
        data_len,
        cancel,
    )
}
