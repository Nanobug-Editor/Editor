//! Real native deletion failures reconcile only files actually removed from the workspace.
#![cfg(windows)]

use super::*;

/// Git-ignored text is protected, while history's own excluded directories remain outside capture.
#[test]
fn history_delete_uses_its_own_excludes_instead_of_git_ignore() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("project");
    let folder = root.join("folder");
    std::fs::create_dir_all(folder.join("target")).unwrap();
    std::fs::write(folder.join(".gitignore"), "ignored.txt\n").unwrap();
    std::fs::write(folder.join("ignored.txt"), "protect this").unwrap();
    std::fs::write(folder.join("target/generated.txt"), "excluded").unwrap();
    let root = root.canonicalize().unwrap();
    let folder = root.join("folder");
    let history = History::open(
        directory.path().join("history"),
        editor_history::Policy::default(),
    )
    .unwrap();
    let guard = prepare(Some(&history), &root, &folder, &AtomicBool::new(false)).unwrap();
    remove(Some(&history), &root, &folder, guard).unwrap();
    let rows = history.list(&editor_history::Query::default()).unwrap();
    assert!(rows.iter().any(|row| row.path.ends_with("ignored.txt")
        && history.read(row.id).ok().as_deref() == Some(b"protect this")));
    assert!(!rows.iter().any(|row| row.path.ends_with("generated.txt")));
    assert!(!folder.exists());
}

/// A held Windows file causes real partial failure; every reported tombstone matches actual absence.
#[test]
fn history_partial_directory_deletion_matches_native_postconditions() {
    use std::os::windows::fs::OpenOptionsExt;
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("project");
    let folder = root.join("folder");
    std::fs::create_dir_all(&folder).unwrap();
    for index in 0..12 {
        std::fs::write(folder.join(format!("a{index:02}.txt")), "first").unwrap();
    }
    let locked = folder.join("z-locked.txt");
    std::fs::write(&locked, "locked").unwrap();
    let root = root.canonicalize().unwrap();
    let folder = root.join("folder");
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&locked)
        .unwrap();
    let history = History::open(
        directory.path().join("history"),
        editor_history::Policy::default(),
    )
    .unwrap();
    let guard = prepare(Some(&history), &root, &folder, &AtomicBool::new(false)).unwrap();
    let identities = history.bindings_under(&folder).unwrap();
    assert!(remove(Some(&history), &root, &folder, guard).is_err());
    assert!(locked.exists());
    let mut removed = 0;
    for (file, path) in identities {
        let present = path.exists();
        assert_eq!(
            history.location(file).unwrap().present,
            present,
            "{}",
            path.display()
        );
        removed += usize::from(!present);
    }
    assert!(
        removed > 0,
        "the native fixture did not exercise a partial deletion"
    );
    drop(held);
}
