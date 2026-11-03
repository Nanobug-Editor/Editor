//! Migration checks exercise real files, OS ownership and a readable historical version.

use super::*;
use editor_history::{Capture, History, Policy, Reason};

/// Create a complete fixture file and its parents without involving the user's actual stores.
fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

/// Copying an existing profile preserves settings, arbitrary plugin resources and real history data.
#[test]
fn portable_import_preserves_history_settings_and_plugin_data() {
    let temporary = tempfile::tempdir().unwrap();
    let old = temporary.path().join("old-profile");
    let old_history = temporary.path().join("old-history");
    let portable = temporary.path().join("installed");
    write(&portable, "Nanobug.exe", b"installed executable");
    write(&old, "Nanobug.exe", b"legacy conflicting file");
    write(&old, "shortcuts.json", b"old shortcuts");
    write(&old, "runtime-plugins/registry.json", b"registry");
    write(
        &old,
        "runtime-plugins/packages/example/gui/view.json",
        b"plugin resource",
    );
    write(
        &old,
        "runtime-plugins/data/application/example/private.bin",
        b"private bytes",
    );
    write(&old, "gui/endpoint.json", b"old transport secret");
    let history = History::open(&old_history, Policy::default()).unwrap();
    let version = history
        .capture(Capture {
            file: None,
            path: temporary.path().join("project/note.txt"),
            workspace: None,
            contents: "保留历史版本\r\n".as_bytes().to_vec(),
            reason: Reason::Saved,
            at_ms: 1_000,
        })
        .unwrap()
        .version()
        .unwrap();
    drop(history);
    let sources = LegacyLocations {
        profile: Some(old.clone()),
        history: Some(old_history.clone()),
    };
    prepare(&portable, Some(sources.clone())).unwrap();
    assert_eq!(
        fs::read(portable.join("Nanobug.exe")).unwrap(),
        b"installed executable"
    );
    assert!(!portable.join("data").exists());
    assert_eq!(
        fs::read(portable.join("shortcuts.json")).unwrap(),
        b"old shortcuts"
    );
    assert_eq!(
        fs::read(portable.join("runtime-plugins/data/application/example/private.bin")).unwrap(),
        b"private bytes"
    );
    assert_eq!(
        fs::read(portable.join("runtime-plugins/packages/example/gui/view.json")).unwrap(),
        b"plugin resource"
    );
    assert!(!portable.join("gui/endpoint.json").exists());
    let migrated = History::open(portable.join("history"), Policy::default()).unwrap();
    assert_eq!(
        migrated.read(version).unwrap(),
        "保留历史版本\r\n".as_bytes()
    );
    drop(migrated);
    assert_eq!(
        History::open(&old_history, Policy::default())
            .unwrap()
            .read(version)
            .unwrap(),
        "保留历史版本\r\n".as_bytes()
    );
    // A later uninstall or preference deletion must never be undone by the untouched backup.
    fs::remove_file(portable.join("shortcuts.json")).unwrap();
    prepare(&portable, Some(sources)).unwrap();
    assert!(!portable.join("shortcuts.json").exists());
    assert_eq!(
        fs::read(old.join("shortcuts.json")).unwrap(),
        b"old shortcuts"
    );
}

/// An existing new preference wins; a conflicting directory blocks completion and can be retried.
#[test]
fn portable_import_is_retryable_without_overwriting_new_data() {
    let temporary = tempfile::tempdir().unwrap();
    let old = temporary.path().join("old");
    let portable = temporary.path().join("new");
    write(&old, "shortcuts.json", b"old");
    write(&old, "runtime-plugins/data/value", b"kept");
    write(&portable, "shortcuts.json", b"new");
    write(&portable, "runtime-plugins", b"conflict");
    let sources = LegacyLocations {
        profile: Some(old.clone()),
        history: None,
    };
    assert!(prepare(&portable, Some(sources.clone())).is_err());
    assert!(!portable.join(COMPLETE).exists());
    assert_eq!(fs::read(portable.join("shortcuts.json")).unwrap(), b"new");
    fs::remove_file(portable.join("runtime-plugins")).unwrap();
    prepare(&portable, Some(sources)).unwrap();
    assert_eq!(
        fs::read(portable.join("runtime-plugins/data/value")).unwrap(),
        b"kept"
    );
    assert_eq!(fs::read(portable.join("shortcuts.json")).unwrap(), b"new");
}

