use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use wonrar_core::*;

fn flag() -> AtomicBool {
    AtomicBool::new(false)
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn pack_raw(index: &[u8], data: &[u8]) -> Vec<u8> {
    let mut raw = b"MRAR\r\n\x1a\n".to_vec();
    raw.extend(0u16.to_le_bytes());
    raw.extend(0u16.to_le_bytes());
    raw.extend((index.len() as u32).to_le_bytes());
    raw.extend(index);
    raw.extend(data);
    let footer = Sha256::digest(&raw);
    raw.extend(footer);
    raw
}
fn pack(index: &Value, data: &[u8]) -> Vec<u8> {
    pack_raw(&serde_json::to_vec(index).unwrap(), data)
}
fn pack_v1(index: &Value, data: &[u8]) -> Vec<u8> {
    let json = serde_json::to_vec(index).unwrap();
    let mut encoded = (json.len() as u32).to_le_bytes().to_vec();
    encoded.extend(zstd::bulk::compress(&json, 3).unwrap());
    let mut raw = b"MRAR\r\n\x1a\n".to_vec();
    raw.extend(1u16.to_le_bytes());
    raw.extend(1u16.to_le_bytes());
    raw.extend((encoded.len() as u32).to_le_bytes());
    raw.extend(encoded);
    raw.extend(data);
    raw.extend(Sha256::digest(&raw));
    raw
}
fn base() -> Value {
    json!({"version":0,"entries":[{"path":"file.txt","kind":"file","size":3,"sha256":sha(b"abc"),"blocks":[{"codec":"store","offset":0,"encoded_size":3,"decoded_size":3,"sha256":sha(b"abc")}]}]})
}
fn write(path: &Path, index: &Value, data: &[u8]) {
    fs::write(path, pack(index, data)).unwrap();
}
fn random(size: usize) -> Vec<u8> {
    let mut value = 0x759b32e5u32;
    (0..size)
        .map(|_| {
            value ^= value << 13;
            value ^= value >> 17;
            value ^= value << 5;
            value as u8
        })
        .collect()
}

#[test]
fn real_container_multiblock_roundtrip_and_adaptive_store_are_verified() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("source");
    fs::create_dir(&src).unwrap();
    fs::create_dir(src.join("empty-dir")).unwrap();
    fs::write(src.join("empty.txt"), []).unwrap();
    fs::write(src.join("compressible.txt"), vec![b'a'; 1024 * 1024 + 27]).unwrap();
    let noise = random(1024 * 1024 + 57);
    fs::write(src.join("random.bin"), &noise).unwrap();
    fs::write(src.join("é.txt"), "Research source").unwrap();
    let path = t.path().join("roundtrip.mrar");
    let info = create(&path, std::slice::from_ref(&src), &flag(), &|_| {}).unwrap();
    assert_eq!(info.format, "mrar");
    assert!(!info.capabilities.can_comment);
    assert!(!info.capabilities.can_delete);
    assert_eq!(&fs::read(&path).unwrap()[..8], b"MRAR\r\n\x1a\n");
    assert!(info
        .findings
        .iter()
        .any(|f| f.code == "experimental_format"));
    let raw = fs::read(&path).unwrap();
    // New archives are MRAR/2: solid units with a codec chosen per unit.
    assert_eq!(&raw[8..12], &[2, 0, 2, 0]);
    assert!(
        raw.len() < noise.len() + 64 * 1024,
        "compressible content should shrink while noise is kept"
    );
    test(&path, &flag(), &|_| {}).unwrap();
    let id = info
        .entries
        .iter()
        .find(|e| e.name.ends_with("random.bin"))
        .unwrap()
        .id
        .clone();
    let prefix = inspect_entry_bytes(&path, &id, 8, None, &flag()).unwrap();
    assert_eq!(prefix.bytes, &noise[..8]);
    assert!(prefix.truncated);
    let dest = t.path().join("selected");
    let p = plan(
        &path,
        &dest,
        std::slice::from_ref(&id),
        ConflictPolicy::Skip,
        Limits::default(),
    )
    .unwrap();
    extract(&p, &flag(), &|_| {}).unwrap();
    assert_eq!(fs::read(dest.join("source/random.bin")).unwrap(), noise);
    assert!(!dest.join("source/compressible.txt").exists());
    let dest = t.path().join("all");
    let p = plan(&path, &dest, &[], ConflictPolicy::Skip, Limits::default()).unwrap();
    extract(&p, &flag(), &|_| {}).unwrap();
    assert_eq!(
        fs::read(dest.join("source/compressible.txt")).unwrap(),
        vec![b'a'; 1024 * 1024 + 27]
    );
    assert_eq!(
        fs::metadata(dest.join("source/empty.txt")).unwrap().len(),
        0
    );
    assert!(dest.join("source/empty-dir").is_dir());
    assert_eq!(
        fs::read_to_string(dest.join("source/é.txt")).unwrap(),
        "Research source"
    );
    let disguised = t.path().join("wrong.zip");
    fs::copy(&path, &disguised).unwrap();
    assert_eq!(inspect(&disguised).unwrap().format, "mrar");
    let second = t.path().join("deterministic.mrar");
    create(&second, &[src], &flag(), &|_| {}).unwrap();
    assert_eq!(fs::read(second).unwrap(), raw);
}

