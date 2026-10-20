//! Independently built contribution packages must checkpoint and retire through ordinary lifecycle.
#[path = "support/demand.rs"]
mod demand_support;
use plugin_runtime::{DemandReason, Manager, Package, plugin_protocol::Environment};

/// A stateless contribution still owes the public snapshot contract; successful semantics cannot
/// excuse a guest that prevents updates or checked shutdown. No private host operation is added.
#[test]
#[ignore = "build ecosystem fixture via --plugin-package; set NANOBUG_ECOSYSTEM_PACKAGE"]
fn independently_packaged_ecosystem_checkpoints_disables_and_reenables() {
    let input = std::env::var_os("NANOBUG_ECOSYSTEM_PACKAGE").expect("ecosystem fixture ZIP");
    let package = Package::read(std::path::Path::new(&input)).unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    let id = package.manifest.id.clone();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    assert!(
        !manager.live.contains_key(&id),
        "installation is declarative and cold"
    );
    for enable in [false, true] {
        if enable {
            manager.enable(&id).unwrap();
        }
        demand_support::ready(
            &mut manager,
            &id,
            DemandReason::Service("language.contribution".into()),
        );
        let original = manager.availability(&id).unwrap();
        assert!(manager.live.contains_key(&id));
        manager
            .checkpoint()
            .expect("an admitted contributor must provide a valid lifecycle snapshot");
        let snapshot = manager.live.get_mut(&id).unwrap().snapshot().unwrap();
        assert_eq!(snapshot.schema, 1);
        assert_eq!(snapshot.data, "{}");
        manager.disable(&id).unwrap();
        assert!(!manager.live.contains_key(&id));
        assert_ne!(manager.availability(&id).ok().as_ref(), Some(&original));
    }
    manager.shutdown_checked().unwrap();
    assert!(manager.live.is_empty());
    assert!(
        std::fs::read_dir(workspace.path())
            .unwrap()
            .next()
            .is_none(),
        "contributor preparation and lifecycle never generate files in the project"
    );
}
