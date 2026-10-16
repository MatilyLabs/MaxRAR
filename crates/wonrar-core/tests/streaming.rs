use std::{fs, io::Write, path::Path, sync::atomic::AtomicBool};
use wonrar_core::*;

fn flag() -> AtomicBool {
    AtomicBool::new(false)
}
fn tar_header(name: &str, size: u64, kind: u8) -> [u8; 512] {
    let mut h = tar::Header::new_gnu();
    h.set_size(size);
    h.set_mode(0o644);
    h.set_entry_type(tar::EntryType::new(kind));
    // Deliberately bypass Builder's path validation to create hostile fixtures.
    h.as_mut_bytes()[..100].fill(0);
    h.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
    h.set_cksum();
    *h.as_bytes()
}
fn raw_tar(entries: &[(&str, &[u8], u8)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, data, kind) in entries {
        out.extend_from_slice(&tar_header(name, data.len() as u64, *kind));
        out.extend_from_slice(data);
        out.resize(out.len() + (512 - data.len() % 512) % 512, 0);
    }
    out.extend_from_slice(&[0; 1024]);
    out
}
fn gzip(path: &Path, contents: &[u8]) {
    let mut w = flate2::write::GzEncoder::new(
        fs::File::create(path).unwrap(),
        flate2::Compression::default(),
    );
    w.write_all(contents).unwrap();
    w.finish().unwrap();
}

#[test]
fn tar_and_all_compound_filters_roundtrip_selected_bytes() {
    for (suffix, format) in [
        ("tar", "tar"),
        ("tar.gz", "tar.gz"),
        ("tgz", "tar.gz"),
        ("tar.bz2", "tar.bz2"),
        ("tbz2", "tar.bz2"),
        ("tar.xz", "tar.xz"),
        ("txz", "tar.xz"),
        ("tar.zst", "tar.zst"),
        ("tzst", "tar.zst"),
    ] {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("source");
        fs::create_dir(&src).unwrap();
        fs::write(
            src.join("paper.tex"),
            b"\\documentclass{article}\n\\begin{document}Test\\end{document}",
        )
        .unwrap();
        fs::write(src.join("binary.bin"), [0, 255, 128, 65]).unwrap();
        let path = t.path().join(format!("a.{suffix}"));
        let info = create(&path, &[src], &flag(), &|_| {}).unwrap();
        assert_eq!(info.format, format);
        assert!(!info.capabilities.can_delete);
        assert!(!info.capabilities.can_comment);
        let binary = info
            .entries
            .iter()
            .find(|e| e.name.ends_with("binary.bin"))
            .unwrap();
        let bytes = inspect_entry_bytes(&path, &binary.id, 100, None, &flag()).unwrap();
        assert_eq!(bytes.bytes, [0, 255, 128, 65]);
        assert!(!bytes.truncated);
        assert!(preview(&path, &binary.id, 100).is_err());
        let tex = info
            .entries
            .iter()
            .find(|e| e.name.ends_with("paper.tex"))
            .unwrap();
        assert!(preview(&path, &tex.id, 1000)
            .unwrap()
            .text
            .contains("documentclass"));
        let dest = t.path().join("out");
        let p = plan(
            &path,
            &dest,
            std::slice::from_ref(&tex.id),
            ConflictPolicy::Skip,
            Limits::default(),
        )
        .unwrap();
        let r = extract(&p, &flag(), &|_| {}).unwrap();
        assert_eq!(r.written.len(), 1);
        assert!(dest.join("source/paper.tex").exists());
        assert!(!dest.join("source/binary.bin").exists());
        test(&path, &flag(), &|_| {}).unwrap();
    }
}

