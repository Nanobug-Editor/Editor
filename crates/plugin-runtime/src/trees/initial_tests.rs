//! Initial tree defaults preserve live pages and yield permanently to native interaction.
use super::*;

/// Allocate the same resource/lifetime as production without granting any command or file access.
fn resource() -> TreeResource {
    TreeResource::new(
        ResourceHandle {
            instance: "owner".into(),
            scope: "workspace".into(),
            resource: 1,
        },
        EditorAuthority::new(Arc::new(AtomicBool::new(true)), None),
        tree::Options::default(),
    )
}

/// Initial positioning leaves page/epoch authority intact and survives suspension until consumed.
#[test]
fn initial_selection_preserves_pages_and_is_one_shot() {
    let tree = resource();
    let pending = tree.request(None, false).unwrap().unwrap();
    let original = tree.snapshot();
    tree.initialize(vec!["folder".into(), "leaf".into()])
        .unwrap();
    assert_eq!(tree.snapshot().epoch, original.epoch);
    assert!(tree.snapshot().branches[&None].loading);
    tree.reply(
        &pending,
        Ok(tree::Page {
            nodes: vec![tree::Node::new("folder", "Folder", true)],
            next: None,
        }),
        Default::default(),
    )
    .unwrap();
    tree.suspend();
    assert_eq!(
        tree.snapshot().initial_selection.as_ref().unwrap(),
        &["folder", "leaf"]
    );
    tree.dismiss_initial_selection();
    tree.refresh().unwrap();
    tree.initialize(vec!["another".into()]).unwrap();
    assert!(tree.snapshot().initial_selection.is_none());
}

/// Input arriving before initialization also wins; malformed paths and revoked owners are rejected.
#[test]
fn initial_selection_yields_to_early_input_and_validates_path() {
    let tree = resource();
    for path in [
        vec![],
        vec!["@reserved".into()],
        vec!["same".into(), "same".into()],
        (0..65).map(|i| format!("n{i}")).collect(),
    ] {
        assert_eq!(
            tree.initialize(path).unwrap_err().code,
            ErrorCode::InvalidRequest
        );
    }
    tree.dismiss_initial_selection();
    tree.initialize(vec!["late".into()]).unwrap();
    assert!(tree.snapshot().initial_selection.is_none());
    tree.revoke();
    assert!(tree.initialize(vec!["closed".into()]).is_err());
}
