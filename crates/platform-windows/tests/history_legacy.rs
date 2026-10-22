//! Compatibility checks use the retired raw-snapshot layout and the public migration interface.
#![cfg(windows)]

use editor_history::{Capture, History, LegacyQuery, Policy, Query, Reason};
use std::path::{Path, PathBuf};

/// Reproduce the documented retired writer's path bucket, not the new importer's implementation.
fn old_snapshot(root: &Path, original: &Path, at_ms: i64, bytes: &[u8]) -> PathBuf {
    let hash = original
        .to_string_lossy()
        .bytes()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
    let directory = root.join(format!("{hash:016x}"));
    std::fs::create_dir_all(&directory).unwrap();
    let target = directory.join(format!("{at_ms}-note.txt"));
    std::fs::write(&target, bytes).unwrap();
    target
}

/// Import preserves original bytes/time, verifies before removing the old copy, and survives retry.
#[test]
fn known_path_import_preserves_time_and_is_retryable_after_source_cleanup() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let original = directory.path().join("project/note.txt");
    let bytes = "\u{feff}旧内容\r\n".as_bytes();
    let source = old_snapshot(&root, &original, 1_000, bytes);
    let history = open_history(&root, Policy::default()).unwrap();
    history.refresh_legacy().unwrap();
    let entry = history.legacy(&LegacyQuery::default()).unwrap().remove(0);
    let version = history.import_legacy(&entry, &original, None).unwrap();
    assert_eq!(history.read(version).unwrap(), bytes);
    assert_eq!(history.list(&Query::default()).unwrap()[0].at_ms, 1_000);
    assert!(!source.exists());
    assert!(!original.exists());
    drop(history);
    let history = open_history(&root, Policy::default()).unwrap();
    assert_eq!(
        history.import_legacy(&entry, &original, None).unwrap(),
        version
    );
    assert!(history.legacy(&LegacyQuery::default()).unwrap().is_empty());
}

/// Old unassociated files consume the same budget and are reclaimed only after the new state commits.
#[test]
fn capacity_reclaims_legacy_bytes_after_committing_the_new_state() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let source = old_snapshot(
        &root,
        &directory.path().join("old.txt"),
        1_000,
        &vec![b'x'; 128 * 1024],
    );
    let history = open_history(
        &root,
        Policy {
            max_total_bytes: 128 * 1024,
            ..Policy::default()
        },
    )
    .unwrap();
    history.refresh_legacy().unwrap();
    let version = history
        .capture(Capture {
            file: None,
            path: directory.path().join("new.txt"),
            workspace: None,
            contents: b"new".to_vec(),
            reason: Reason::Saved,
            at_ms: 2_000,
        })
        .unwrap()
        .version()
        .unwrap();
    assert_eq!(history.read(version).unwrap(), b"new");
    assert!(!source.exists());
}

/// Raw old entries use the same calendar expiry without needing a guessed project identity.
#[test]
fn legacy_expiry_removes_only_the_older_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let original = directory.path().join("note.txt");
    let old = old_snapshot(&root, &original, 1_000, b"old");
    let recent = old_snapshot(&root, &original, 86_401_001, b"recent");
    let history = open_history(
        &root,
        Policy {
            retention_days: 1,
            ..Policy::default()
        },
    )
    .unwrap();
    history.refresh_legacy().unwrap();
    history.maintain(86_401_001).unwrap();
    assert!(!old.exists());
    let retained = history.legacy(&LegacyQuery::default()).unwrap();
    assert_eq!(
        (retained.len(), std::fs::read(recent).unwrap()),
        (1, b"recent".to_vec())
    );
}

/// One deleted source's legacy bucket remains one logical file across all of its snapshots.
#[test]
fn deleted_legacy_versions_share_one_file_identity() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let original = directory.path().join("missing.txt");
    old_snapshot(&root, &original, 1_000, b"first");
    old_snapshot(&root, &original, 2_000, b"second");
    let history = open_history(&root, Policy::default()).unwrap();
    assert_eq!(history.import_known(&original, None).unwrap(), 2);
    let rows = history
        .list(&Query {
            deleted_only: true,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].file, rows[1].file);
}

/// A sharing violation preserves both committed content and the only old source until retry succeeds.
#[test]
fn legacy_gc_failure_keeps_a_durable_retry_ledger() {
    use std::os::windows::fs::OpenOptionsExt;
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let original = directory.path().join("note.txt");
    let source = old_snapshot(&root, &original, 1_000, b"must survive");
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&source)
        .unwrap();
    let history = open_history(&root, Policy::default()).unwrap();
    let entry = history.legacy(&LegacyQuery::default()).unwrap().remove(0);
    assert!(history.import_legacy(&entry, &original, None).is_err());
    assert!(source.exists());
    let rows = history.list(&Query::default()).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(history.read(rows[0].id).unwrap(), b"must survive");
    assert!(history.stats().unwrap().pending_gc_bytes > 0);
    drop(held);
    history.maintain(1_001).unwrap();
    assert!(!source.exists());
    assert_eq!(
        history.import_legacy(&entry, &original, None).unwrap(),
        rows[0].id
    );
    assert_eq!(history.list(&Query::default()).unwrap().len(), 1);
}

/// The raw export path remains available when a retired payload is binary and above the preview limit.
#[test]
fn oversized_binary_legacy_export_streams_original_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let bytes = vec![0xff; 32 * 1024 * 1024 + 1];
    old_snapshot(&root, &directory.path().join("binary.bin"), 1_000, &bytes);
    let history = open_history(&root, Policy::default()).unwrap();
    let entry = history.legacy(&LegacyQuery::default()).unwrap().remove(0);
    assert!(matches!(
        history.read_legacy(&entry),
        Err(editor_history::Error::TooLarge)
    ));
    let mut output = tempfile::tempfile().unwrap();
    history.copy_legacy(&entry, &mut output).unwrap();
    use std::io::{Read, Seek};
    output.rewind().unwrap();
    let mut copied = Vec::new();
    output.read_to_end(&mut copied).unwrap();
    assert_eq!(copied, bytes);
}

