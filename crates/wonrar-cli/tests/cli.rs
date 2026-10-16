use serde_json::Value;
use std::{
    fs,
    process::{Command, Output},
};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_maxrar-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn cli_round_trip_and_reviewed_plan() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("notes.txt");
    let archive = dir.path().join("bundle.zip");
    let output = dir.path().join("unpacked");
    let plan_file = dir.path().join("plan.json");
    fs::write(&input, "WonRaR CLI integration fixture\n").unwrap();
    let created = run(&[
        "create",
        "--output",
        archive.to_str().unwrap(),
        input.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stdout)
    );
    let identity = json(&created)["identity"].as_str().unwrap().to_owned();
    let listed = run(&["list", archive.to_str().unwrap(), "--json"]);
    assert!(listed.status.success());
    assert_eq!(json(&listed)["archive_identity"], identity);
    let preview = run(&[
        "preview",
        archive.to_str().unwrap(),
        "--entry",
        "0",
        "--json",
    ]);
    assert!(preview.status.success());
    assert_eq!(json(&preview)["text"], "WonRaR CLI integration fixture\n");
    let asked = run(&[
        "ask",
        archive.to_str().unwrap(),
        "integration fixture",
        "--json",
    ]);
    assert!(
        asked.status.success(),
        "{}",
        String::from_utf8_lossy(&asked.stdout)
    );
    assert_eq!(json(&asked)["mode"], "evidence_search");
    assert!(!json(&asked)["evidence"].as_array().unwrap().is_empty());
    let planned = run(&[
        "plan",
        archive.to_str().unwrap(),
        "--destination",
        output.to_str().unwrap(),
        "--json",
    ]);
    assert!(planned.status.success());
    fs::write(&plan_file, &planned.stdout).unwrap();
    let extracted = run(&[
        "extract-plan",
        plan_file.to_str().unwrap(),
        "--json",
        "--progress",
    ]);
    assert!(
        extracted.status.success(),
        "{}",
        String::from_utf8_lossy(&extracted.stdout)
    );
    assert_eq!(
        fs::read(output.join("notes.txt")).unwrap(),
        fs::read(input).unwrap()
    );
    assert_eq!(json(&extracted)["operation"], "extract");
    assert!(String::from_utf8_lossy(&extracted.stderr).contains("progress"));
    let tested = run(&["test", archive.to_str().unwrap(), "--json"]);
    assert!(tested.status.success());
}

#[test]
fn cli_errors_are_machine_readable_and_nonzero() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing.zip");
    let result = run(&["inspect", missing.to_str().unwrap(), "--json"]);
    assert!(!result.status.success());
    assert!(json(&result)["error"].is_string());
    assert_eq!(json(&result)["cancelled"], false);
}

#[test]
fn passwords_arrive_on_stdin_and_never_appear_in_json() {
    use std::{io::Write, process::Stdio};
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("private.txt");
    let archive = dir.path().join("private.zip");
    fs::write(&input, "confidential fixture").unwrap();
    let with_password = |args: &[&str]| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_maxrar-cli"))
            .args(args)
            .arg("--password-stdin")
            .arg("--json")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"fixture-password-947\n")
            .unwrap();
        child.wait_with_output().unwrap()
    };
    let created = with_password(&[
        "create",
        "--output",
        archive.to_str().unwrap(),
        input.to_str().unwrap(),
    ]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stdout)
    );
    assert!(!String::from_utf8_lossy(&created.stdout).contains("fixture-password-947"));
    let absent = run(&[
        "preview",
        archive.to_str().unwrap(),
        "--entry",
        "0",
        "--json",
    ]);
    assert!(!absent.status.success());
    let read = with_password(&["preview", archive.to_str().unwrap(), "--entry", "0"]);
    assert!(read.status.success());
    assert_eq!(json(&read)["text"], "confidential fixture");
    let scanned = with_password(&["scan", archive.to_str().unwrap()]);
    assert!(
        scanned.status.success(),
        "{}",
        String::from_utf8_lossy(&scanned.stdout)
    );
    assert_eq!(json(&scanned)["coverage"]["complete"], true);
    assert!(!String::from_utf8_lossy(&scanned.stdout).contains("fixture-password-947"));
}

