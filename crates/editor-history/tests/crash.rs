//! Parent-owned process interruption verifies acknowledged persistence and transaction rollback.

use editor_history::{Capture, History, Policy, Query, Reason};

/// An acknowledged version survives an immediate process exit; uncommitted bytes stay invisible.
#[test]
fn interrupted_process_keeps_only_complete_committed_versions() {
    for stage in ["acknowledged", "uncommitted"] {
        let directory = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "history_crash_child", "--ignored", "--nocapture"])
            .env("NANOBUG_HISTORY_CRASH_ROOT", directory.path())
            .env("NANOBUG_HISTORY_CRASH_STAGE", stage)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(23),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let history = History::open(directory.path().join("history"), Policy::default()).unwrap();
        let rows = history.list(&Query::default()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(history.read(rows[0].id).unwrap(), b"acknowledged");
        assert_eq!(history.stats().unwrap().objects, 1);
        let external =
            rusqlite::Connection::open(directory.path().join("history/history.sqlite3")).unwrap();
        assert_eq!(
            external
                .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
        assert!(
            !external
                .prepare("PRAGMA foreign_key_check")
                .unwrap()
                .query([])
                .unwrap()
                .next()
                .unwrap()
                .is_some()
        );
    }
}

/// Exit bypasses all destructors, modelling process termination rather than orderly connection close.
#[test]
#[ignore = "spawn only from the parent-owned crash test with a private temporary root"]
fn history_crash_child() {
    let root = std::path::PathBuf::from(std::env::var_os("NANOBUG_HISTORY_CRASH_ROOT").unwrap());
    let history = History::open(root.join("history"), Policy::default()).unwrap();
    history
        .capture(Capture {
            file: None,
            path: root.join("note.txt"),
            workspace: None,
            contents: b"acknowledged".to_vec(),
            reason: Reason::Saved,
            at_ms: 1_000,
        })
        .unwrap();
    if std::env::var("NANOBUG_HISTORY_CRASH_STAGE").as_deref() == Ok("uncommitted") {
        let external = rusqlite::Connection::open(root.join("history/history.sqlite3")).unwrap();
        external.execute_batch("BEGIN IMMEDIATE; INSERT INTO objects(hash,raw_size,payload) VALUES(zeroblob(32),1,x'00');").unwrap();
        // Deliberately retain the open writer until the process ends; no rollback destructor runs.
        std::mem::forget(external);
    }
    std::mem::forget(history);
    std::process::exit(23);
}