/// A process ending after the migration ledger commits leaves the verified target and retryable source.
#[test]
fn history_migration_gc_process_interruption_is_recoverable() {
    let directory = tempfile::tempdir().unwrap();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "history_migration_crash_child",
            "--ignored",
            "--nocapture",
        ])
        .env("NANOBUG_HISTORY_LEGACY_CRASH_ROOT", directory.path())
        .output()
        .unwrap();
    assert_eq!(
        result.status.code(),
        Some(24),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let history = open_history(directory.path().join("history"), Policy::default()).unwrap();
    history.maintain(1_001).unwrap();
    assert!(history.legacy(&LegacyQuery::default()).unwrap().is_empty());
    let rows = history.list(&Query::default()).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].at_ms, 1_000);
    assert_eq!(history.read(rows[0].id).unwrap(), b"migrated");
    assert_eq!(history.stats().unwrap().pending_gc_bytes, 0);
}

/// Inject a process exit at the native GC seam, after durable capture and migration metadata commits.
#[test]
#[ignore = "run only from the private migration crash parent"]
fn history_migration_crash_child() {
    struct ExitBeforeDelete;
    impl editor_history::LegacyIo for ExitBeforeDelete {
        fn open(
            &self,
            root: &Path,
            relative: &Path,
            expected: editor_history::LegacyFingerprint,
        ) -> std::io::Result<std::fs::File> {
            editor_history::LegacyIo::open(
                &platform_windows::history::NativeLegacyIo,
                root,
                relative,
                expected,
            )
        }
        fn remove(
            &self,
            _: &Path,
            _: &Path,
            _: editor_history::LegacyFingerprint,
        ) -> std::io::Result<()> {
            std::process::exit(24)
        }
    }
    let directory = PathBuf::from(std::env::var_os("NANOBUG_HISTORY_LEGACY_CRASH_ROOT").unwrap());
    let root = directory.join("history");
    let original = directory.join("note.txt");
    old_snapshot(&root, &original, 1_000, b"migrated");
    let history = History::open_with_legacy_io(
        &root,
        Policy::default(),
        Some(std::sync::Arc::new(ExitBeforeDelete)),
    )
    .unwrap();
    history.import_known(&original, None).unwrap();
    panic!("migration never reached native reclamation");
}

/// Known independent identities at one path make the path-hash association ambiguous, so raw bytes stay available.
#[test]
fn ambiguous_legacy_path_is_not_assigned_to_a_recreated_file() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let original = directory.path().join("note.txt");
    let source = old_snapshot(&root, &original, 1_000, b"unassociated");
    let history = open_history(&root, Policy::default()).unwrap();
    history
        .capture(Capture {
            file: None,
            path: original.clone(),
            workspace: None,
            contents: b"first".to_vec(),
            reason: Reason::Saved,
            at_ms: 2_000,
        })
        .unwrap();
    let file = history.file_at(&original).unwrap().unwrap();
    history
        .change(editor_history::Change {
            file,
            path: original.clone(),
            workspace: None,
            reason: Reason::Deleted,
            present: false,
            at_ms: 2_001,
        })
        .unwrap();
    history
        .capture(Capture {
            file: None,
            path: original.clone(),
            workspace: None,
            contents: b"second".to_vec(),
            reason: Reason::Saved,
            at_ms: 3_000,
        })
        .unwrap();
    assert_eq!(history.import_known(&original, None).unwrap(), 0);
    assert!(source.exists());
    assert_eq!(history.legacy(&LegacyQuery::default()).unwrap().len(), 1);
}

/// A verified old spelling from a reliable rename imports into the current identity without reviving the old name.
#[test]
fn legacy_import_after_a_reliable_move_keeps_the_current_path_and_identity() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let original = directory.path().join("old.txt");
    let current = directory.path().join("new.txt");
    old_snapshot(&root, &original, 1_000, b"retired");
    std::fs::write(&original, "current").unwrap();
    let history = open_history(&root, Policy::default()).unwrap();
    history
        .capture(Capture {
            file: None,
            path: original.clone(),
            workspace: None,
            contents: b"current".to_vec(),
            reason: Reason::Saved,
            at_ms: 2_000,
        })
        .unwrap();
    let file = history.file_at(&original).unwrap().unwrap();
    std::fs::rename(&original, &current).unwrap();
    history
        .change(editor_history::Change {
            file,
            path: current.clone(),
            workspace: None,
            reason: Reason::Renamed,
            present: true,
            at_ms: 2_001,
        })
        .unwrap();
    assert_eq!(history.import_known_into(&original, None, file).unwrap(), 1);
    let location = history.location(file).unwrap();
    assert!(location.present);
    assert_eq!(location.path, current);
    let rows = history
        .list(&Query {
            file: Some(file),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(rows.len(), 3);
    assert!(
        rows.iter()
            .any(|row| row.reason == Reason::Imported && row.at_ms == 1_000)
    );
}

/// Exercise the production native adapter through the public platform-independent store.
fn open_history(root: impl AsRef<Path>, policy: Policy) -> editor_history::Result<History> {
    History::open_with_legacy_io(
        root,
        policy,
        Some(std::sync::Arc::new(
            platform_windows::history::NativeLegacyIo,
        )),
    )
}