#[test]
fn single_file_filters_and_extensionless_arxiv_payloads() {
    for (suffix, format) in [
        ("gz", "gzip"),
        ("bz2", "bzip2"),
        ("xz", "xz"),
        ("zst", "zstd"),
    ] {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("source.tex");
        fs::write(&src, b"\\documentclass{article}\nResearch source").unwrap();
        let path = t.path().join(format!("paper.tex.{suffix}"));
        let info = create(&path, &[src], &flag(), &|_| {}).unwrap();
        assert_eq!(info.format, format);
        assert_eq!(info.entries.len(), 1);
        assert_eq!(info.entries[0].name, "paper.tex");
        let bytes = inspect_entry_bytes(&path, "0", 4, None, &flag()).unwrap();
        assert_eq!(bytes.bytes, b"\\doc");
        assert!(bytes.truncated);
        let dest = t.path().join("out");
        let p = plan(&path, &dest, &[], ConflictPolicy::Skip, Limits::default()).unwrap();
        extract(&p, &flag(), &|_| {}).unwrap();
        assert!(dest.join("paper.tex").exists());
        test(&path, &flag(), &|_| {}).unwrap();
        let extensionless = t.path().join("2401.12345");
        fs::rename(path, &extensionless).unwrap();
        let info = inspect(&extensionless).unwrap();
        assert_eq!(info.format, format);
        assert!(preview(&extensionless, "0", 100)
            .unwrap()
            .text
            .contains("documentclass"));
    }
    let t = tempfile::tempdir().unwrap();
    let extensionless = t.path().join("2401.12345");
    gzip(&extensionless, &raw_tar(&[("./main.tex", b"source", b'0')]));
    let info = inspect(&extensionless).unwrap();
    assert_eq!(info.format, "tar.gz");
    assert_eq!(info.entries[0].name, "./main.tex");
    let dest = t.path().join("out");
    let p = plan(
        &extensionless,
        &dest,
        &[],
        ConflictPolicy::Skip,
        Limits::default(),
    )
    .unwrap();
    assert_eq!(p.entries[0].output_name, "main.tex");
    extract(&p, &flag(), &|_| {}).unwrap();
    assert_eq!(fs::read(dest.join("main.tex")).unwrap(), b"source");
}

#[test]
fn tar_traversal_links_and_devices_are_blocked_before_writes() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("wrong-suffix.zip");
    gzip(
        &path,
        &raw_tar(&[
            ("../escape", b"bad", b'0'),
            ("/absolute", b"bad", b'0'),
            ("symlink", b"../outside", b'2'),
            ("hardlink", b"target", b'1'),
            ("device", b"", b'3'),
            ("good.txt", b"okay", b'0'),
        ]),
    );
    let info = inspect(&path).unwrap();
    assert_eq!(info.format, "tar.gz");
    let dest = t.path().join("out");
    let p = plan(&path, &dest, &[], ConflictPolicy::Skip, Limits::default()).unwrap();
    assert_eq!(
        p.entries.iter().filter(|e| e.action == "blocked").count(),
        5
    );
    let r = extract(&p, &flag(), &|_| {}).unwrap();
    assert_eq!(r.written, ["good.txt"]);
    assert!(!t.path().join("escape").exists());
}

#[test]
fn corrupted_compression_and_truncated_tar_are_rejected() {
    for suffix in ["gz", "bz2", "xz", "zst"] {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("source.txt");
        fs::write(&src, b"normal payload normal payload").unwrap();
        let path = t.path().join(format!("a.{suffix}"));
        create(&path, &[src], &flag(), &|_| {}).unwrap();
        let mut bytes = fs::read(&path).unwrap();
        let len = bytes.len();
        // Bzip2's final byte may contain ignored bit padding, so corrupt the
        // first block's stored CRC instead of padding bits.
        let corrupt = if suffix == "bz2" { 10 } else { len - 1 };
        bytes[corrupt] ^= 0x55;
        fs::write(&path, bytes).unwrap();
        assert!(inspect(&path).is_err(), "corrupt {suffix}");
        assert!(test(&path, &flag(), &|_| {}).is_err());
    }
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("a.tar");
    let mut bytes = raw_tar(&[("a.txt", b"payload", b'0')]);
    bytes.truncate(514);
    fs::write(&path, bytes).unwrap();
    assert!(inspect(&path).is_err());
}

