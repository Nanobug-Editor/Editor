//! Public package-manager admission retains legacy preferences without touching retired executable formats.
use plugin_runtime::{Manager, plugin_protocol::Environment};
use serde_json::json;

/// Immutable development bytes share the installed ZIP validator without starting a WASM instance.
fn resource_compatibility_files(
    manifest: serde_json::Value,
    host_range: &str,
) -> std::collections::BTreeMap<String, Vec<u8>> {
    let id = manifest["id"].as_str().unwrap();
    let version = manifest["version"].as_str().unwrap();
    let declaration = format!(
        "[plugin]\nid=\"{id}\"\nname=\"Host compatibility\"\nversion=\"{version}\"\nhost_version=\"{host_range}\"\n"
    );
    [
        (
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        ),
        ("plugin.toml".into(), declaration.into_bytes()),
    ]
    .into()
}

/// Author-declared host ranges must be checked by local package admission, not only a marketplace.
#[test]
fn local_package_rejects_an_unsupported_contribution_host_range() {
    let manifest = json!({"id":"host-floor-check","name":"Host floor","version":"1.0.0",
        "protocol":7,"api":{"base":"^1"},"contributions":"plugin.toml","storage_limit":1024});
    let files = resource_compatibility_files(manifest, ">=2.0.0");
    let error = plugin_runtime::Package::from_files(files)
        .err()
        .expect("a future host requirement cannot be admitted by this host");
    assert!(error.to_string().contains("host"), "{error:#}");
}

/// Release floors and optional capability fallback stay independent of the current wire protocol.
#[test]
fn package_host_floor_and_optional_fallback_do_not_require_empty_wasm() {
    for minimum in [
        None,
        Some("0.0.1"),
        Some(env!("CARGO_PKG_VERSION")),
        Some("0.1.0+sdk-build"),
    ] {
        let mut manifest = json!({"id":"resource-floor","name":"Resource floor","version":"1.0.0",
            "protocol":7,"api":{"base":"^1","optional":{"future.optional":"^2"}},
            "contributions":"plugin.toml","storage_limit":1024});
        if let Some(minimum) = minimum {
            manifest["minimum_host"] = json!(minimum);
        }
        let package =
            plugin_runtime::Package::from_files(resource_compatibility_files(manifest, ">=0.1.0"))
                .unwrap();
        assert!(package.component().is_none());
        assert!(package.manifest.activation.is_default());
        let root = tempfile::tempdir().unwrap();
        let mut manager = Manager::open(root.path().into(), Environment::default()).unwrap();
        manager.install(&package, Default::default()).unwrap();
        assert!(manager.live.is_empty());
        assert!(
            manager
                .published_entries()
                .iter()
                .any(|entry| entry.enabled && entry.manifest.id == "resource-floor")
        );
    }
    for minimum in ["2.0.0", "^0.1", "", "0.1"] {
        let manifest = json!({"id":"resource-floor","name":"Resource floor","version":"1.0.0",
            "protocol":7,"api":{"base":"^1"},"contributions":"plugin.toml",
            "minimum_host":minimum,"storage_limit":1024});
        let error =
            plugin_runtime::Package::from_files(resource_compatibility_files(manifest, ">=0.1.0"))
                .err()
                .expect("unsupported or malformed release floors must fail admission");
        assert!(error.to_string().contains("host"), "{error:#}");
    }
}

/// Restoring a future package diagnoses incompatibility while preserving enablement and private data.
#[test]
fn restored_host_floor_preserves_existing_choices_and_private_bytes() {
    let root = tempfile::tempdir().unwrap();
    let record = json!({"future-floor": {
        "manifest":{"id":"future-floor","name":"Future floor","version":"1.0.0",
            "protocol":7,"api":{"base":"^1"},"minimum_host":"2.0.0","component":"missing.wasm",
            "permissions":["storage"],"storage_limit":1024},
        "digest":"c".repeat(64),"grants":["storage"],"enabled":true,"project_enabled":["retained-project"]
    }});
    std::fs::write(
        root.path().join("registry.json"),
        serde_json::to_vec(&record).unwrap(),
    )
    .unwrap();
    let private = root.path().join("data/future-floor");
    std::fs::create_dir_all(&private).unwrap();
    let original = b"retained opaque private bytes";
    std::fs::write(private.join("retained.bin"), original).unwrap();
    let mut manager = Manager::open(root.path().into(), Environment::default()).unwrap();
    assert!(
        manager.installed["future-floor"]
            .error
            .as_ref()
            .is_some_and(|error| error.contains("minimum host"))
    );
    assert!(manager.live.is_empty());
    assert!(manager.enable("future-floor").is_err());
    let saved = Manager::read_registry(root.path()).unwrap();
    assert!(saved["future-floor"].enabled && saved["future-floor"].grants.contains("storage"));
    assert!(
        saved["future-floor"]
            .project_enabled
            .contains("retained-project")
    );
    assert_eq!(
        std::fs::read(private.join("retained.bin")).unwrap(),
        original
    );
}

