//! External storage failures and unsupported inputs must preserve the user's existing data.

use editor_history::{Capture, History, Policy, Reason};

/// A foreign database is an external input fixture, not a history database queried for assertions.
#[test]
fn foreign_database_is_rejected_without_replacing_its_bytes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("history.sqlite3");
    let foreign = rusqlite::Connection::open(&path).unwrap();
    foreign.execute_batch("PRAGMA application_id=42; CREATE TABLE unrelated(value TEXT); INSERT INTO unrelated VALUES('keep me');").unwrap();
    drop(foreign);
    let before = std::fs::read(&path).unwrap();
    let result = History::open(root.path(), Policy::default());
    assert!(matches!(
        result,
        Err(editor_history::Error::UnsupportedFormat)
    ));
    assert_eq!(std::fs::read(path).unwrap(), before);
}

/// An externally supplied unsupported object codec must not be read as if it were known Zstd.
#[test]
fn unknown_object_encoding_is_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let history = History::open(&root, Policy::default()).unwrap();
    let version = history
        .capture(Capture {
            file: None,
            path: directory.path().join("note.txt"),
            workspace: None,
            contents: b"keep".to_vec(),
            reason: Reason::Saved,
            at_ms: 1_000,
        })
        .unwrap()
        .version()
        .unwrap();
    drop(history);
    let external = rusqlite::Connection::open(root.join("history.sqlite3")).unwrap();
    external.execute("UPDATE objects SET codec=99", []).unwrap();
    drop(external);
    let history = History::open(&root, Policy::default()).unwrap();
    assert!(matches!(
        history.read(version),
        Err(editor_history::Error::UnsupportedFormat)
    ));
}

/// Deduplication must never acknowledge a corrupt shared object as durable restore protection.
#[test]
fn corrupt_shared_object_cannot_be_reused_as_protection() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let capture = || Capture {
        file: None,
        path: directory.path().join("note.txt"),
        workspace: None,
        contents: b"only unsaved copy".to_vec(),
        reason: Reason::Saved,
        at_ms: 1_000,
    };
    let history = History::open(&root, Policy::default()).unwrap();
    history.capture(capture()).unwrap();
    drop(history);
    let external = rusqlite::Connection::open(root.join("history.sqlite3")).unwrap();
    external.execute("UPDATE objects SET codec=99", []).unwrap();
    drop(external);
    let history = History::open(&root, Policy::default()).unwrap();
    assert!(history.protect(capture()).is_err());
    assert!(history.capture(capture()).is_err());
}

/// Payload, declared length, content identity and dangling references are independent corruption cases.
#[test]
fn corrupted_object_fields_never_produce_readable_version_bytes() {
    for alteration in [
        "UPDATE objects SET payload=x'00'",
        "UPDATE objects SET raw_size=raw_size+1",
        "UPDATE objects SET hash=zeroblob(32); UPDATE versions SET object=zeroblob(32)",
        "DELETE FROM objects",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("history");
        let history = History::open(&root, Policy::default()).unwrap();
        let version = history
            .capture(Capture {
                file: None,
                path: directory.path().join("note.txt"),
                workspace: None,
                contents: b"retained original".to_vec(),
                reason: Reason::Saved,
                at_ms: 1_000,
            })
            .unwrap()
            .version()
            .unwrap();
        drop(history);
        // An external damaged database bypasses foreign-key checks only while constructing this fixture.
        let external = rusqlite::Connection::open(root.join("history.sqlite3")).unwrap();
        external.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        external.execute_batch(alteration).unwrap();
        drop(external);
        let history = History::open(root, Policy::default()).unwrap();
        assert!(
            history.read(version).is_err(),
            "accepted damage: {alteration}"
        );
        assert_eq!(
            history.stats().unwrap().versions,
            1,
            "read must not silently clear damaged history"
        );
    }
}

/// An incomplete metadata schema is diagnosed before any replacement schema or empty history is created.
#[test]
fn missing_metadata_table_is_rejected_without_recreating_history() {
    let root = tempfile::tempdir().unwrap();
    drop(History::open(root.path(), Policy::default()).unwrap());
    let database = root.path().join("history.sqlite3");
    let external = rusqlite::Connection::open(&database).unwrap();
    external.execute_batch("DROP TABLE versions").unwrap();
    drop(external);
    let before = std::fs::read(&database).unwrap();
    assert!(History::open(root.path(), Policy::default()).is_err());
    assert_eq!(std::fs::read(database).unwrap(), before);
}

/// A native recovery ceiling does not grant permission to exceed the user's capture policy.
#[test]
fn restore_protection_obeys_the_user_file_limit() {
    let directory = tempfile::tempdir().unwrap();
    let history = History::open(
        directory.path().join("history"),
        Policy {
            max_file_bytes: 4,
            ..Policy::default()
        },
    )
    .unwrap();
    let result = history.protect(Capture {
        file: None,
        path: directory.path().join("note.txt"),
        workspace: None,
        contents: b"12345".to_vec(),
        reason: Reason::Saved,
        at_ms: 1_000,
    });
    assert!(matches!(
        result,
        Err(editor_history::Error::ProtectionSkipped(
            editor_history::SkipReason::TooLarge
        ))
    ));
}

/// An external writer lock produces a bounded error and leaves every previously acknowledged version readable.
#[test]
fn locked_database_does_not_damage_an_existing_version() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let history = History::open(&root, Policy::default()).unwrap();
    let path = directory.path().join("note.txt");
    let old = history
        .capture(Capture {
            file: None,
            path: path.clone(),
            workspace: None,
            contents: b"keep".to_vec(),
            reason: Reason::Saved,
            at_ms: 1_000,
        })
        .unwrap()
        .version()
        .unwrap();
    let external = rusqlite::Connection::open(root.join("history.sqlite3")).unwrap();
    external.execute_batch("BEGIN IMMEDIATE").unwrap();
    let start = std::time::Instant::now();
    assert!(
        history
            .capture(Capture {
                file: None,
                path,
                workspace: None,
                contents: b"new".to_vec(),
                reason: Reason::Saved,
                at_ms: 1_001
            })
            .is_err()
    );
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
    external.execute_batch("ROLLBACK").unwrap();
    assert_eq!(history.read(old).unwrap(), b"keep");
    assert_eq!(history.stats().unwrap().versions, 1);
}

/// Windows' read-only file attribute denies real storage writes without changing ACLs or filling the drive.
#[cfg(windows)]
#[test]
fn read_only_storage_preserves_the_acknowledged_version() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("history");
    let path = directory.path().join("note.txt");
    let capture = |contents: &[u8]| Capture {
        file: None,
        path: path.clone(),
        workspace: None,
        contents: contents.to_vec(),
        reason: Reason::Saved,
        at_ms: 1_000,
    };
    let history = History::open(&root, Policy::default()).unwrap();
    let version = history
        .capture(capture(b"keep"))
        .unwrap()
        .version()
        .unwrap();
    drop(history);
    let database = root.join("history.sqlite3");
    let original = std::fs::metadata(&database).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    std::fs::set_permissions(&database, readonly).unwrap();
    let denied = History::open(&root, Policy::default())
        .and_then(|history| history.capture(capture(b"denied")));
    // Restore the private fixture's original attribute before any assertion or temporary-directory cleanup.
    std::fs::set_permissions(&database, original).unwrap();
    assert!(
        denied.is_err(),
        "read-only storage unexpectedly accepted a write"
    );
    let history = History::open(root, Policy::default()).unwrap();
    assert_eq!(history.read(version).unwrap(), b"keep");
    assert_eq!(history.stats().unwrap().versions, 1);
}
