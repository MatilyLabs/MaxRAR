use std::{fs, io::Write, path::Path, sync::atomic::AtomicBool};
use wonrar_core::*;
use zip::{write::SimpleFileOptions, ZipWriter};

fn fixture(path: &Path, entries: &[(&str, &[u8])]) {
    let mut writer = ZipWriter::new(fs::File::create(path).unwrap());
    for (name, contents) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(contents).unwrap();
    }
    writer.finish().unwrap();
}
fn cancel() -> AtomicBool {
    AtomicBool::new(false)
}

#[test]
fn roundtrip_preview_plan_extract_and_crc_test() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("README.md");
    fs::write(&source, b"hello archive").unwrap();
    let archive = temp.path().join("bundle.zip");
    let info = create(&archive, &[source], &cancel(), &|_| {}).unwrap();
    assert_eq!(info.entries[0].name, "README.md");
    let view = preview(&archive, "0", 5).unwrap();
    assert_eq!(view.text, "hello");
    assert!(view.truncated);
    let dest = temp.path().join("out");
    let plan = plan(
        &archive,
        &dest,
        &[],
        ConflictPolicy::Skip,
        Limits::default(),
    )
    .unwrap();
    let receipt = extract(&plan, &cancel(), &|_| {}).unwrap();
    assert_eq!(receipt.written.len(), 1);
    assert_eq!(fs::read(dest.join("README.md")).unwrap(), b"hello archive");
    assert_eq!(
        test(&archive, &cancel(), &|_| {}).unwrap().bytes_processed,
        13
    );
}

#[test]
fn hostile_paths_are_blocked_and_normal_file_survives() {
    let t = tempfile::tempdir().unwrap();
    let archive = t.path().join("a.zip");
    fixture(
        &archive,
        &[
            ("../escape", b"bad"),
            ("/absolute", b"bad"),
            ("C:/escape", b"bad"),
            ("a\\escape", b"bad"),
            ("CON.txt", b"bad"),
            ("safe.txt", b"ok"),
        ],
    );
    let p = plan(
        &archive,
        &t.path().join("out"),
        &[],
        ConflictPolicy::Skip,
        Limits::default(),
    )
    .unwrap();
    assert_eq!(
        p.entries.iter().filter(|e| e.action == "blocked").count(),
        5
    );
    let r = extract(&p, &cancel(), &|_| {}).unwrap();
    assert_eq!(r.written, ["safe.txt"]);
    assert!(!t.path().join("escape").exists());
}

#[test]
fn no_silent_overwrite_and_changed_plan_is_rejected() {
    let t = tempfile::tempdir().unwrap();
    let archive = t.path().join("a.zip");
    fixture(&archive, &[("file.txt", b"new")]);
    let dest = t.path().join("out");
    fs::create_dir(&dest).unwrap();
    fs::write(dest.join("file.txt"), b"old").unwrap();
    let p = plan(
        &archive,
        &dest,
        &[],
        ConflictPolicy::Skip,
        Limits::default(),
    )
    .unwrap();
    extract(&p, &cancel(), &|_| {}).unwrap();
    assert_eq!(fs::read(dest.join("file.txt")).unwrap(), b"old");
    let p = plan(
        &archive,
        &dest,
        &[],
        ConflictPolicy::KeepBoth,
        Limits::default(),
    )
    .unwrap();
    assert_eq!(p.entries[0].output_name, "file.txt (2)");
    extract(&p, &cancel(), &|_| {}).unwrap();
    assert_eq!(fs::read(dest.join("file.txt (2)")).unwrap(), b"new");
    let p = plan(
        &archive,
        &dest,
        &[],
        ConflictPolicy::Replace,
        Limits::default(),
    )
    .unwrap();
    extract(&p, &cancel(), &|_| {}).unwrap();
    assert_eq!(fs::read(dest.join("file.txt")).unwrap(), b"new");
    let fresh = t.path().join("fresh");
    let p = plan(
        &archive,
        &fresh,
        &[],
        ConflictPolicy::Skip,
        Limits::default(),
    )
    .unwrap();
    fs::create_dir(&fresh).unwrap();
    fs::write(fresh.join("file.txt"), b"surprise").unwrap();
    assert!(extract(&p, &cancel(), &|_| {}).is_err());
    assert_eq!(fs::read(fresh.join("file.txt")).unwrap(), b"surprise");
}

