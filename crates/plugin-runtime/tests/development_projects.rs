//! Frozen SDK packages and explicit external CLI builds enter the public development-directory boundary.
/// First real consumption uses the same captured demand/receipt as native host callers.
#[path = "support/demand.rs"]
mod demand;
/// Shared checked-in inputs preserve real guest code after the example projects are retired.
#[path = "support/packages.rs"]
mod test_packages;
use plugin_runtime::{
    DemandReason, Manager, Package, development,
    plugin_protocol::{Environment, api},
};
use std::path::PathBuf;

/// Explicit CLI candidates remain supported; otherwise stage the fixed guest through the same public builder seam.
fn candidate() -> Package {
    let temporary = tempfile::tempdir().unwrap();
    let directory = std::env::var_os("NANOBUG_DEVELOPMENT_CANDIDATE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            // The pinned 0.4 guest preserves community reload/state assertions without a retired source tree.
            development::stage(&test_packages::package("example-0.4.0"), temporary.path()).unwrap()
        });
    development::read_directory(&directory).expect("read the admitted directory candidate")
}
/// Successful/failed replacement preserves logical state, private data and installed identity.
#[test]
#[ignore = "opt-in frozen SDK/WASM directory reload; external CLI candidates may use NANOBUG_DEVELOPMENT_CANDIDATE"]
fn real_directory_reload_preserves_state_and_rolls_back_failed_component() {
    let package = candidate();
    let id = package.manifest.id.clone();
    let view = package
        .manifest
        .panels
        .first()
        .expect("the independently built example must declare a native view")
        .id
        .clone();
    let data = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(data.path().into(), Environment::default()).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    assert!(!manager.live.contains_key(&id));
    // Static discovery stays cold; consume the real view through its original availability/receipt.
    demand::ready(&mut manager, &id, DemandReason::View(view.clone()));
    manager
        .event(
            &id,
            None,
            api::Notification::Command {
                id: "increment".into(),
                context: None,
                arguments: None,
            },
        )
        .unwrap();
    let state = manager.live.get_mut(&id).unwrap().snapshot().unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&state.data).unwrap()[0],
        1
    );
    let mut files = package.files.clone();
    files.insert("README.md".into(), b"Changed resource candidate".to_vec());
    let updated = Package::from_files(files.clone()).unwrap();
    manager
        .install(&updated, updated.manifest.permissions.clone())
        .unwrap();
    assert_eq!(
        manager.live.get_mut(&id).unwrap().snapshot().unwrap().data,
        state.data
    );
    assert_eq!(manager.installed[&id].digest, updated.digest);
    // A component header passes package admission; an incomplete component must still fail
    // compilation/instantiation without replacing the current installation.
    files.insert(
        package.manifest.component.clone().unwrap(),
        b"\0asm\x0d\0\x01\0".to_vec(),
    );
    let invalid = Package::from_files(files).unwrap();
    assert!(
        manager
            .install(&invalid, invalid.manifest.permissions.clone())
            .is_err()
    );
    assert_eq!(manager.installed[&id].digest, updated.digest);
    assert_eq!(
        manager.live.get_mut(&id).unwrap().snapshot().unwrap().data,
        state.data
    );
    manager.shutdown();
    drop(manager); // Release the exclusive private-data lease before reopening this profile.
    let mut reopened = Manager::open(data.path().into(), Environment::default()).unwrap();
    assert!(!reopened.live.contains_key(&id));
    // Reopening restores logical installation/snapshot, then this actual view demands the new owner.
    demand::ready(&mut reopened, &id, DemandReason::View(view));
    assert_eq!(
        reopened.live.get_mut(&id).unwrap().snapshot().unwrap().data,
        state.data
    );
    // An ordinary profile selected independently never receives these installation records.
    let ordinary = tempfile::tempdir().unwrap();
    assert!(
        Manager::open(ordinary.path().into(), Environment::default())
            .unwrap()
            .installed
            .is_empty()
    );
}
/// Rebuilding cannot turn a new manifest permission into inherited consent.
#[test]
#[ignore = "opt-in frozen example-0.4.0 SDK directory; external CLI candidates may set NANOBUG_DEVELOPMENT_CANDIDATE"]
fn real_directory_new_permissions_require_confirmation() {
    let package = candidate();
    let id = package.manifest.id.clone();
    let view = package
        .manifest
        .panels
        .first()
        .expect("the independently built example must declare a native view")
        .id
        .clone();
    let root = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(root.path().into(), Environment::default()).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    assert!(!manager.live.contains_key(&id));
    // A rejected permission upgrade must preserve an already demanded owner, not only dormant metadata.
    demand::ready(&mut manager, &id, DemandReason::View(view));
    let mut files = package.files.clone();
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["permissions"] = serde_json::json!(["storage"]);
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let changed = Package::from_files(files).unwrap();
    assert!(
        manager
            .install(&changed, package.manifest.permissions)
            .is_err()
    );
    assert_eq!(manager.installed[&id].digest, package.digest);
    assert!(manager.live.contains_key(&id));
}