/// A running legacy GUI or history writer prevents an inconsistent snapshot before any publication.
#[test]
fn portable_import_waits_for_legacy_owners_to_exit() {
    let temporary = tempfile::tempdir().unwrap();
    let old = temporary.path().join("old");
    let portable = temporary.path().join("new");
    write(&old, "gui/owner.lock", b"");
    write(&old, "shortcuts.json", b"kept");
    let lease = lock(&old.join("gui/owner.lock")).unwrap();
    let sources = LegacyLocations {
        profile: Some(old),
        history: None,
    };
    assert_eq!(
        prepare(&portable, Some(sources.clone()))
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    assert!(!portable.join("shortcuts.json").exists());
    assert!(!portable.join(COMPLETE).exists());
    drop(lease);
    prepare(&portable, Some(sources)).unwrap();
}

/// Explicit profiles start independently, and a blocked destination never redirects into user data.
#[test]
fn portable_isolation_and_unwritable_destination_do_not_fall_back() {
    let temporary = tempfile::tempdir().unwrap();
    let isolated = temporary.path().join("isolated");
    prepare(&isolated, None).unwrap();
    assert!(!isolated.join(COMPLETE).exists());
    let blocked = temporary.path().join("blocked");
    fs::write(&blocked, b"existing file").unwrap();
    assert!(prepare(&blocked, None).is_err());
    assert_eq!(fs::read(&blocked).unwrap(), b"existing file");
    let nested = isolated.join("data");
    assert!(
        prepare(
            &nested,
            Some(LegacyLocations {
                profile: Some(isolated),
                history: None
            })
        )
        .is_err()
    );
    assert!(!nested.join(COMPLETE).exists());
}

/// Existing history databases and plugin registries must not acquire a different store's journals.
#[test]
fn portable_import_keeps_existing_multifile_stores_intact() {
    let temporary = tempfile::tempdir().unwrap();
    let old = temporary.path().join("old");
    let old_history = temporary.path().join("old-history");
    let portable = temporary.path().join("new");
    write(&old, "runtime-plugins/registry.json", b"old registry");
    write(
        &old,
        "runtime-plugins/data-transactions/unfinished.json",
        b"old transaction",
    );
    write(&old_history, "history.sqlite3-wal", b"old WAL");
    write(&portable, "runtime-plugins/registry.json", b"new registry");
    write(&portable, "history/history.sqlite3", b"new database");
    prepare(
        &portable,
        Some(LegacyLocations {
            profile: Some(old),
            history: Some(old_history),
        }),
    )
    .unwrap();
    assert_eq!(
        fs::read(portable.join("runtime-plugins/registry.json")).unwrap(),
        b"new registry"
    );
    assert_eq!(
        fs::read(portable.join("history/history.sqlite3")).unwrap(),
        b"new database"
    );
    assert!(!portable.join("history/history.sqlite3-wal").exists());
    assert!(
        !portable
            .join("runtime-plugins/data-transactions/unfinished.json")
            .exists()
    );
}

/// A redirected plugin subtree cannot copy unrelated external files into the installation.
#[cfg(windows)]
#[test]
fn portable_import_rejects_redirected_entries() {
    let temporary = tempfile::tempdir().unwrap();
    let old = temporary.path().join("old");
    let outside = temporary.path().join("outside");
    let portable = temporary.path().join("new");
    fs::create_dir(&old).unwrap();
    write(&outside, "private.txt", b"outside");
    let status = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command",
            "New-Item -ItemType Junction -Path $env:NANOBUG_MIGRATION_LINK -Target $env:NANOBUG_MIGRATION_TARGET -ErrorAction Stop | Out-Null"])
        .env("NANOBUG_MIGRATION_LINK", old.join("redirect"))
        .env("NANOBUG_MIGRATION_TARGET", &outside).status().unwrap();
    assert!(status.success());
    assert!(
        prepare(
            &portable,
            Some(LegacyLocations {
                profile: Some(old),
                history: None
            })
        )
        .is_err()
    );
    assert!(!portable.join(COMPLETE).exists());
    assert_eq!(fs::read(outside.join("private.txt")).unwrap(), b"outside");
}
