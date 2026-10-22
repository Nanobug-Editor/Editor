//! Persisted startup policy covers defaults, target precedence and safe failure without user data.

use super::*;

/// Existing profiles without a new setting retain the requested enabled default.
#[test]
fn startup_restore_defaults_on_without_creating_profile_files() {
    let profile = tempfile::tempdir().unwrap();
    assert!(
        Preferences::load(profile.path())
            .unwrap()
            .restore_last_project
    );
    assert!(target(profile.path(), None).is_none());
    assert_eq!(std::fs::read_dir(profile.path()).unwrap().count(), 0);
    std::fs::write(profile.path().join("startup.json"), b"{}").unwrap();
    assert!(
        Preferences::load(profile.path())
            .unwrap()
            .restore_last_project
    );
}

/// Disabling restoration survives project closure and never overrides a requested file or folder.
#[test]
fn startup_restore_switch_persists_and_explicit_targets_win() {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    remember_project(profile.path(), &root).unwrap();
    assert_eq!(target(profile.path(), None), Some(root.clone()));
    let explicit = project.path().join("explicit.txt");
    assert_eq!(
        target(profile.path(), Some(explicit.clone())),
        Some(explicit.clone())
    );
    let mut preferences = Preferences::load(profile.path()).unwrap();
    preferences.restore_last_project = false;
    preferences.save(profile.path()).unwrap();
    remember_project(profile.path(), &root).unwrap();
    assert!(target(profile.path(), None).is_none());
    assert_eq!(
        target(profile.path(), Some(explicit.clone())),
        Some(explicit)
    );
    assert!(
        !Preferences::load(profile.path())
            .unwrap()
            .restore_last_project
    );
}

/// Missing saved roots and other profiles stay empty instead of borrowing another recent project.
#[test]
fn startup_restore_rejects_missing_or_relative_roots_and_isolates_profiles() {
    let profile = tempfile::tempdir().unwrap();
    let isolated = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    remember_project(profile.path(), project.path()).unwrap();
    assert!(target(isolated.path(), None).is_none());
    project.close().unwrap();
    assert!(target(profile.path(), None).is_none());
    remember_project(profile.path(), Path::new(".")).unwrap();
    assert!(target(profile.path(), None).is_none());
}

/// Malformed preferences cannot be silently replaced by exit bookkeeping or default settings.
#[test]
fn startup_restore_protects_damaged_preferences() {
    let profile = tempfile::tempdir().unwrap();
    let path = profile.path().join("startup.json");
    let original = b"{broken startup settings";
    std::fs::write(&path, original).unwrap();
    assert!(Preferences::load(profile.path()).is_err());
    assert!(remember_project(profile.path(), profile.path()).is_err());
    assert!(target(profile.path(), None).is_none());
    assert_eq!(std::fs::read(path).unwrap(), original);
}