#[test]
fn strict_index_rejects_unknown_fields_versions_paths_offsets_and_lengths() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("bad.mrar");
    let mut cases = Vec::new();
    let mut v = base();
    v["unknown"] = json!(true);
    cases.push(v);
    let mut v = base();
    v["version"] = json!(1);
    cases.push(v);
    for path in [
        "../escape",
        "/absolute",
        "a\\b",
        "C:/file",
        "./file.txt",
        "a/../b",
        "CON.txt",
    ] {
        let mut v = base();
        v["entries"][0]["path"] = json!(path);
        cases.push(v);
    }
    let mut v = base();
    v["entries"][0]["path"] = json!((0..257).map(|_| "a").collect::<Vec<_>>().join("/"));
    cases.push(v);
    let mut v = base();
    v["entries"][0]["kind"] = json!("symlink");
    cases.push(v);
    for (key, value) in [
        ("codec", json!("rar")),
        ("offset", json!(u64::MAX)),
        ("encoded_size", json!(4)),
        ("decoded_size", json!(1024 * 1024 + 1)),
        ("sha256", json!("bad")),
    ] {
        let mut v = base();
        v["entries"][0]["blocks"][0][key] = value;
        cases.push(v);
    }
    let mut v = base();
    v["entries"][0]["size"] = json!(4);
    cases.push(v);
    let mut v = base();
    let duplicate = v["entries"][0].clone();
    v["entries"].as_array_mut().unwrap().push(duplicate);
    cases.push(v);
    let mut v = base();
    v["entries"][0]["path"] = json!("a/b");
    v["entries"]
        .as_array_mut()
        .unwrap()
        .push(json!({"path":"a","kind":"file","size":0,"sha256":sha(&[]),"blocks":[]}));
    cases.push(v);
    let mut v = base();
    v["entries"][0]["kind"] = json!("directory");
    cases.push(v);
    for (i, case) in cases.into_iter().enumerate() {
        write(&path, &case, b"abc");
        assert!(inspect(&path).is_err(), "hostile case {i}");
    }
    let duplicate = br#"{"version":0,"version":0,"entries":[]}"#;
    fs::write(&path, pack_raw(duplicate, &[])).unwrap();
    assert!(inspect(&path)
        .unwrap_err()
        .to_string()
        .contains("duplicate"));
    write(&path, &base(), b"abcX");
    assert!(inspect(&path).is_err());
    let mut raw = pack(&base(), b"abc");
    raw.push(0);
    fs::write(&path, raw).unwrap();
    assert!(inspect(&path).is_err());
    for (field, bytes) in [(8, 1u16.to_le_bytes()), (10, 1u16.to_le_bytes())] {
        let mut raw = pack(&base(), b"abc");
        raw[field..field + 2].copy_from_slice(&bytes);
        fs::write(&path, raw).unwrap();
        assert!(inspect(&path).is_err());
    }
    let mut raw = pack(&base(), b"abc");
    raw[12..16].copy_from_slice(&(8u32 * 1024 * 1024 + 1).to_le_bytes());
    fs::write(&path, raw).unwrap();
    assert!(inspect(&path).is_err());
}