#[test]
fn identity_and_client_plan_tampering_are_rejected() {
    let t = tempfile::tempdir().unwrap();
    let archive = t.path().join("a.zip");
    fixture(&archive, &[("a.txt", b"old")]);
    let mut p = plan(
        &archive,
        &t.path().join("out"),
        &[],
        ConflictPolicy::Skip,
        Limits::default(),
    )
    .unwrap();
    p.entries[0].output_name = "../../escape".into();
    assert!(extract(&p, &cancel(), &|_| {}).is_err());
    let p = plan(
        &archive,
        &t.path().join("out"),
        &[],
        ConflictPolicy::Skip,
        Limits::default(),
    )
    .unwrap();
    fixture(&archive, &[("a.txt", b"new")]);
    assert!(extract(&p, &cancel(), &|_| {}).is_err());
    assert!(!t.path().join("out").exists());
}

#[test]
fn limits_cancel_and_creation_no_clobber() {
    let t = tempfile::tempdir().unwrap();
    let archive = t.path().join("a.zip");
    fixture(&archive, &[("a", &[b'a'; 20000])]);
    let p = plan(
        &archive,
        &t.path().join("out"),
        &[],
        ConflictPolicy::Skip,
        Limits {
            max_file_bytes: 10,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(p.entries[0].action, "blocked");
    assert!(plan(
        &archive,
        &t.path().join("out"),
        &[],
        ConflictPolicy::Skip,
        Limits {
            max_total_bytes: 10,
            ..Limits::default()
        }
    )
    .is_err());
    let p = plan(
        &archive,
        &t.path().join("out"),
        &[],
        ConflictPolicy::Skip,
        Limits::default(),
    )
    .unwrap();
    assert!(matches!(
        extract(&p, &AtomicBool::new(true), &|_| {}),
        Err(Error::Cancelled)
    ));
    let source = t.path().join("input.txt");
    fs::write(&source, b"source").unwrap();
    let before = fs::read(&archive).unwrap();
    assert!(create(&archive, &[source], &cancel(), &|_| {}).is_err());
    assert_eq!(fs::read(&archive).unwrap(), before);
}

#[test]
fn transactional_edits_preserve_payload_and_cancelled_original() {
    let t = tempfile::tempdir().unwrap();
    let archive = t.path().join("a.zip");
    fixture(&archive, &[("a", b"one"), ("b", b"two")]);
    let original = fs::read(&archive).unwrap();
    assert!(set_comment(&archive, "no", &AtomicBool::new(true)).is_err());
    assert_eq!(fs::read(&archive).unwrap(), original);
    assert_eq!(
        set_comment(&archive, "my note", &cancel()).unwrap().comment,
        "my note"
    );
    let info = delete_entries(&archive, &["0".into()], &cancel()).unwrap();
    assert_eq!(info.entries.len(), 1);
    assert_eq!(info.entries[0].name, "b");
    assert_eq!(preview(&archive, "0", 100).unwrap().text, "two");
    assert_eq!(info.comment, "my note");
}

#[cfg(unix)]
#[test]
fn destination_symlink_cannot_escape_scope() {
    use std::os::unix::fs::symlink;
    let t = tempfile::tempdir().unwrap();
    let archive = t.path().join("a.zip");
    fixture(&archive, &[("link/escape.txt", b"bad")]);
    let dest = t.path().join("out");
    let external = t.path().join("outside");
    fs::create_dir(&dest).unwrap();
    fs::create_dir(&external).unwrap();
    symlink(&external, dest.join("link")).unwrap();
    let p = plan(
        &archive,
        &dest,
        &[],
        ConflictPolicy::Skip,
        Limits::default(),
    )
    .unwrap();
    assert!(p.entries[0].conflict);
    assert_eq!(p.entries[0].action, "skip");
    assert!(extract(&p, &cancel(), &|_| {}).unwrap().written.is_empty());
    assert!(!external.join("escape.txt").exists());
    assert!(fs::read_dir(&dest).unwrap().all(|e| !e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".part")));
}

#[test]
fn cancellation_during_payload_cleans_staged_file() {
    use std::sync::atomic::Ordering;
    let t = tempfile::tempdir().unwrap();
    let archive = t.path().join("a.zip");
    fixture(&archive, &[("a", &[b'a'; 200000])]);
    let dest = t.path().join("out");
    let p = plan(
        &archive,
        &dest,
        &[],
        ConflictPolicy::Skip,
        Limits::default(),
    )
    .unwrap();
    let flag = cancel();
    assert!(matches!(
        extract(&p, &flag, &|_| {
            flag.store(true, Ordering::Relaxed);
        }),
        Err(Error::Cancelled)
    ));
    assert_eq!(fs::read_dir(dest).unwrap().count(), 0);
}

#[test]
fn keep_both_renames_implicit_directory_and_children_together() {
    let t = tempfile::tempdir().unwrap();
    let a = t.path().join("a.zip");
    fixture(
        &a,
        &[("folder/one.txt", b"one"), ("folder/two.txt", b"two")],
    );
    let dest = t.path().join("out");
    fs::create_dir(&dest).unwrap();
    fs::write(dest.join("folder"), b"keep").unwrap();
    let p = plan(&a, &dest, &[], ConflictPolicy::KeepBoth, Limits::default()).unwrap();
    assert_eq!(p.entries[0].output_name, "folder (2)/one.txt");
    extract(&p, &cancel(), &|_| {}).unwrap();
    assert_eq!(fs::read(dest.join("folder (2)/two.txt")).unwrap(), b"two");
    assert_eq!(fs::read(dest.join("folder")).unwrap(), b"keep");
}

#[test]
fn aes_payload_requires_correct_password_and_failed_decrypt_publishes_nothing() {
    let t = tempfile::tempdir().unwrap();
    let source = t.path().join("secret.txt");
    fs::write(&source, b"private payload").unwrap();
    let a = t.path().join("encrypted.zip");
    let info = create_with_password(&a, &[source], &cancel(), &|_| {}, Some("pássword")).unwrap();
    assert!(info.entries[0].is_encrypted);
    // ZIP exposes directory metadata; this must never imply readable payloads.
    let inventory = inspect(&a).unwrap();
    assert_eq!(inventory.entries[0].name, "secret.txt");
    assert!(inventory.entries[0].is_encrypted);
    assert!(preview(&a, "0", 100).is_err());
    assert!(preview_with_password(&a, "0", 100, Some("wrong")).is_err());
    assert_eq!(
        preview_with_password(&a, "0", 100, Some("pássword"))
            .unwrap()
            .text,
        "private payload"
    );
    assert!(test(&a, &cancel(), &|_| {}).is_err());
    test_with_password(&a, &cancel(), &|_| {}, Some("pássword")).unwrap();
    let dest = t.path().join("out");
    let p = plan_with_password(
        &a,
        &dest,
        &[],
        ConflictPolicy::Skip,
        Limits::default(),
        Some("wrong"),
    )
    .unwrap();
    assert!(extract_with_password(&p, &cancel(), &|_| {}, Some("wrong")).is_err());
    assert!(!dest.join("secret.txt").exists());
    let p = plan_with_password(
        &a,
        &dest,
        &[],
        ConflictPolicy::Skip,
        Limits::default(),
        Some("pássword"),
    )
    .unwrap();
    extract_with_password(&p, &cancel(), &|_| {}, Some("pássword")).unwrap();
    assert_eq!(
        fs::read(dest.join("secret.txt")).unwrap(),
        b"private payload"
    );
}

#[test]
fn unicode_collisions_and_truncated_multibyte_text_are_handled() {
    let t = tempfile::tempdir().unwrap();
    let a = t.path().join("a.zip");
    fixture(
        &a,
        &[("é.txt", "éé".as_bytes()), ("e\u{301}.txt", b"other")],
    );
    let info = inspect(&a).unwrap();
    assert!(info.findings.iter().any(|f| f.code == "duplicate_path"));
    assert_eq!(preview(&a, "0", 3).unwrap().text, "é");
    let p = plan(
        &a,
        &t.path().join("out"),
        &[],
        ConflictPolicy::Skip,
        Limits::default(),
    )
    .unwrap();
    assert_eq!(p.entries[1].action, "skip");
}

#[test]
fn sevenz_roundtrip_with_password_and_explicit_read_only_edits() {
    let t = tempfile::tempdir().unwrap();
    let source = t.path().join("hello.txt");
    fs::write(&source, "héllo sevenz").unwrap();
    let a = t.path().join("a.7z");
    let info = create_with_password(&a, &[source], &cancel(), &|_| {}, Some("password")).unwrap();
    assert_eq!(info.format, "7z");
    assert!(info.entries[0].is_encrypted);
    assert!(!info.capabilities.can_delete);
    assert!(!info.capabilities.can_comment);
    assert!(preview(&a, "0", 100).is_err());
    assert_eq!(
        preview_with_password(&a, "0", 100, Some("password"))
            .unwrap()
            .text,
        "héllo sevenz"
    );
    assert!(test_with_password(&a, &cancel(), &|_| {}, Some("wrong")).is_err());
    test_with_password(&a, &cancel(), &|_| {}, Some("password")).unwrap();
    let dest = t.path().join("out");
    let p = plan_with_password(
        &a,
        &dest,
        &[],
        ConflictPolicy::Skip,
        Limits::default(),
        Some("password"),
    )
    .unwrap();
    extract_with_password(&p, &cancel(), &|_| {}, Some("password")).unwrap();
    assert_eq!(
        fs::read_to_string(dest.join("hello.txt")).unwrap(),
        "héllo sevenz"
    );
    let original = fs::read(&a).unwrap();
    assert!(delete_entries(&a, &["0".into()], &cancel()).is_err());
    assert_eq!(fs::read(&a).unwrap(), original);
}

#[test]
fn sevenz_traversal_and_symlink_metadata_are_blocked() {
    use sevenz_rust2::{ArchiveEntry, ArchiveWriter};
    let t = tempfile::tempdir().unwrap();
    let a = t.path().join("a.7z");
    let mut w = ArchiveWriter::create(&a).unwrap();
    w.push_archive_entry(ArchiveEntry::new_file("../escape"), Some("bad".as_bytes()))
        .unwrap();
    let mut link = ArchiveEntry::new_file("link");
    link.has_windows_attributes = true;
    link.windows_attributes = 0o120777 << 16;
    w.push_archive_entry(link, Some("../outside".as_bytes()))
        .unwrap();
    w.push_archive_entry(ArchiveEntry::new_file("safe.txt"), Some("okay".as_bytes()))
        .unwrap();
    w.finish().unwrap();
    let p = plan(
        &a,
        &t.path().join("out"),
        &[],
        ConflictPolicy::Skip,
        Limits::default(),
    )
    .unwrap();
    assert_eq!(
        p.entries.iter().filter(|e| e.action == "blocked").count(),
        2
    );
    let r = extract(&p, &cancel(), &|_| {}).unwrap();
    assert_eq!(r.written, ["safe.txt"]);
    assert!(!t.path().join("escape").exists());
}

#[test]
fn corrupt_payload_does_not_publish_failed_file() {
    let t = tempfile::tempdir().unwrap();
    let a = t.path().join("a.zip");
    let mut w = ZipWriter::new(fs::File::create(&a).unwrap());
    w.start_file(
        "file.txt",
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )
    .unwrap();
    w.write_all(b"payload content").unwrap();
    w.finish().unwrap();
    let mut bytes = fs::read(&a).unwrap();
    let offset = bytes
        .windows(15)
        .position(|b| b == b"payload content")
        .unwrap();
    bytes[offset] ^= 1;
    fs::write(&a, bytes).unwrap();
    assert!(test(&a, &cancel(), &|_| {}).is_err());
    let dest = t.path().join("out");
    let p = plan(&a, &dest, &[], ConflictPolicy::Skip, Limits::default()).unwrap();
    assert!(extract(&p, &cancel(), &|_| {}).is_err());
    assert_eq!(fs::read_dir(&dest).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn creation_rejects_output_inside_symlinked_source_alias() {
    use std::os::unix::fs::symlink;
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("source");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("a.txt"), b"input").unwrap();
    let alias = t.path().join("alias");
    symlink(t.path(), &alias).unwrap();
    for extension in ["zip", "7z"] {
        assert!(create(
            &alias.join("source").join(format!("out.{extension}")),
            std::slice::from_ref(&dir),
            &cancel(),
            &|_| {}
        )
        .is_err());
    }
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
}

#[test]
fn ancestor_files_are_visible_conflicts_without_partial_runtime_failure() {
    for policy in [
        ConflictPolicy::Skip,
        ConflictPolicy::Replace,
        ConflictPolicy::KeepBoth,
    ] {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a.zip");
        fixture(&a, &[("ok.txt", b"okay"), ("folder/child.txt", b"child")]);
        let dest = t.path().join("out");
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("folder"), b"keep ancestor").unwrap();
        let p = plan(&a, &dest, &[], policy, Limits::default()).unwrap();
        let child = &p.entries[1];
        if policy == ConflictPolicy::KeepBoth {
            assert_eq!(child.output_name, "folder (2)/child.txt");
            assert_eq!(child.action, "extract");
        } else {
            assert!(child.conflict);
            assert_eq!(
                child.action,
                if policy == ConflictPolicy::Skip {
                    "skip"
                } else {
                    "blocked"
                }
            );
            assert!(child.reason.as_ref().unwrap().contains("ancestor"));
        }
        let receipt = extract(&p, &cancel(), &|_| {}).unwrap();
        assert!(receipt.written.contains(&"ok.txt".to_string()));
        assert_eq!(fs::read(dest.join("folder")).unwrap(), b"keep ancestor");
        if policy == ConflictPolicy::KeepBoth {
            assert_eq!(
                fs::read(dest.join("folder (2)/child.txt")).unwrap(),
                b"child"
            );
        }
    }
}

#[test]
fn archive_file_directory_prefix_collisions_are_planned_independent_of_order() {
    for reverse in [false, true] {
        for policy in [
            ConflictPolicy::Skip,
            ConflictPolicy::Replace,
            ConflictPolicy::KeepBoth,
        ] {
            let t = tempfile::tempdir().unwrap();
            let a = t.path().join("a.zip");
            let mut entries = vec![
                ("a", b"root file".as_slice()),
                ("a/b.txt", b"child file".as_slice()),
            ];
            if reverse {
                entries.reverse();
            }
            fixture(&a, &entries);
            let dest = t.path().join("out");
            let p = plan(&a, &dest, &[], policy, Limits::default()).unwrap();
            let file = p.entries.iter().find(|e| e.source_name == "a").unwrap();
            assert!(file.conflict);
            assert_eq!(
                file.action,
                match policy {
                    ConflictPolicy::Skip => "skip",
                    ConflictPolicy::Replace => "blocked",
                    ConflictPolicy::KeepBoth => "extract",
                }
            );
            if policy == ConflictPolicy::KeepBoth {
                assert_eq!(file.output_name, "a (2)");
            }
            extract(&p, &cancel(), &|_| {}).unwrap();
            assert_eq!(fs::read(dest.join("a/b.txt")).unwrap(), b"child file");
            if policy == ConflictPolicy::KeepBoth {
                assert_eq!(fs::read(dest.join("a (2)")).unwrap(), b"root file");
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn concurrent_destination_parent_swap_cannot_escape_scope() {
    use std::{os::unix::fs::symlink, sync::atomic::Ordering};
    let t = tempfile::tempdir().unwrap();
    let a = t.path().join("a.zip");
    fixture(&a, &[("nested/file.txt", &[b'x'; 200000])]);
    let dest = t.path().join("out");
    let outside = t.path().join("outside");
    fs::create_dir(&outside).unwrap();
    let p = plan(&a, &dest, &[], ConflictPolicy::Skip, Limits::default()).unwrap();
    let swapped = AtomicBool::new(false);
    let r = extract(&p, &cancel(), &|_| {
        if !swapped.swap(true, Ordering::Relaxed) {
            fs::rename(dest.join("nested"), dest.join("moved")).unwrap();
            symlink(&outside, dest.join("nested")).unwrap();
        }
    });
    assert!(r.is_err());
    assert!(swapped.load(Ordering::Relaxed));
    assert!(!outside.join("file.txt").exists());
    assert!(fs::read_dir(&dest).unwrap().all(|e| !e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".part")));
}

#[test]
fn checked_mutations_reject_stale_inventory_ids_and_comments_without_changes() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("a.zip");
    fixture(&path, &[("old.txt", b"old")]);
    let old = inspect(&path).unwrap();
    let stale_id = old.entries[0].id.clone();
    fixture(
        &path,
        &[
            ("unrelated.txt", b"new repacked content"),
            ("keep.txt", b"preserve"),
        ],
    );
    let current = fs::read(&path).unwrap();
    assert!(delete_entries_checked(
        &path,
        std::slice::from_ref(&stale_id),
        &old.identity,
        &cancel()
    )
    .unwrap_err()
    .to_string()
    .contains("changed"));
    assert_eq!(fs::read(&path).unwrap(), current);
    assert!(
        set_comment_checked(&path, "stale edit", &old.identity, &cancel())
            .unwrap_err()
            .to_string()
            .contains("changed")
    );
    assert_eq!(fs::read(&path).unwrap(), current);
    let refreshed = inspect(&path).unwrap();
    let edited = set_comment_checked(&path, "reviewed", &refreshed.identity, &cancel()).unwrap();
    assert_eq!(edited.comment, "reviewed");
    let updated = delete_entries_checked(
        &path,
        std::slice::from_ref(&edited.entries[0].id),
        &edited.identity,
        &cancel(),
    )
    .unwrap();
    assert_eq!(updated.entries.len(), 1);
    assert_eq!(updated.entries[0].name, "keep.txt");
}