/// Declaration admission precedes component compilation; full guest callbacks use SDK lifecycle fixtures.
fn executable_contract_files(
    manifest: serde_json::Value,
) -> std::collections::BTreeMap<String, Vec<u8>> {
    [
        (
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        ),
        ("test.wasm".into(), b"\0asm\x0d\0\x01\0".to_vec()),
    ]
    .into()
}

/// New schema semantics require an explicit minor minimum and an actual configuration callback.
#[test]
fn new_settings_declarations_cannot_silently_negotiate_configuration_1_0() {
    let base = json!({"id":"settings-contract","name":"Settings contract","version":"1.0.0",
    "protocol":7,"component":"test.wasm","settings_hook":true,
    "api":{"base":"^1","required":{"configuration":">=1.1,<2","storage.secrets":"^1"}},
    "permissions":["secrets"],"storage_limit":1024,
    "settings":{
        "timeout":{"title":"Timeout","value_type":{"kind":"integer","min":100,"max":120000},"default":1000,"apply":"hot_apply"},
        "key":{"title":"Own secret key","value_type":{"kind":"secret_reference"},"default":"api-token","apply":"hot_apply"}
    }});
    assert!(plugin_runtime::Package::from_files(executable_contract_files(base.clone())).is_ok());
    for mutation in [
        "old_range",
        "optional_only",
        "no_hook",
        "no_secret_grant",
        "control_key",
    ] {
        let mut manifest = base.clone();
        match mutation {
            "old_range" => manifest["api"]["required"]["configuration"] = json!("^1"),
            "optional_only" => {
                manifest["api"]["required"]
                    .as_object_mut()
                    .unwrap()
                    .remove("configuration");
                manifest["api"]["optional"] = json!({"configuration":">=1.1,<2"});
            }
            "no_hook" => manifest["settings_hook"] = json!(false),
            "no_secret_grant" => manifest["permissions"] = json!([]),
            "control_key" => manifest["settings"]["key"]["default"] = json!("foreign\nkey"),
            _ => unreachable!(),
        }
        assert!(
            plugin_runtime::Package::from_files(executable_contract_files(manifest)).is_err(),
            "{mutation}"
        );
    }
}

/// Continuous execution needs a useful purpose and required capability; resource-only assets stay cold.
#[test]
fn background_purpose_requires_a_component_and_explicit_lifecycle_negotiation() {
    let base = json!({"id":"background-contract","name":"Background contract","version":"1.0.0",
        "protocol":7,"component":"test.wasm","api":{"base":"^1","required":{"lifecycle.activation":"^1"}},
        "activation":{"background":{"purpose":"Refresh an explicitly authorized service cache"}},"storage_limit":1024});
    assert!(plugin_runtime::Package::from_files(executable_contract_files(base.clone())).is_ok());
    for purpose in ["", "  ", "credential\nrefresh", &"x".repeat(257)] {
        let mut manifest = base.clone();
        manifest["activation"]["background"]["purpose"] = json!(purpose);
        assert!(plugin_runtime::Package::from_files(executable_contract_files(manifest)).is_err());
    }
    let mut no_capability = base.clone();
    no_capability["api"] = json!({"base":"^1","optional":{"lifecycle.activation":"^1"}});
    assert!(plugin_runtime::Package::from_files(executable_contract_files(no_capability)).is_err());
    let mut no_component = base;
    no_component.as_object_mut().unwrap().remove("component");
    no_component["contributions"] = json!("plugin.toml");
    assert!(
        plugin_runtime::Package::from_files(resource_compatibility_files(no_component, ">=0.1.0"))
            .is_err()
    );
}

/// Missing component bytes cannot hide the earlier compatibility decision or change persistent user choices.
#[test]
fn old_registry_reports_update_requirement_before_accessing_component() {
    let root = tempfile::tempdir().unwrap();
    let record = json!({"legacy": {
        "manifest": {"id":"legacy","name":"Legacy","version":"1.0.0","protocol":6,
            "component":"missing.wasm","permissions":["storage"],"storage_limit":1024},
        "digest":"a".repeat(64),"grants":["storage"],"enabled":true,
        "project_enabled":["retained-project"]
    }});
    std::fs::write(
        root.path().join("registry.json"),
        serde_json::to_vec(&record).unwrap(),
    )
    .unwrap();
    let mut manager = Manager::open(root.path().into(), Environment::default()).unwrap();
    assert!(
        manager.installed["legacy"]
            .error
            .as_ref()
            .is_some_and(|error| error.contains("不兼容"))
    );
    assert!(manager.installed["legacy"].enabled);
    assert!(manager.live.is_empty());
    assert!(!manager.published_entries()[0].enabled);
    assert!(
        manager
            .enable("legacy")
            .unwrap_err()
            .to_string()
            .contains("不兼容")
    );
    let saved = Manager::read_registry(root.path()).unwrap();
    assert!(
        saved["legacy"].enabled && saved["legacy"].project_enabled.contains("retained-project")
    );
    assert!(saved["legacy"].grants.contains("storage"));
}