#[test]
fn footer_block_and_file_corruption_fail_without_publishing_payload() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("bad.mrar");
    let mut raw = pack(&base(), b"abc");
    let last = raw.len() - 1;
    raw[last] ^= 1;
    fs::write(&path, raw).unwrap();
    assert!(inspect(&path)
        .unwrap_err()
        .to_string()
        .contains("whole-container"));
    for block_hash in [true, false] {
        let mut v = base();
        if block_hash {
            v["entries"][0]["blocks"][0]["sha256"] = json!(sha(b"xyz"));
        } else {
            v["entries"][0]["sha256"] = json!(sha(b"xyz"));
        }
        write(&path, &v, b"abc");
        inspect(&path).unwrap();
        assert!(test(&path, &flag(), &|_| {}).is_err());
        let dest = t.path().join(if block_hash { "block" } else { "file" });
        let p = plan(&path, &dest, &[], ConflictPolicy::Skip, Limits::default()).unwrap();
        assert!(extract(&p, &flag(), &|_| {}).is_err());
        assert_eq!(fs::read_dir(dest).unwrap().count(), 0);
    }
    for n in [0, 7, 15, 16, 31, 47] {
        let raw = pack(&base(), b"abc");
        fs::write(&path, &raw[..n]).unwrap();
        assert!(inspect(&path).is_err());
    }
}

#[test]
fn zstd_expansion_and_trailing_frames_are_rejected_with_recomputed_footer() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("bad.mrar");
    let decoded = vec![b'x'; 100000];
    let compressed = zstd::bulk::compress(&decoded, 3).unwrap();
    let mut v = base();
    v["entries"][0]["size"] = json!(512);
    v["entries"][0]["sha256"] = json!(sha(&decoded[..512]));
    v["entries"][0]["blocks"][0] = json!({"codec":"zstd","offset":0,"encoded_size":compressed.len(),"decoded_size":512,"sha256":sha(&decoded[..512])});
    write(&path, &v, &compressed);
    inspect(&path).unwrap();
    assert!(test(&path, &flag(), &|_| {}).is_err());
    let mut trailing = compressed.clone();
    trailing.extend(zstd::bulk::compress(&[], 3).unwrap());
    v["entries"][0]["size"] = json!(decoded.len());
    v["entries"][0]["sha256"] = json!(sha(&decoded));
    v["entries"][0]["blocks"][0] = json!({"codec":"zstd","offset":0,"encoded_size":trailing.len(),"decoded_size":decoded.len(),"sha256":sha(&decoded)});
    write(&path, &v, &trailing);
    inspect(&path).unwrap();
    assert!(test(&path, &flag(), &|_| {})
        .unwrap_err()
        .to_string()
        .contains("trailing"));
}