#[test]
fn builtin_scan_uses_same_binary_worker_and_scans_decoded_content() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("notes café.txt");
    let archive = dir.path().join("archive café.zip");
    fs::write(&input, b"WONRAR-BENIGN-SCAN-TEST-2026").unwrap();
    wonrar_core::create(
        &archive,
        &[input],
        &std::sync::atomic::AtomicBool::new(false),
        &|_| {},
    )
    .unwrap();
    let report = run(&["scan", archive.to_str().unwrap(), "--json", "--progress"]);
    assert!(
        report.status.success(),
        "{}",
        String::from_utf8_lossy(&report.stdout)
    );
    assert_eq!(json(&report)["engine"], wonrar_scan::ENGINE);
    assert_eq!(json(&report)["status"], "findings");
    assert!(json(&report)["rule_findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["code"] == "WonRaR_Test_Sentinel"));
    assert_eq!(json(&report)["coverage"]["complete"], true);
    assert!(json(&report)["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r.as_str().is_some_and(|s| s.contains("worker"))));
}

#[cfg(unix)]
#[test]
fn builtin_scan_cancels_live_worker_after_progress() {
    use std::{
        io::{BufRead, BufReader, Write},
        process::Stdio,
        time::{Duration, Instant},
    };
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("many.zip");
    let mut writer = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
    let bytes = vec![b'a'; 8192];
    for i in 0..4000 {
        writer
            .start_file(
                format!("file{i}.txt"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_maxrar-cli"))
        .args(["scan", archive.to_str().unwrap(), "--json", "--progress"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut first = String::new();
    stderr.read_line(&mut first).unwrap();
    assert!(
        first.contains("progress"),
        "worker produced no progress: {first}"
    );
    // SAFETY: sends SIGINT to the child process owned by this test; no pointer arguments.
    assert_eq!(
        unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) },
        0
    );
    let started = Instant::now();
    while child.try_wait().unwrap().is_none() {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "cancellation failed to reap worker promptly"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let result = child.wait_with_output().unwrap();
    assert_eq!(result.status.code(), Some(130));
    assert_eq!(json(&result)["cancelled"], true);
    assert!(json(&result)["error"]
        .as_str()
        .unwrap()
        .contains("cancelled"));
}

#[test]
fn expected_identity_rejects_stale_entry_ids_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    let archive = dir.path().join("bundle.zip");
    let replacement = dir.path().join("replacement.zip");
    fs::write(&a, b"first").unwrap();
    fs::write(&b, b"second").unwrap();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let initial = wonrar_core::create(&archive, &[a.clone(), b.clone()], &cancel, &|_| {}).unwrap();
    let repacked = wonrar_core::create(&replacement, &[b, a], &cancel, &|_| {}).unwrap();
    fs::copy(&replacement, &archive).unwrap();
    let expected_bytes = fs::read(&archive).unwrap();
    let rejected = run(&[
        "delete",
        archive.to_str().unwrap(),
        "--entry",
        "0",
        "--yes",
        "--expect-identity",
        &initial.identity,
        "--json",
    ]);
    assert!(!rejected.status.success());
    assert!(json(&rejected)["error"]
        .as_str()
        .unwrap()
        .contains("changed"));
    assert_eq!(fs::read(&archive).unwrap(), expected_bytes);
    let destination = dir.path().join("unpacked");
    let rejected = run(&[
        "extract",
        archive.to_str().unwrap(),
        "--destination",
        destination.to_str().unwrap(),
        "--entry",
        "0",
        "--expect-identity",
        &initial.identity,
        "--json",
    ]);
    assert!(!rejected.status.success());
    assert!(!destination.exists());
    let accepted = run(&[
        "delete",
        archive.to_str().unwrap(),
        "--entry",
        "0",
        "--yes",
        "--expect-identity",
        &repacked.identity,
        "--json",
    ]);
    assert!(accepted.status.success());
    assert_eq!(json(&accepted)["entries"][0]["name"], "a.txt");
}

#[test]
fn experimental_mrar_cli_create_list_test_plan_extract_and_scan_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("marker.txt");
    let binary = dir.path().join("binary.bin");
    let archive = dir.path().join("bundle.mrar");
    let output = dir.path().join("unpacked");
    fs::write(&first, b"WONRAR-BENIGN-SCAN-TEST-2026").unwrap();
    let binary_bytes: Vec<u8> = (0..=255).cycle().take(65537).collect();
    fs::write(&binary, &binary_bytes).unwrap();
    let created = run(&[
        "create",
        "--output",
        archive.to_str().unwrap(),
        first.to_str().unwrap(),
        binary.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stdout)
    );
    assert_eq!(json(&created)["format"], "mrar");
    assert_eq!(json(&created)["capabilities"]["can_comment"], false);
    assert_eq!(json(&created)["capabilities"]["can_delete"], false);
    assert!(fs::read(&archive).unwrap().starts_with(b"MRAR\r\n\x1a\n"));
    let listed = run(&["list", archive.to_str().unwrap(), "--json"]);
    assert!(listed.status.success());
    assert_eq!(json(&listed)["entries"].as_array().unwrap().len(), 2);
    assert_eq!(
        json(&listed)["archive_identity"],
        json(&created)["identity"]
    );
    let tested = run(&["test", archive.to_str().unwrap(), "--json"]);
    assert!(
        tested.status.success(),
        "{}",
        String::from_utf8_lossy(&tested.stdout)
    );
    let plan = run(&[
        "plan",
        archive.to_str().unwrap(),
        "--destination",
        output.to_str().unwrap(),
        "--json",
    ]);
    assert!(plan.status.success());
    let plan_path = dir.path().join("plan.json");
    fs::write(&plan_path, &plan.stdout).unwrap();
    let extracted = run(&["extract-plan", plan_path.to_str().unwrap(), "--json"]);
    assert!(
        extracted.status.success(),
        "{}",
        String::from_utf8_lossy(&extracted.stdout)
    );
    assert_eq!(
        fs::read(output.join("marker.txt")).unwrap(),
        fs::read(&first).unwrap()
    );
    assert_eq!(fs::read(output.join("binary.bin")).unwrap(), binary_bytes);
    let scanned = run(&["scan", archive.to_str().unwrap(), "--json"]);
    assert!(
        scanned.status.success(),
        "{}",
        String::from_utf8_lossy(&scanned.stdout)
    );
    assert_eq!(json(&scanned)["coverage"]["complete"], true);
    assert_eq!(
        json(&scanned)["archive_identity"],
        json(&created)["identity"]
    );
    assert!(json(&scanned)["rule_findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["code"] == "WonRaR_Test_Sentinel"));
    let before = fs::read(&archive).unwrap();
    let repeated = run(&[
        "create",
        "--output",
        archive.to_str().unwrap(),
        first.to_str().unwrap(),
        "--json",
    ]);
    assert!(!repeated.status.success());
    assert_eq!(fs::read(&archive).unwrap(), before);
}

#[test]
fn experimental_mrar_refuses_password_instead_of_creating_plaintext() {
    use std::{io::Write, process::Stdio};
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("private.txt");
    let archive = dir.path().join("private.mrar");
    fs::write(&input, b"private fixture").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_maxrar-cli"))
        .args(["create", "--output"])
        .arg(&archive)
        .arg(&input)
        .args(["--password-stdin", "--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"private-mrar-password\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(!archive.exists());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("private-mrar-password"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("private-mrar-password"));
}

#[test]
fn selected_worker_scan_is_explicit_and_tied_to_reviewed_identity() {
    let dir = tempfile::tempdir().unwrap();
    let clean = dir.path().join("clean.txt");
    let marker = dir.path().join("marker.txt");
    let archive = dir.path().join("selection.zip");
    fs::write(&clean, b"plain text").unwrap();
    fs::write(&marker, b"WONRAR-BENIGN-SCAN-TEST-2026").unwrap();
    let created = run(&[
        "create",
        "--output",
        archive.to_str().unwrap(),
        clean.to_str().unwrap(),
        marker.to_str().unwrap(),
    ]);
    assert!(created.status.success());
    let value = json(&created);
    let identity = value["identity"].as_str().unwrap();
    let entries = value["entries"].as_array().unwrap();
    let clean_id = entries.iter().find(|e| e["name"] == "clean.txt").unwrap()["id"]
        .as_str()
        .unwrap();
    let marker_id = entries.iter().find(|e| e["name"] == "marker.txt").unwrap()["id"]
        .as_str()
        .unwrap();
    let selected = run(&[
        "scan",
        archive.to_str().unwrap(),
        "--entry",
        clean_id,
        "--expect-identity",
        identity,
    ]);
    assert!(
        selected.status.success(),
        "{}",
        String::from_utf8_lossy(&selected.stdout)
    );
    let report = json(&selected);
    assert_eq!(report["scope"]["kind"], "selection");
    assert_eq!(report["coverage"]["total_files"], 1);
    assert_eq!(report["rule_findings"].as_array().unwrap().len(), 0);
    let selected = run(&[
        "scan",
        archive.to_str().unwrap(),
        "--entry",
        marker_id,
        "--expect-identity",
        identity,
    ]);
    assert!(selected.status.success());
    assert!(!json(&selected)["rule_findings"]
        .as_array()
        .unwrap()
        .is_empty());
    let stale = run(&[
        "scan",
        archive.to_str().unwrap(),
        "--entry",
        clean_id,
        "--expect-identity",
        "stale",
    ]);
    assert!(!stale.status.success());
    assert!(json(&stale)["error"]
        .as_str()
        .unwrap()
        .contains("changed since review"));
    let missing = run(&["scan", archive.to_str().unwrap(), "--entry", clean_id]);
    assert!(!missing.status.success());
}
