//! Real SDK stores and the current broker expose incompatible provider updates before cutover.
use super::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
mod boundaries;
mod resource_only;

/// Retain the immutable S1 input; only declarations change, never the guest's compiled behavior.
fn package(id: &str, version: &str, provides: bool) -> Package {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/packages/capability-example-0.19.0.zip");
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "bdfc5d3327b5b7a4f3d25ba1ae95e2a6a47b62634fb040e17fb4430c9aa46773"
    );
    let mut files = Package::from_bytes(&bytes).unwrap().files;
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["id"] = json!(id);
    manifest["name"] = json!(id);
    manifest["version"] = json!(version);
    manifest["settings_hook"] = json!(false);
    manifest["api"]["required"]["plugin.services"] = json!("^1");
    manifest["permissions"]
        .as_array_mut()
        .unwrap()
        .push(json!("services.call"));
    let methods = json!({"echo":{
        "parameters":{"type":"string","max_bytes":2048},
        "result":{"type":"string","max_bytes":4096}
    }});
    manifest["plugin_services"] = if provides {
        json!({"provides":{"example.echo":{"version":version,"methods":methods}}})
    } else {
        json!({"requires":{"example.echo":{"version":"^1","methods":methods}}})
    };
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    files.insert("service-label.txt".into(), id.as_bytes().to_vec());
    Package::from_files(files).unwrap()
}

/// Establish live owners through real isolation/activation while reusing immutable code only.
/// This broker-level test skips repeated installation preparation, not dispatch or cutover checks.
fn publish(
    manager: &mut Manager,
    package: &Package,
    reused: Option<&crate::instance::CompiledCode>,
) -> crate::instance::CompiledCode {
    let manifest = &package.manifest;
    let assets = manager
        .root
        .join("packages")
        .join(&manifest.id)
        .join(&package.digest);
    package.extract(&assets).unwrap();
    let data = manager.instance_data_directory(manifest);
    let mut instance = match reused {
        Some(code) => Instance::prepare_role_worker(
            code,
            package.component().unwrap(),
            manifest,
            &manifest.permissions,
            manager.environment.clone(),
            data,
            assets,
            None,
            manager.host_resources.clone(),
        ),
        None => Instance::prepare_with_resources(
            manager.engine.as_ref().unwrap(),
            package.component().unwrap(),
            manifest,
            &manifest.permissions,
            manager.environment.clone(),
            data,
            assets,
            None,
            manager.host_resources.clone(),
        ),
    }
    .unwrap();
    instance
        .connect_services(manager.plugin_services.clone())
        .unwrap();
    instance.activate().unwrap();
    instance.commit_data().unwrap();
    instance.commit_activation();
    let code = instance.compiled_code();
    manager.installed.insert(
        manifest.id.clone(),
        Installed {
            manifest: manifest.clone(),
            digest: package.digest.clone(),
            grants: manifest.permissions.clone(),
            enabled: true,
            project_enabled: BTreeSet::new(),
            replaced_by: None,
            retired_ui_contract: false,
            global_enabled: None,
            error: None,
        },
    );
    manager.live.insert(manifest.id.clone(), instance);
    manager.save_registry().unwrap();
    manager.refresh_services();
    code
}

/// Updating a provider must preserve a currently resolved mandatory consumer's contract and owners.
/// A valid candidate with no requirements isolates the reverse direction from its own dependencies.
#[test]
#[ignore = "real frozen capability-example-0.19.0 SDK component; opt-in compilation, no native process"]
fn replacement_contracts_preserve_resolved_mandatory_consumers() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = directory.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let mut manager = Manager::open(
        directory.path().join("plugins"),
        Environment {
            workspace: workspace.display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager.acquire_data_owner().unwrap();
    manager.engine = Some(Instance::engine().unwrap());
    let old = package("contract-provider", "1.0.0", true);
    let code = publish(&mut manager, &old, None);
    let consumer = package("mandatory-consumer", "1.0.0", false);
    publish(&mut manager, &consumer, Some(&code));
    // Preparation executes only the new isolated guest. Its valid exports must not replace the
    // broker's version-one publication or cancel either original incarnation on rejection.
    let candidate = package("contract-provider", "2.0.0", true);
    // User choices and other installations precede this candidate's exclusive data transaction.
    // The boundary probes reuse real isolated candidate code, without holding a commit token.
    boundaries::verify(&mut manager, &code, &candidate, &consumer);
    let control = InstallControl::default();
    let prepared = manager
        .prepare_installation(&candidate, candidate.manifest.permissions.clone(), &control)
        .unwrap();
    // Boundary setup deliberately changes user choices and consumer purposes. Capture the final
    // live identities after that setup; rejecting the actual candidate must preserve all of them.
    let old_owner = manager.instance_id("contract-provider").unwrap().to_owned();
    let consumer_owner = manager
        .instance_id("mandatory-consumer")
        .unwrap()
        .to_owned();
    let caller = manager.live["mandatory-consumer"]
        .service_provider()
        .unwrap()
        .caller;
    let dependency = &consumer.manifest.plugin_services.requires["example.echo"];
    let reference = manager
        .plugin_services
        .lock()
        .unwrap()
        .resolve(&caller, "example.echo", dependency)
        .unwrap();
    assert_eq!(reference.owner(), Some(old_owner.as_str()));

    let result = manager.commit_installation(prepared, &control);
    assert!(
        result.is_err(),
        "incompatible provider exports must be rejected before cutover"
    );
    let rejected = result.unwrap_err();
    let failure = rejected.downcast_ref::<api::Failure>().unwrap();
    assert_eq!(failure.code, api::ErrorCode::CapabilityUnavailable);
    assert!(failure.message.contains("example.echo"), "{rejected:#}");
    assert_eq!(
        manager.instance_id("contract-provider"),
        Some(old_owner.as_str())
    );
    assert_eq!(
        manager.instance_id("mandatory-consumer"),
        Some(consumer_owner.as_str())
    );
    manager
        .plugin_services
        .lock()
        .unwrap()
        .validate_reference(&caller, &reference)
        .unwrap();
    manager.shutdown_checked().unwrap();
}
