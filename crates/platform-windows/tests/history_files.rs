//! Native publication protects a history restoration from a competing file at the same path.

use editor_core::DocumentSession;
use platform_windows::PreparedFileStore;

/// Preparation may be asynchronous, but create-only publication cannot overwrite a later claimant.
#[test]
fn prepared_history_recovery_does_not_overwrite_a_new_file() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let path = root.join("recovered.txt");
    let prepared = PreparedFileStore::prepare_new(root, path.clone(), "historical".into()).unwrap();
    assert!(!path.exists());
    std::fs::write(&path, "new file").unwrap();
    let mut session =
        DocumentSession::from_file_contents(path.clone(), "historical".into()).session;
    session.note_edit();
    assert!(session.save(&prepared, prepared.contents()).is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "new file");
    assert!(session.is_dirty());
}
