//! Native history recovery never recreates a missing project root.

use platform_windows::history::{HistoryPaths, prepare_recovery_parent};

/// Isolated profiles retain both settings and history without consulting the ambient user profile.
#[test]
fn explicit_history_profile_and_recovery_parent_are_isolated() {
    let directory = tempfile::tempdir().unwrap();
    let profile = directory.path().join("profile");
    let paths = HistoryPaths::for_profile(Some(&profile)).unwrap();
    assert_eq!(paths.root, profile.join("history"));
    assert_eq!(paths.settings, profile.join("history.json"));
    let workspace = directory.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let workspace = workspace.canonicalize().unwrap();
    paths.validate_workspace(&workspace).unwrap();
    let inside = HistoryPaths::for_profile(Some(&workspace.join("profile"))).unwrap();
    assert!(inside.validate_workspace(&workspace).is_err());
    assert!(!workspace.join("profile").exists());
    let target = workspace.join("missing").join("note.txt");
    prepare_recovery_parent(&workspace, &target).unwrap();
    assert!(target.parent().unwrap().is_dir());
    assert!(!target.exists());
    let gone = directory.path().join("gone");
    assert!(prepare_recovery_parent(&gone, &gone.join("note.txt")).is_err());
    assert!(!gone.exists());
    assert!(
        prepare_recovery_parent(&workspace, &workspace.join("..").join("outside.txt")).is_err()
    );
}