#[test]
fn cancellation_limits_no_clobber_and_encryption_boundaries_are_honest() {
    let t = tempfile::tempdir().unwrap();
    let source = t.path().join("file.txt");
    fs::write(&source, vec![b'a'; 2 * 1024 * 1024]).unwrap();
    let path = t.path().join("a.mrar");
    assert!(create_with_password(
        &path,
        std::slice::from_ref(&source),
        &flag(),
        &|_| {},
        Some("password")
    )
    .unwrap_err()
    .to_string()
    .contains("does not support encryption"));
    assert!(!path.exists());
    let cancelled = flag();
    assert!(
        create(&path, std::slice::from_ref(&source), &cancelled, &|_| {
            cancelled.store(true, Ordering::Relaxed)
        })
        .is_err()
    );
    assert!(!path.exists());
    assert_eq!(fs::read_dir(t.path()).unwrap().count(), 1);
    create(&path, std::slice::from_ref(&source), &flag(), &|_| {}).unwrap();
    let original = fs::read(&path).unwrap();
    assert!(create(&path, &[source], &flag(), &|_| {}).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(set_comment(&path, "unsupported", &flag()).is_err());
    assert!(delete_entries(&path, &["0".into()], &flag()).is_err());
    assert!(inspect_entry_bytes(&path, "0", 100, None, &AtomicBool::new(true)).is_err());
    assert!(plan(
        &path,
        &t.path().join("out"),
        &[],
        ConflictPolicy::Skip,
        Limits {
            max_total_bytes: 1,
            ..Limits::default()
        }
    )
    .is_err());
    let dest = t.path().join("out");
    let p = plan(&path, &dest, &[], ConflictPolicy::Skip, Limits::default()).unwrap();
    let cancelled = flag();
    assert!(extract(&p, &cancelled, &|_| cancelled
        .store(true, Ordering::Relaxed))
    .is_err());
    assert_eq!(fs::read_dir(&dest).unwrap().count(), 0);
}

#[test]
fn solid_units_cross_file_boundaries_and_selected_extraction_is_exact() {
    let t = tempfile::tempdir().unwrap();
    let source = t.path().join("related");
    fs::create_dir(&source).unwrap();
    let shared = random(128 * 1024);
    for i in 0..80 {
        let mut bytes = shared.clone();
        bytes[..4].copy_from_slice(&(i as u32).to_le_bytes());
        fs::write(source.join(format!("{i:03}.bin")), bytes).unwrap();
    }
    let path = t.path().join("grouped.mrar");
    let info = create(&path, std::slice::from_ref(&source), &flag(), &|_| {}).unwrap();
    let raw = fs::read(&path).unwrap();
    assert_eq!(&raw[8..12], &[2, 0, 2, 0]);
    assert!(
        raw.len() < shared.len() * 4,
        "shared random content should compress across files"
    );
    let cancel = flag();
    assert!(EntryInspectionSession::open(&path, None, &"0".repeat(64), &cancel).is_err());
    let mut session = EntryInspectionSession::open(&path, None, &info.identity, &cancel).unwrap();
    for item in &info.entries {
        if item.is_dir {
            continue;
        }
        let bytes = session.read(&item.id, 256 * 1024).unwrap();
        assert!(!bytes.truncated);
        assert_eq!(bytes.bytes, fs::read(t.path().join(&item.name)).unwrap());
    }
    assert!(session.read("99999", 8).is_err());
    cancel.store(true, Ordering::Relaxed);
    assert!(session.read("1", 8).is_err());
    drop(session);
    let id = info
        .entries
        .iter()
        .find(|e| e.name.ends_with("079.bin"))
        .unwrap()
        .id
        .clone();
    let p = plan(
        &path,
        &t.path().join("selected"),
        std::slice::from_ref(&id),
        ConflictPolicy::Skip,
        Limits::default(),
    )
    .unwrap();
    extract(&p, &flag(), &|_| {}).unwrap();
    let mut expected = shared.clone();
    expected[..4].copy_from_slice(&79u32.to_le_bytes());
    assert_eq!(
        fs::read(t.path().join("selected/related/079.bin")).unwrap(),
        expected
    );
    assert!(!t.path().join("selected/related/000.bin").exists());
    assert_eq!(
        inspect_entry_bytes(&path, &id, 31, None, &flag())
            .unwrap()
            .bytes,
        expected[..31]
    );
    test(&path, &flag(), &|_| {}).unwrap();
    let p = plan(
        &path,
        &t.path().join("all"),
        &[],
        ConflictPolicy::Skip,
        Limits::default(),
    )
    .unwrap();
    extract(&p, &flag(), &|_| {}).unwrap();
    for i in 0..80 {
        assert_eq!(
            fs::read(t.path().join(format!("all/related/{i:03}.bin"))).unwrap(),
            fs::read(source.join(format!("{i:03}.bin"))).unwrap()
        );
    }
    let again = t.path().join("deterministic.mrar");
    create(&again, &[source], &flag(), &|_| {}).unwrap();
    assert_eq!(fs::read(again).unwrap(), raw);
}

fn grouped_manifest() -> Value {
    json!({"version":1,"groups":[{"codec":"store","offset":0,"encoded_size":6,"decoded_size":6,"sha256":sha(b"abcdef")}],
        "entries":[{"path":"a.txt","kind":"file","size":3,"sha256":sha(b"abc"),"blocks":[],"group":{"id":0,"offset":0}},
        {"path":"b.txt","kind":"file","size":3,"sha256":sha(b"def"),"blocks":[],"group":{"id":0,"offset":3}}]})
}

#[test]
fn v1_rejects_hostile_references_and_corruption_even_with_a_valid_footer() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("bad.mrar");
    let mut cases = Vec::new();
    for (key, value) in [
        ("id", json!(1)),
        ("id", json!(u64::MAX)),
        ("offset", json!(1)),
        ("offset", json!(u64::MAX)),
    ] {
        let mut v = grouped_manifest();
        v["entries"][0]["group"][key] = value;
        cases.push(v);
    }
    for (key, value) in [
        ("decoded_size", json!(8 * 1024 * 1024 + 1)),
        ("offset", json!(1)),
        ("encoded_size", json!(7)),
    ] {
        let mut v = grouped_manifest();
        v["groups"][0][key] = value;
        cases.push(v);
    }
    let mut v = grouped_manifest();
    v["entries"][1]["group"]["offset"] = json!(0);
    cases.push(v);
    let mut v = grouped_manifest();
    v["entries"].as_array_mut().unwrap().pop();
    cases.push(v);
    let mut v = grouped_manifest();
    let duplicate = v["groups"][0].clone();
    v["groups"].as_array_mut().unwrap().push(duplicate);
    cases.push(v);
    let mut v = grouped_manifest();
    v["entries"][0]["blocks"] = base()["entries"][0]["blocks"].clone();
    cases.push(v);
    for v in cases {
        fs::write(&path, pack_v1(&v, b"abcdef")).unwrap();
        assert!(inspect(&path).is_err(), "{v}");
    }
    for group_hash in [true, false] {
        let mut v = grouped_manifest();
        if group_hash {
            v["groups"][0]["sha256"] = json!(sha(b"xxxxxx"));
        } else {
            v["entries"][0]["sha256"] = json!(sha(b"xxx"));
        }
        fs::write(&path, pack_v1(&v, b"abcdef")).unwrap();
        inspect(&path).unwrap();
        assert!(test(&path, &flag(), &|_| {}).is_err());
        let dest = t.path().join(if group_hash {
            "group-hash"
        } else {
            "file-hash"
        });
        let p = plan(&path, &dest, &[], ConflictPolicy::Skip, Limits::default()).unwrap();
        assert!(extract(&p, &flag(), &|_| {}).is_err());
        assert!(!dest.join("a.txt").exists());
    }
    let good = pack_v1(&grouped_manifest(), b"abcdef");
    for n in [0, 15, 19, good.len() - 1] {
        fs::write(&path, &good[..n]).unwrap();
        assert!(inspect(&path).is_err());
    }
    for decoded in [0u32, 8 * 1024 * 1024 + 1, 2] {
        let mut raw = good.clone();
        raw[16..20].copy_from_slice(&decoded.to_le_bytes());
        let end = raw.len() - 32;
        let footer = Sha256::digest(&raw[..end]);
        raw[end..].copy_from_slice(&footer);
        fs::write(&path, raw).unwrap();
        assert!(inspect(&path).is_err());
    }
    // /0 stays strict and old known-good archives still decode.
    write(&path, &base(), b"abc");
    test(&path, &flag(), &|_| {}).unwrap();
    let mut v = base();
    v["groups"] = json!([]);
    write(&path, &v, b"abc");
    assert!(inspect(&path).is_err());
}