#[test]
fn decoder_limits_include_skipped_data_and_ignore_gzip_size_trailer() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("a.tgz");
    gzip(
        &path,
        &raw_tar(&[("big", &[b'x'; 65536], b'0'), ("small", b"tiny", b'0')]),
    );
    let info = inspect(&path).unwrap();
    let small = info.entries.iter().find(|e| e.name == "small").unwrap();
    let limits = Limits {
        max_file_bytes: 10,
        max_total_bytes: 20,
        ..Limits::default()
    };
    assert!(plan(
        &path,
        &t.path().join("out"),
        std::slice::from_ref(&small.id),
        ConflictPolicy::Skip,
        limits
    )
    .is_err());
    assert!(!t.path().join("out").exists());
    let path = t.path().join("single.gz");
    gzip(&path, &[b'x'; 65536]);
    let info = inspect(&path).unwrap();
    assert_eq!(info.entries[0].size, 65536);
    let raw = inspect_entry_bytes(&path, "0", 8, None, &flag()).unwrap();
    assert!(raw.truncated);
    assert_eq!(raw.bytes.len(), 8);
    let mut bytes = fs::read(&path).unwrap();
    let len = bytes.len();
    bytes[len - 4..].copy_from_slice(&1u32.to_le_bytes());
    fs::write(&path, bytes).unwrap();
    assert!(inspect(&path).is_err());
}

#[test]
fn tar_extension_and_zero_padding_bombs_are_bounded() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("metadata.gz");
    let mut payload = tar_header("pax", 1024 * 1024 + 1, b'x').to_vec();
    payload.extend_from_slice(&[0; 1024]);
    gzip(&path, &payload);
    assert!(inspect(&path)
        .unwrap_err()
        .to_string()
        .contains("metadata limit"));
    let path = t.path().join("zeros.gz");
    gzip(&path, &vec![0; 2 * 1024 * 1024]);
    assert!(inspect(&path).unwrap_err().to_string().contains("padding"));
    let path = t.path().join("sparse.tar");
    fs::write(&path, raw_tar(&[("sparse", b"", b'S')])).unwrap();
    assert!(inspect(&path).unwrap_err().to_string().contains("sparse"));
}

#[test]
fn stream_creation_preserves_existing_files_and_rejects_unsupported_options() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("source");
    fs::create_dir(&src).unwrap();
    fs::write(src.join("a"), b"input").unwrap();
    assert!(create(
        &t.path().join("a.gz"),
        std::slice::from_ref(&src),
        &flag(),
        &|_| {}
    )
    .is_err());
    for suffix in ["tar", "tar.gz", "tar.bz2", "tar.xz", "tar.zst"] {
        let path = t.path().join(format!("a.{suffix}"));
        let first = create(&path, std::slice::from_ref(&src), &flag(), &|_| {}).unwrap();
        let original = fs::read(&path).unwrap();
        assert!(create(&path, std::slice::from_ref(&src), &flag(), &|_| {}).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        assert!(set_comment(&path, "unsupported", &flag()).is_err());
        assert!(delete_entries(&path, &[first.entries[0].id.clone()], &flag()).is_err());
        assert!(create_with_password(
            &t.path().join(format!("secret.{suffix}")),
            std::slice::from_ref(&src),
            &flag(),
            &|_| {},
            Some("password")
        )
        .is_err());
    }
}

#[test]
fn raw_entry_api_preserves_binary_and_honors_cancellation() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("source.bin");
    fs::write(&src, [0, 255, 128, 65, 0, 66]).unwrap();
    for suffix in ["zip", "7z", "tar", "gz"] {
        let path = t.path().join(format!("a.{suffix}"));
        create(&path, std::slice::from_ref(&src), &flag(), &|_| {}).unwrap();
        let bytes = inspect_entry_bytes(&path, "0", 100, None, &flag()).unwrap();
        assert_eq!(bytes.bytes, [0, 255, 128, 65, 0, 66]);
        assert!(!bytes.truncated);
        assert!(matches!(
            inspect_entry_bytes(&path, "0", 100, None, &AtomicBool::new(true)),
            Err(Error::Cancelled)
        ));
    }
}

