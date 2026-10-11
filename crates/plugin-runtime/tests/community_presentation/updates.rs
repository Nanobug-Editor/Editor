//! Real tree ownership verifies final snapshots and fresh-guest rollback through public Manager updates.
use super::*;
use plugin_runtime::InstallControl;

/// A live tree remains usable while its logical snapshot is captured for an ordinary package cutover.
/// The real guest republishes its scene with that snapshot, so retirement cannot precede validation.
#[test]
#[ignore = "package plugins/database-tree through the current host --plugin-package first"]
fn community_database_tree_update_captures_live_snapshot_before_retirement() {
    let runtime = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let original = provider("database-tree");
    let mut manager = Manager::open(
        runtime.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&original, original.manifest.permissions.clone())
        .unwrap();
    let old_identity = manager.instance_id("database-tree").unwrap().to_owned();
    let old_tree = manager.live["database-tree"]
        .tree_resources()
        .pop()
        .unwrap();
    assert!(old_tree.is_live());

    // Only the package version changes; both generations execute the independently built SDK guest.
    let mut manifest = original.manifest.clone();
    manifest.version = "0.1.2".into();
    let candidate = repack(&original, manifest);
    let control = InstallControl::default();
    let prepared = manager
        .prepare_installation(&candidate, candidate.manifest.permissions.clone(), &control)
        .unwrap();
    assert!(old_tree.is_live(), "preparation must retain the old tree");
    manager
        .commit_installation(prepared, &control)
        .expect("cutover must capture the current guest snapshot before retiring its tree");
    assert_ne!(
        manager.instance_id("database-tree"),
        Some(old_identity.as_str())
    );
    assert!(
        !old_tree.is_live(),
        "successful cutover revokes the old tree"
    );
    let new_tree = manager.live["database-tree"]
        .tree_resources()
        .pop()
        .unwrap();
    assert!(new_tree.is_live());
    assert_ne!(new_tree.handle(), old_tree.handle());
    load(&mut manager, &new_tree, None, false);
    assert!(new_tree.snapshot().nodes.contains_key("main"));
}

/// A cancelled cutover restores logical state in a fresh guest, never reuses its retired tree handle.
#[test]
#[ignore = "package plugins/database-tree through the current host --plugin-package first"]
fn community_database_tree_failed_cutover_restores_fresh_owned_tree() {
    let runtime = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let original = provider("database-tree");
    let mut manager = Manager::open(
        runtime.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&original, original.manifest.permissions.clone())
        .unwrap();
    let old_identity = manager.instance_id("database-tree").unwrap().to_owned();
    let old_tree = manager.live["database-tree"]
        .tree_resources()
        .pop()
        .unwrap();
    let mut manifest = original.manifest.clone();
    manifest.version = "0.1.2".into();
    let candidate = repack(&original, manifest);

    // Cancel at the public progress boundary after the old owner stops, not during candidate preparation.
    let control_slot = std::sync::Arc::new(std::sync::Mutex::new(None::<InstallControl>));
    let cancelling_slot = control_slot.clone();
    let control = InstallControl::new(move |stage| {
        if stage == plugin_runtime::InstallStage::Migrating {
            cancelling_slot.lock().unwrap().as_ref().unwrap().cancel();
        }
    });
    *control_slot.lock().unwrap() = Some(control.clone());
    let prepared = manager
        .prepare_installation(&candidate, candidate.manifest.permissions.clone(), &control)
        .unwrap();
    let error = manager.commit_installation(prepared, &control).unwrap_err();
    control_slot.lock().unwrap().take();
    assert!(format!("{error:#}").contains("cancelled"));
    assert_eq!(manager.installed["database-tree"].digest, original.digest);
    assert!(!old_tree.is_live());
    assert_ne!(
        manager.instance_id("database-tree"),
        Some(old_identity.as_str())
    );
    let restored_tree = manager
        .live
        .get("database-tree")
        .expect("cancelled cutover must restore a usable old-version guest")
        .tree_resources()
        .pop()
        .unwrap();
    assert!(restored_tree.is_live());
    assert_ne!(restored_tree.handle(), old_tree.handle());
    load(&mut manager, &restored_tree, None, false);
    assert!(restored_tree.snapshot().nodes.contains_key("main"));
}