#[test]
fn grouped_creation_cancellation_cleans_pending_data_and_empty_archives_work() {
    let t = tempfile::tempdir().unwrap();
    let source = t.path().join("tiny.txt");
    fs::write(&source, b"Pending grouped content").unwrap();
    let output = t.path().join("cancelled.mrar");
    let cancelled = flag();
    assert!(
        create(&output, std::slice::from_ref(&source), &cancelled, &|_| {
            cancelled.store(true, Ordering::Relaxed)
        })
        .is_err()
    );
    assert!(!output.exists());
    assert_eq!(fs::read_dir(t.path()).unwrap().count(), 1);
    fs::write(&source, []).unwrap();
    create(&output, &[source], &flag(), &|_| {}).unwrap();
    test(&output, &flag(), &|_| {}).unwrap();
    let info = inspect(&output).unwrap();
    assert_eq!(info.total_size, 0);
    assert_eq!(
        inspect_entry_bytes(&output, "0", 100, None, &flag())
            .unwrap()
            .bytes,
        b""
    );
}

fn varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}
/// One stored unit holding `data`, then the given entries as (dir, path, size).
fn v2_index(data: &[u8], entries: &[(bool, &str, u64)]) -> Vec<u8> {
    let mut out = Vec::new();
    varint(&mut out, 1);
    out.push(0);
    varint(&mut out, data.len() as u64);
    varint(&mut out, data.len() as u64);
    out.extend(Sha256::digest(data));
    varint(&mut out, entries.len() as u64);
    for (dir, path, size) in entries {
        out.push(u8::from(*dir));
        varint(&mut out, 0);
        varint(&mut out, path.len() as u64);
        out.extend(path.as_bytes());
        if !dir {
            varint(&mut out, *size);
        }
    }
    out
}
fn pack_v2(index: &[u8], data: &[u8]) -> Vec<u8> {
    let mut encoded = (index.len() as u32).to_le_bytes().to_vec();
    encoded.extend(zstd::bulk::compress(index, 3).unwrap());
    let mut raw = b"MRAR\r\n\x1a\n".to_vec();
    raw.extend(2u16.to_le_bytes());
    raw.extend(2u16.to_le_bytes());
    raw.extend((encoded.len() as u32).to_le_bytes());
    raw.extend(encoded);
    raw.extend(data);
    raw.extend(Sha256::digest(&raw));
    raw
}