#[test]
fn python_generated_pax_gnu_and_single_source_interoperate() {
    for bytes in [
        include_bytes!("fixtures/python-pax.tar.gz").as_slice(),
        include_bytes!("fixtures/python-gnu.tar.gz").as_slice(),
    ] {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("2401.12345");
        fs::write(&path, bytes).unwrap();
        let info = inspect(&path).unwrap();
        assert_eq!(info.format, "tar.gz");
        assert_eq!(info.entries.len(), 2);
        let tex = info
            .entries
            .iter()
            .find(|e| e.name.ends_with("main.tex"))
            .unwrap();
        assert!(tex.name.len() > 100);
        assert!(preview(&path, &tex.id, 1000)
            .unwrap()
            .text
            .contains("Python fixture"));
        let p = plan(
            &path,
            &t.path().join("out"),
            &[],
            ConflictPolicy::Skip,
            Limits::default(),
        )
        .unwrap();
        assert_eq!(
            p.entries.iter().filter(|e| e.action == "blocked").count(),
            1
        );
        let r = extract(&p, &flag(), &|_| {}).unwrap();
        assert_eq!(r.written.len(), 1);
        assert_eq!(r.skipped, ["./shortcut"]);
    }
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("2401.12345");
    fs::write(&path, include_bytes!("fixtures/python-single-source.gz")).unwrap();
    let info = inspect(&path).unwrap();
    assert_eq!(info.format, "gzip");
    assert!(preview(&path, "0", 1000)
        .unwrap()
        .text
        .contains("Single arXiv-like source"));
}

#[test]
fn zip_container_aliases_are_detected_without_loading_application_content() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("content.dat");
    fs::write(&src, [0, 255, 1, 2]).unwrap();
    let base = t.path().join("base.zip");
    create(&base, &[src], &flag(), &|_| {}).unwrap();
    for suffix in [
        "jar",
        "odt",
        "ods",
        "odp",
        "whl",
        "npz",
        "pt",
        "pth",
        "no-suffix",
    ] {
        let path = t.path().join(format!("artifact.{suffix}"));
        fs::copy(&base, &path).unwrap();
        let info = inspect(&path).unwrap();
        assert_eq!(info.format, "zip");
        assert_eq!(
            inspect_entry_bytes(&path, "0", 16, None, &flag())
                .unwrap()
                .bytes,
            [0, 255, 1, 2]
        );
    }
}

#[test]
fn pax_path_and_size_override_raw_header_without_parser_differentials() {
    fn record(key: &str, value: &str) -> Vec<u8> {
        let body = format!("{key}={value}\n");
        let mut len = body.len() + 2;
        loop {
            let candidate = format!("{len} {body}");
            if candidate.len() == len {
                return candidate.into_bytes();
            }
            len = candidate.len();
        }
    }
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("pax.tar");
    let mut pax = record("path", "pax/override.txt");
    pax.extend(record("size", "3"));
    let mut bytes = tar_header("PaxHeader", pax.len() as u64, b'x').to_vec();
    bytes.extend(&pax);
    bytes.resize(bytes.len() + (512 - pax.len() % 512) % 512, 0);
    let mut header = tar::Header::new_gnu();
    header
        .as_mut_bytes()
        .copy_from_slice(&tar_header("raw-name", 0, b'0'));
    header.as_mut_bytes()[0] = 0xff;
    header.set_cksum();
    bytes.extend(header.as_bytes());
    bytes.extend(b"abc");
    bytes.resize(bytes.len() + 509, 0);
    bytes.extend(&[0; 1024]);
    fs::write(&path, &bytes).unwrap();
    let info = inspect(&path).unwrap();
    assert_eq!(info.entries[0].name, "pax/override.txt");
    assert_eq!(info.entries[0].size, 3);
    assert_eq!(preview(&path, "0", 100).unwrap().text, "abc");
    let dest = t.path().join("out");
    let p = plan(&path, &dest, &[], ConflictPolicy::Skip, Limits::default()).unwrap();
    extract(&p, &flag(), &|_| {}).unwrap();
    assert_eq!(fs::read(dest.join("pax/override.txt")).unwrap(), b"abc");
}
