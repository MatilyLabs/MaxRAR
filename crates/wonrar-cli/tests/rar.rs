//! End-to-end RAR tests exercise the installed-style same-executable decoder worker.
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../wonrar-core/tests/fixtures/rar")
        .join(name)
}
fn run(args: &[&str], password: Option<&str>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_maxrar-cli"));
    cmd.args(args)
        .arg("--json")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if password.is_some() {
        cmd.arg("--password-stdin").stdin(Stdio::piped());
    }
    let mut child = cmd.spawn().unwrap();
    if let Some(password) = password {
        writeln!(child.stdin.take().unwrap(), "{password}").unwrap();
    }
    child.wait_with_output().unwrap()
}
fn value(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{} / {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn rar4_list_preview_test_extract_conflicts_and_no_creation() {
    let archive = fixture("version.rar");
    let path = archive.to_str().unwrap();
    let info = value(run(&["inspect", path], None));
    assert_eq!(info["format"], "rar");
    assert_eq!(info["capabilities"]["can_create"], false);
    assert_eq!(info["capabilities"]["can_extract"], true);
    assert_eq!(info["capabilities"]["can_delete"], false);
    assert_eq!(
        value(run(&["preview", path, "--entry", "0"], None))["text"],
        "unrar-0.4.0"
    );
    let prefix = value(run(
        &["preview", path, "--entry", "0", "--max-bytes", "4"],
        None,
    ));
    assert_eq!(prefix["text"], "unra");
    assert_eq!(prefix["truncated"], true);
    assert_eq!(value(run(&["test", path], None))["bytes_processed"], 11);
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("out");
    let out = dest.to_str().unwrap();
    let extracted = value(run(&["extract", path, "--destination", out], None));
    assert_eq!(extracted["written"][0], "VERSION");
    assert_eq!(fs::read(dest.join("VERSION")).unwrap(), b"unrar-0.4.0");
    fs::write(dest.join("VERSION"), b"keep me").unwrap();
    value(run(&["extract", path, "--destination", out], None));
    assert_eq!(fs::read(dest.join("VERSION")).unwrap(), b"keep me");
    let both = value(run(
        &[
            "extract",
            path,
            "--destination",
            out,
            "--policy",
            "keep-both",
        ],
        None,
    ));
    assert_eq!(both["written"].as_array().unwrap().len(), 1);
    assert_ne!(both["written"][0], "VERSION");
    value(run(
        &["extract", path, "--destination", out, "--policy", "replace"],
        None,
    ));
    assert_eq!(fs::read(dest.join("VERSION")).unwrap(), b"unrar-0.4.0");
    let target = dir.path().join("cannot-create.rar");
    assert!(!run(
        &[
            "create",
            "--output",
            target.to_str().unwrap(),
            dest.join("VERSION").to_str().unwrap()
        ],
        None
    )
    .status
    .success());
    assert!(!target.exists());
}

#[test]
fn rar5_compressed_solid_selected_extraction_and_scan() {
    for name in [
        "solid.rar",
        "test_read_format_rar5_compressed.rar",
        "test_read_format_rar5_multiple_files_solid.rar",
    ] {
        let archive = fixture(name);
        let path = archive.to_str().unwrap();
        let info = value(run(&["inspect", path], None));
        let receipt = value(run(&["test", path], None));
        assert_eq!(
            receipt["written"].as_array().unwrap().len(),
            info["entries"].as_array().unwrap().len()
        );
        let entries = info["entries"].as_array().unwrap();
        let file = entries.iter().rev().find(|e| e["is_dir"] == false).unwrap();
        let id = file["id"].as_str().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let extracted = value(run(
            &[
                "extract",
                path,
                "--entry",
                id,
                "--destination",
                temp.path().to_str().unwrap(),
            ],
            None,
        ));
        assert_eq!(extracted["written"].as_array().unwrap().len(), 1);
        let output = temp.path().join(extracted["written"][0].as_str().unwrap());
        assert_eq!(
            fs::metadata(&output).unwrap().len(),
            file["size"].as_u64().unwrap()
        );
        if name == "test_read_format_rar5_multiple_files_solid.rar" {
            // Independently regenerate libarchive's documented fourth-file fixture pattern.
            let expected: Vec<u8> = (1i32..=1024)
                .flat_map(|k| (k * k - 3 * k + 5).max(0).to_le_bytes())
                .collect();
            assert_eq!(fs::read(&output).unwrap(), expected);
        }
        let scan = value(run(&["scan", path], None));
        assert!(scan["coverage"]["scanned_files"].as_u64().unwrap_or(0) > 0);
    }
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("nested.zip");
    let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
    zip.start_file("inside.rar", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&fs::read(fixture("version.rar")).unwrap())
        .unwrap();
    zip.finish().unwrap();
    let scan = value(run(&["scan", archive.to_str().unwrap()], None));
    assert_eq!(scan["coverage"]["nested_archives"], 1);
    assert_eq!(scan["coverage"]["opaque_containers"], 0);
    assert_eq!(scan["coverage"]["complete"], true);
}

#[test]
fn rar_encrypted_payload_and_headers_require_the_correct_password() {
    for (name, password, hidden) in [
        ("crypted.rar", "unrar", false),
        ("comment-hpw-password.rar", "password", true),
    ] {
        let archive = fixture(name);
        let path = archive.to_str().unwrap();
        let absent = run(&["inspect", path], None);
        assert_eq!(absent.status.success(), !hidden);
        if !hidden {
            assert_eq!(
                serde_json::from_slice::<Value>(&absent.stdout).unwrap()["entries"][0]
                    ["is_encrypted"],
                true
            );
        }
        assert!(!run(&["preview", path, "--entry", "0"], None)
            .status
            .success());
        assert!(!run(&["test", path], Some("not-the-password"))
            .status
            .success());
        assert_eq!(
            value(run(&["preview", path, "--entry", "0"], Some(password)))["text"],
            "target\nCargo.lock\n"
        );
        value(run(&["test", path], Some(password)));
        let dest = tempfile::tempdir().unwrap();
        let failed = run(
            &[
                "extract",
                path,
                "--destination",
                dest.path().to_str().unwrap(),
            ],
            Some("not-the-password"),
        );
        assert!(!failed.status.success());
        assert_eq!(fs::read_dir(dest.path()).unwrap().count(), 0);
        value(run(
            &[
                "extract",
                path,
                "--destination",
                dest.path().to_str().unwrap(),
            ],
            Some(password),
        ));
        assert_eq!(
            fs::read(dest.path().join(".gitignore")).unwrap(),
            b"target\nCargo.lock\n"
        );
    }
}

#[test]
fn rar_unicode_and_content_detection_ignore_the_extension() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("archive café.bin");
    fs::copy(fixture("unicode-entry.rar"), &path).unwrap();
    let info = value(run(&["inspect", path.to_str().unwrap()], None));
    assert_eq!(info["entries"][0]["name"], "unicodefilename❤️.txt");
    value(run(
        &[
            "extract",
            path.to_str().unwrap(),
            "--destination",
            dir.path().join("out").to_str().unwrap(),
        ],
        None,
    ));
    assert_eq!(
        fs::metadata(dir.path().join("out/unicodefilename❤️.txt"))
            .unwrap()
            .len(),
        7
    );
}

#[test]
fn rar_multivolume_and_link_entries_fail_closed() {
    let archive = fixture("archive.part1.rar");
    let output = run(&["inspect", archive.to_str().unwrap()], None);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Multipart"));
    let archive = fixture("test_read_format_rar5_symlink.rar");
    let path = archive.to_str().unwrap();
    let info = value(run(&["inspect", path], None));
    assert!(info["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["code"] == "special_entry"));
    let dir = tempfile::tempdir().unwrap();
    let plan = value(run(
        &[
            "plan",
            path,
            "--destination",
            dir.path().join("out").to_str().unwrap(),
        ],
        None,
    ));
    for finding in info["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["code"] == "special_entry")
    {
        let entry = plan["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["entry_id"] == finding["entry_id"])
            .unwrap();
        assert_ne!(entry["action"], "extract");
        assert!(!run(
            &[
                "preview",
                path,
                "--entry",
                finding["entry_id"].as_str().unwrap()
            ],
            None
        )
        .status
        .success());
    }
    assert!(!run(&["test", path], None).status.success());
    let out = dir.path().join("extracted");
    value(run(
        &["extract", path, "--destination", out.to_str().unwrap()],
        None,
    ));
    assert!(out.join("file.txt").is_file());
    assert!(out.join("dir").is_dir());
    assert!(!out.join("symlink.txt").exists());
    assert!(!out.join("dirlink").exists());
}

#[test]
fn rar_corrupt_payload_is_never_committed() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("corrupt.rar");
    let mut bytes = fs::read(fixture("version.rar")).unwrap();
    bytes[65] ^= 0xff; // compressed payload, preserving the valid inventory header
    fs::write(&archive, bytes).unwrap();
    value(run(&["inspect", archive.to_str().unwrap()], None));
    assert!(!run(&["test", archive.to_str().unwrap()], None)
        .status
        .success());
    let dest = dir.path().join("out");
    assert!(!run(
        &[
            "extract",
            archive.to_str().unwrap(),
            "--destination",
            dest.to_str().unwrap()
        ],
        None
    )
    .status
    .success());
    assert!(!dest.join("VERSION").exists());
    assert_eq!(fs::read_dir(dest).unwrap().count(), 0);
}

#[test]
fn rar_unsafe_path_is_blocked_and_stale_plan_cannot_write() {
    // Change an existing RAR4 filename, preserving its length and header CRC.
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for byte in bytes {
            crc ^= *byte as u32;
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb88320 & (0u32.wrapping_sub(crc & 1)));
            }
        }
        !crc
    }
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("paths.rar");
    let mut bytes = fs::read(fixture("version.rar")).unwrap();
    bytes[52..59].copy_from_slice(b"../evil");
    let crc = crc32(&bytes[22..59]) as u16;
    bytes[20..22].copy_from_slice(&crc.to_le_bytes());
    fs::write(&archive, bytes).unwrap();
    let path = archive.to_str().unwrap();
    let info = value(run(&["inspect", path], None));
    assert!(info["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["code"] == "unsafe_path"));
    let dest = dir.path().join("out");
    let out = dest.to_str().unwrap();
    let plan = value(run(&["plan", path, "--destination", out], None));
    assert_ne!(plan["entries"][0]["action"], "extract");
    value(run(&["extract", path, "--destination", out], None));
    assert!(!dir.path().join("evil").exists());

    fs::copy(fixture("version.rar"), &archive).unwrap();
    let plan = run(&["plan", path, "--destination", out], None);
    assert!(plan.status.success());
    let planfile = dir.path().join("plan.json");
    fs::write(&planfile, plan.stdout).unwrap();
    fs::copy(fixture("solid.rar"), &archive).unwrap();
    let rejected = run(&["extract-plan", planfile.to_str().unwrap()], None);
    assert!(!rejected.status.success());
    assert!(!dest.join("VERSION").exists());
}