#[test]
fn v2_index_is_strict_and_unit_hashes_guard_every_byte() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("v2.mrar");
    let data = b"abcdef";
    let good = v2_index(
        data,
        &[(true, "d", 0), (false, "d/a.txt", 2), (false, "d/b.txt", 4)],
    );
    fs::write(&path, pack_v2(&good, data)).unwrap();
    let info = inspect(&path).unwrap();
    assert_eq!(info.entries.len(), 3);
    assert_eq!(
        inspect_entry_bytes(&path, "2", 10, None, &flag())
            .unwrap()
            .bytes,
        b"cdef"
    );
    test(&path, &flag(), &|_| {}).unwrap();
    let dest = t.path().join("out");
    let p = plan(&path, &dest, &[], ConflictPolicy::Skip, Limits::default()).unwrap();
    extract(&p, &flag(), &|_| {}).unwrap();
    assert_eq!(fs::read(dest.join("d/b.txt")).unwrap(), b"cdef");

    let rejected = |index: Vec<u8>, data: &[u8], why: &str| {
        fs::write(&path, pack_v2(&index, data)).unwrap();
        assert!(inspect(&path).is_err(), "accepted: {why}");
    };
    // Unknown codec, oversized PPMd model and LZMA2 dictionary.
    let mut unknown = good.clone();
    unknown[1] = 9;
    rejected(unknown, data, "unknown codec");
    let mut ppmd = vec![1, 2, 6, 30];
    varint(&mut ppmd, 6);
    varint(&mut ppmd, 6);
    ppmd.extend(Sha256::digest(data));
    ppmd.extend(&good[1 + 1 + 1 + 1 + 32..]);
    rejected(ppmd, data, "PPMd model above 64 MiB");
    let mut lzma = vec![1, 1, 30];
    varint(&mut lzma, 6);
    varint(&mut lzma, 6);
    lzma.extend(Sha256::digest(data));
    lzma.extend(&good[1 + 1 + 1 + 1 + 32..]);
    rejected(lzma, data, "LZMA2 dictionary above 16 MiB");
    // Sizes that do not add up, store with mismatched sizes, trailing bytes.
    rejected(
        v2_index(data, &[(false, "a", 5)]),
        data,
        "files shorter than units",
    );
    rejected(
        v2_index(data, &[(false, "a", 7)]),
        data,
        "files longer than units",
    );
    rejected(
        v2_index(data, &[(false, "a", 6)]),
        b"abcdefg",
        "data longer than units",
    );
    let mut trailing = v2_index(data, &[(false, "a", 6)]);
    trailing.push(0);
    rejected(trailing, data, "trailing index bytes");
    let mut overlong = v2_index(data, &[(false, "a", 6)]);
    overlong[0] = 0x81;
    overlong.insert(1, 0x00);
    rejected(overlong, data, "non-minimal number");
    // Unsafe, non-canonical and duplicate paths.
    for bad in ["../a", "/a", "a//b", "a\\b", "C:/a", "a/./b"] {
        rejected(v2_index(data, &[(false, bad, 6)]), data, bad);
    }
    rejected(
        v2_index(data, &[(false, "a", 3), (false, "A", 3)]),
        data,
        "case duplicate",
    );

    // A changed payload byte with a recomputed footer still fails on the unit hash.
    let mut tampered = pack_v2(&good, data);
    let at = tampered.len() - 32 - 1;
    tampered[at] ^= 1;
    tampered.truncate(tampered.len() - 32);
    let footer = Sha256::digest(&tampered);
    tampered.extend(footer);
    fs::write(&path, tampered).unwrap();
    assert!(inspect(&path).is_ok());
    assert!(test(&path, &flag(), &|_| {}).is_err());
    assert!(inspect_entry_bytes(&path, "2", 10, None, &flag()).is_err());
}