/// Historical aliases carry host settings and an immutable backup, while current package identities stay literal.
#[test]
fn legacy_alias_import_preserves_settings_and_original_bytes() {
    let root = tempfile::tempdir().unwrap();
    let record = json!({"me.example": {
        "manifest": {"id":"me.example","name":"Example","version":"1.0.0","protocol":6,
            "component":"missing.wasm","permissions":["storage"],"storage_limit":1024},
        "digest":"a".repeat(64),"grants":["storage"],"enabled":false,"project_enabled":["project-a"]
    }});
    let original = serde_json::to_vec(&record).unwrap();
    std::fs::write(root.path().join("registry.json"), &original).unwrap();
    std::fs::create_dir_all(root.path().join("settings")).unwrap();
    std::fs::create_dir_all(root.path().join("data/me.example")).unwrap();
    let settings = br#"{"user":{"label":"retained"},"projects":{}}"#;
    std::fs::write(root.path().join("settings/me.example.json"), settings).unwrap();
    std::fs::write(
        root.path().join("data/me.example/settings.json"),
        b"opaque private preferences",
    )
    .unwrap();
    let imported = Manager::read_registry(root.path()).unwrap();
    assert!(!imported["example"].enabled);
    assert!(imported["example"].project_enabled.contains("project-a"));
    assert_eq!(
        std::fs::read(root.path().join("settings/example.json")).unwrap(),
        settings
    );
    assert_eq!(
        std::fs::read(root.path().join("legacy-backup/v7/registry.json")).unwrap(),
        original
    );
    assert_eq!(
        std::fs::read(
            root.path()
                .join("legacy-backup/v7/plugins/example/data/settings.json")
        )
        .unwrap(),
        b"opaque private preferences"
    );
    Manager::read_registry(root.path()).unwrap();
    assert_eq!(
        std::fs::read(root.path().join("legacy-backup/v7/registry.json")).unwrap(),
        original
    );
    assert_eq!(
        std::fs::read(root.path().join("data/me.example/settings.json")).unwrap(),
        b"opaque private preferences"
    );
}

/// Build a current resource-only package whose author-selected ID remains literal at the public ZIP boundary.
fn literal_resource_package(id: &str) -> plugin_runtime::Package {
    use std::io::{Cursor, Write};
    let manifest = json!({"id":id,"name":"Literal resource","version":"1.0.0","protocol":7,
        "api":{"base":"^1"},"contributions":"plugin.toml","storage_limit":1024});
    let declaration = format!(
        "[plugin]\nid = \"{id}\"\nname = \"Literal resource\"\nversion = \"1.0.0\"\nhost_version = \">=0.1.0\"\n"
    );
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in [
        ("manifest.json", serde_json::to_vec(&manifest).unwrap()),
        ("plugin.toml", declaration.into_bytes()),
        ("README.md", b"# Literal resource".to_vec()),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    plugin_runtime::Package::from_bytes(&zip.finish().unwrap().into_inner()).unwrap()
}

/// A collision backup is recovery evidence for the old canonical owner, never data owned by a new literal alias.
#[test]
fn current_literal_alias_uninstalls_without_claiming_a_collision_backup() {
    let root = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(root.path().into(), Environment::default()).unwrap();
    let canonical = literal_resource_package("example");
    manager.install(&canonical, Default::default()).unwrap();
    drop(manager);
    let registry_path = root.path().join("registry.json");
    let mut registry: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&registry_path).unwrap()).unwrap();
    registry["me.example"] = json!({"manifest":{"id":"me.example","name":"Historical alias",
        "version":"0.0.6","protocol":6,"component":"missing.wasm","permissions":["storage"],"storage_limit":1024},
        "digest":"b".repeat(64),"grants":["storage"],"enabled":false,"project_enabled":["retained-project"]});
    std::fs::write(&registry_path, serde_json::to_vec(&registry).unwrap()).unwrap();
    let historical = root.path().join("data/me.example");
    std::fs::create_dir_all(&historical).unwrap();
    std::fs::write(
        historical.join("settings.json"),
        b"opaque historical settings",
    )
    .unwrap();
    let mut manager = Manager::open(root.path().into(), Environment::default()).unwrap();
    assert_eq!(manager.installed["example"].digest, canonical.digest);
    assert!(!manager.installed.contains_key("me.example"));
    let backup = root.path().join("legacy-backup/v7/plugins/me.example");
    let record = std::fs::read(backup.join("record.json")).unwrap();
    let metadata: serde_json::Value = serde_json::from_slice(&record).unwrap();
    assert_eq!(metadata["project_enabled"], json!(["retained-project"]));
    assert_eq!(metadata["grants"], json!(["storage"]));
    let literal = literal_resource_package("me.example");
    manager.install(&literal, Default::default()).unwrap();
    assert_eq!(manager.installed["me.example"].manifest.id, "me.example");
    manager
        .uninstall("me.example", true)
        .expect("deleting a current literal ID must not claim a historical collision backup");
    assert!(manager.installed.contains_key("example"));
    assert!(!manager.installed.contains_key("me.example"));
    assert_eq!(std::fs::read(backup.join("record.json")).unwrap(), record);
    assert_eq!(
        std::fs::read(backup.join("data/settings.json")).unwrap(),
        b"opaque historical settings"
    );
}
