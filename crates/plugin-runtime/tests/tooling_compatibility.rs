//! Original released ZIP bytes and their old SDK identities are never replaced by newly built consumers.
#[path = "support/demand.rs"]
mod demand;
use plugin_runtime::{DemandReason, Manager, Package, plugin_protocol::Environment, validation};
use sha2::{Digest, Sha256};

/// Run original archives through production admission, actual activation and final owner retirement.
#[test]
#[ignore = "explicit original frozen WASM inputs; verify packages/inputs.json and do not rebuild old archives"]
fn tooling_original_zip_matrix_preserves_admission_activation_and_retirement() {
    let ledger: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/packages/inputs.json")).unwrap();
    for name in [
        "capability-example-0.19.0.zip",
        "cloud-translation-0.1.1.zip",
        "code-host-query-0.1.1.zip",
    ] {
        let input = ledger["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|input| input["archive"] == name)
            .expect("every original byte input must be pinned");
        let bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/packages")
                .join(name),
        )
        .unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            input["sha256"].as_str().unwrap(),
            "original archives cannot be silently rebuilt"
        );
        let report = validation::check_bytes(&bytes, Default::default());
        assert!(report.passed, "{name}: {:?}", report.diagnostics);
        let package = Package::from_bytes(&bytes).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let mut manager = Manager::open(
            directory.path().join("runtime"),
            Environment {
                workspace: directory.path().display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        let panel = package
            .manifest
            .panels
            .first()
            .expect("original UI consumer has a declared panel");
        demand::ready(
            &mut manager,
            &package.manifest.id,
            DemandReason::View(panel.id.clone()),
        );
        assert!(!manager.live[&package.manifest.id].views.is_empty());
        let observations = manager.runtime_observations();
        manager.shutdown_checked().unwrap();
        let snapshot = observations.snapshot(Some(&package.manifest.id));
        assert!(
            snapshot
                .owners
                .iter()
                .filter(|owner| owner.owner.resource_owner)
                .all(|owner| owner
                    .resources
                    .as_ref()
                    .is_some_and(|resources| resources.retired
                        && resources.handles == 0
                        && resources.processes == 0))
        );
    }
}