#[test]
fn v2_real_archive_tampering_and_unit_boundaries() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("src");
    fs::create_dir(&src).unwrap();
    // 40 MiB of text crosses two unit boundaries; a file sits across each one.
    let line = b"2026-10-31,record,user42,upload,eu-central,1234,ok\n";
    let text: Vec<u8> = line.iter().copied().cycle().take(15 << 20).collect();
    for name in ["a.csv", "b.csv"] {
        fs::write(src.join(name), &text).unwrap();
    }
    fs::write(src.join("c.bin"), random(10 << 20)).unwrap();
    let path = t.path().join("big.mrar");
    let info = create(&path, std::slice::from_ref(&src), &flag(), &|_| {}).unwrap();
    assert!(fs::metadata(&path).unwrap().len() < (11 << 20));
    let b = info
        .entries
        .iter()
        .find(|e| e.name.ends_with("b.csv"))
        .unwrap();
    let bytes = inspect_entry_bytes(&path, &b.id, 4 << 20, None, &flag()).unwrap();
    assert_eq!(bytes.bytes, text[..4 << 20]);
    let dest = t.path().join("out");
    let p = plan(
        &path,
        &dest,
        std::slice::from_ref(&b.id),
        ConflictPolicy::Skip,
        Limits::default(),
    )
    .unwrap();
    extract(&p, &flag(), &|_| {}).unwrap();
    assert_eq!(fs::read(dest.join("src/b.csv")).unwrap(), text);
    let mut raw = fs::read(&path).unwrap();
    let middle = raw.len() / 2;
    raw[middle] ^= 0x40;
    raw.truncate(raw.len() - 32);
    let footer = Sha256::digest(&raw);
    raw.extend(footer);
    let bad = t.path().join("bad.mrar");
    fs::write(&bad, raw).unwrap();
    assert!(test(&bad, &flag(), &|_| {}).is_err());
}
