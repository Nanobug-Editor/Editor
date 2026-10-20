//! A valid resource-only update cannot remove a live consumer's currently selected guest contract.
use super::*;

/// Admission validates a genuine TOML resource package with no component, service or command.
/// The old SDK bytes are not present, so this cannot accidentally exercise WASM preparation.
fn resource_package(source: &Package) -> Package {
    let id = &source.manifest.id;
    let manifest = json!({
        "id":id, "name":"Resource-only replacement", "version":"2.0.0", "protocol":7,
        "api":{"base":"^1","required":{},"optional":{}},
        "contributions":"plugin.toml", "permissions":[], "panels":[], "commands":[], "storage_limit":1024
    });
    Package::from_files(BTreeMap::from([
        ("manifest.json".into(), serde_json::to_vec(&manifest).unwrap()),
        (
            "plugin.toml".into(),
            format!(
                "[plugin]\nid=\"{id}\"\nname=\"Resource-only replacement\"\nversion=\"2.0.0\"\nhost_version=\"^0.1\"\n"
            )
            .into_bytes(),
        ),
    ]))
    .unwrap()
}

/// Public synchronous installation must retain active mandatory owners and references on refusal.
/// After the mandatory purpose is disabled, the same valid resource package remains installable.
#[test]
#[ignore = "real frozen capability-example-0.19.0 SDK component; no native process"]
fn resource_only_update_cannot_remove_a_resolved_mandatory_contract() {
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
    assert!(
        old.manifest.data_format.is_none(),
        "isolate the contract refusal from data migration"
    );
    let code = publish(&mut manager, &old, None);
    let consumer = package("mandatory-consumer", "1.0.0", false);
    publish(&mut manager, &consumer, Some(&code));
    let old_owner = manager.instance_id("contract-provider").unwrap().to_owned();
    let consumer_owner = manager
        .instance_id("mandatory-consumer")
        .unwrap()
        .to_owned();
    let caller = manager.live["mandatory-consumer"]
        .service_provider()
        .unwrap()
        .caller;
    let reference = manager
        .plugin_services
        .lock()
        .unwrap()
        .resolve(
            &caller,
            "example.echo",
            &consumer.manifest.plugin_services.requires["example.echo"],
        )
        .unwrap();
    assert_eq!(reference.owner(), Some(old_owner.as_str()));

    let candidate = resource_package(&old);
    assert!(candidate.component().is_none());
    assert!(candidate.manifest.plugin_services.provides.is_empty());
    let result = manager.install(&candidate, candidate.manifest.permissions.clone());
    assert!(
        result.is_err(),
        "resource-only update must not remove a resolved mandatory contract"
    );
    let rejected = result.unwrap_err();
    let failure = rejected.downcast_ref::<api::Failure>().unwrap();
    assert_eq!(failure.code, api::ErrorCode::CapabilityUnavailable);
    assert!(failure.message.contains("example.echo"), "{rejected:#}");
    assert_eq!(manager.installed["contract-provider"].digest, old.digest);
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

    // This is a compatibility guard, not a ban on resource-only replacements. No candidate guest
    // exists to activate; only the retired old component could have supplied executable contracts.
    manager.disable("mandatory-consumer").unwrap();
    manager
        .install(&candidate, candidate.manifest.permissions.clone())
        .unwrap();
    assert_eq!(
        manager.installed["contract-provider"].digest,
        candidate.digest
    );
    assert!(manager.instance_id("contract-provider").is_none());
    assert!(!manager.live.contains_key("contract-provider"));
    manager.shutdown_checked().unwrap();
}
