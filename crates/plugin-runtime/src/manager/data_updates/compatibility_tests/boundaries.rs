//! Unpublished real SDK stores check scoped choices before acquiring an installation transaction.
use super::*;
use plugin_protocol::settings::{EffectiveValue, Scope, Source};

/// Keep the real guest and resources byte-for-byte; public Package admission checks every variation.
fn changed(source: &Package, update: impl FnOnce(&mut Value)) -> Package {
    let mut files = source.files.clone();
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    update(&mut manifest);
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    Package::from_files(files).unwrap()
}

/// An unpublished Store shares only compiled instructions, never owner authority or mutable state.
fn candidate(
    manager: &Manager,
    package: &Package,
    code: &crate::instance::CompiledCode,
) -> Instance {
    let assets = manager
        .root
        .join("packages")
        .join(&package.manifest.id)
        .join(&package.digest);
    package.extract(&assets).unwrap();
    let mut instance = Instance::prepare_role_worker(
        code,
        package.component().unwrap(),
        &package.manifest,
        &package.manifest.permissions,
        manager.environment.clone(),
        manager
            .root
            .join("preflight-candidates")
            .join(&package.digest),
        assets,
        None,
        manager.host_resources.clone(),
    )
    .unwrap();
    let values = settings::SavedSettings::default()
        .resolve(&package.manifest, &manager.environment.workspace)
        .unwrap();
    instance
        .configure_settings(&package.manifest, values)
        .unwrap();
    instance
}

/// Match all current live consumers through the same pure broker rule used by commit preflight.
/// The full parent test additionally exercises the opaque token and public commit path.
fn validate(manager: &Manager, candidate: &Instance) -> Result<(), api::Failure> {
    let replacement = candidate.service_replacement();
    let broker = manager.plugin_services.lock().unwrap();
    for instance in manager.live.values() {
        instance.validate_service_replacement(&replacement, &broker)?;
    }
    Ok(())
}

/// Cover the policy boundaries before the original mandatory consumer is captured for commit.
pub(super) fn verify(
    manager: &mut Manager,
    code: &crate::instance::CompiledCode,
    package_to_replace: &Package,
    consumer: &Package,
) {
    let replacement = candidate(manager, package_to_replace, code);
    assert!(validate(manager, &replacement).is_err());
    let other = package("other-compatible", "1.0.0", true);
    publish(manager, &other, Some(code));
    manager
        .set_service_provider(
            api::InstanceScope::Workspace,
            Scope::Project,
            "example.echo",
            Some("other-compatible"),
        )
        .unwrap();
    assert!(
        validate(manager, &replacement).is_ok(),
        "a consumer selected to another compatible provider cannot lock this candidate"
    );

    let unavailable = package("other-incompatible", "2.0.0", true);
    publish(manager, &unavailable, Some(code));
    manager
        .set_service_provider(
            api::InstanceScope::Workspace,
            Scope::Project,
            "example.echo",
            Some("other-incompatible"),
        )
        .unwrap();
    assert!(
        validate(manager, &replacement).is_ok(),
        "an already unresolved mandatory dependency must not invent a new update restriction"
    );
    manager
        .set_service_provider(
            api::InstanceScope::Workspace,
            Scope::Project,
            "example.echo",
            Some("contract-provider"),
        )
        .unwrap();

    // Remove the mandatory purpose through its public lifecycle, then establish a genuine optional
    // consumer. The final mandatory owner/reference are captured only after restoring that purpose.
    manager.disable("mandatory-consumer").unwrap();
    let optional = changed(consumer, |manifest| {
        manifest["id"] = json!("optional-consumer");
        manifest["name"] = json!("optional-consumer");
        manifest["plugin_services"]["requires"]["example.echo"]["optional"] = json!(true);
    });
    publish(manager, &optional, Some(code));
    assert!(
        validate(manager, &replacement).is_ok(),
        "a live optional consumer must retain its documented fallback rather than lock an update"
    );
    publish(manager, consumer, Some(code));

    let compatible = changed(package_to_replace, |manifest| {
        manifest["plugin_services"]["provides"]["example.echo"]["version"] = json!("1.0.0");
        manifest["api"]["required"]["dependencies"] = json!(">=1.2,<2");
    });
    let gated = changed(&compatible, |manifest| {
        manifest["plugin_services"]["provides"]["example.echo"]["enabled_setting"] =
            json!("service.enabled");
        manifest["settings"]["service.enabled"] = json!({
            "title":"Enable fixture contract", "value_type":{"kind":"boolean"}, "default":false
        });
    });
    let mut gated_instance = candidate(manager, &gated, code);
    let validate = |candidate: &Instance| {
        let replacement = candidate.service_replacement();
        let broker = manager.plugin_services.lock().unwrap();
        manager.live["mandatory-consumer"].validate_service_replacement(&replacement, &broker)
    };
    assert!(
        validate(&gated_instance).is_err(),
        "a compatible but disabled candidate contribution is not a usable replacement"
    );
    let mut values = gated_instance.configuration.clone();
    values.insert(
        "service.enabled".into(),
        EffectiveValue {
            value: json!(true),
            source: Source::User,
        },
    );
    gated_instance
        .configure_settings(&gated.manifest, values)
        .unwrap();
    assert!(validate(&gated_instance).is_ok());

    let unsupported = changed(&compatible, |manifest| {
        manifest["plugin_services"]["provides"]["example.echo"]["platforms"] =
            json!([if crate::development::platform().starts_with("windows-") {
                "linux-x86_64"
            } else {
                "windows-x86_64"
            }]);
    });
    assert!(
        validate(&candidate(manager, &unsupported, code)).is_err(),
        "a compatible declaration for another platform cannot keep this consumer working"
    );
    assert!(
        validate(&candidate(manager, &compatible, code)).is_ok(),
        "a compatible ungated declaration must remain admissible"
    );
}
