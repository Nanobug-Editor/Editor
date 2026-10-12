//! Actual SDK configuration hooks revalidate selected native plans without reviving retired authority.
#[path = "support/demand.rs"]
mod demand_support;
/// Immutable SDK inputs bind exact versions and hashes, independently of generated target outputs.
#[path = "support/packages.rs"]
mod frozen_packages;
use plugin_runtime::{
    DemandReason, LanguageRole, Manager, Package,
    plugin_protocol::{Environment, settings::Scope},
};
use std::sync::Arc;

/// Same-owner HotApply hides the old configuration until selected-role validation proves its plan unchanged.
/// A genuinely changed executable then retires the old transport instead of rebinding its native authority.
#[test]
#[ignore = "opt-in fixed SDK/WASM packages; rebuild sources: crates/plugin-runtime/tests/fixtures/community; see fixtures/packages/README.md"]
fn hot_configuration_reuses_only_an_unchanged_selected_native_plan() {
    // This Hot/native-plan test enters the actual consumer built with the merged host SDK.
    let original = frozen_packages::package("capability-example-0.19.1-main-sdk");
    let mut files = original.files;
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["api"]["required"]["configuration"] = serde_json::json!("^1.1");
    manifest["api"]["required"]["language.lsp"] = serde_json::json!("^1");
    manifest["api"]["required"]["process"] = serde_json::json!("^1");
    manifest["permissions"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!("process.service.analysis"));
    manifest["services"] =
        serde_json::json!({"analysis":{"program":std::env::current_exe().unwrap(),"args":[]}});
    manifest["language_servers"] = serde_json::json!([
        {"id":"analysis","language":"independent","service":"analysis","executable_setting":"serverPath"},
    ]);
    manifest["settings"]["label"]["apply"] = serde_json::json!("hot_apply");
    manifest["settings"]["serverPath"] = serde_json::json!({
        "title":"Server path", "value_type":{"kind":"string","max_length":4096},
        "default":std::env::current_exe().unwrap(), "scope":"project", "apply":"hot_apply",
    });
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let package = Package::from_files(files).unwrap();
    let profile = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let id = &package.manifest.id;
    let key = format!("{id}/analysis");
    let role = || DemandReason::Language {
        provider: key.clone(),
        role: LanguageRole::Lsp,
    };
    demand_support::ready(&mut manager, id, role());
    let owner = manager.instance_id(id).unwrap().to_string();
    let original_token = manager.availability(id).unwrap();
    let before = manager.language_services()[&key].as_ref().unwrap().clone();
    manager
        .update_setting(
            id,
            Scope::Project,
            "label",
            Some(serde_json::json!("Changed label")),
        )
        .unwrap();
    assert_eq!(manager.instance_id(id), Some(owner.as_str()));
    assert_ne!(
        manager.availability(id).unwrap(),
        original_token,
        "pending old configuration intent must be revoked"
    );
    assert!(
        manager.language_services().is_empty(),
        "changed configuration stays hidden until selected-role revalidation"
    );
    assert!(
        before.is_active(),
        "same-owner hot validation must retain the old transport while its plan is checked"
    );
    demand_support::ready(&mut manager, id, role());
    let unchanged = manager.language_services()[&key].as_ref().unwrap().clone();
    assert!(
        Arc::ptr_eq(&before, &unchanged),
        "an unchanged plan preserves its exact original transport and authority"
    );
    assert!(unchanged.authority().is_live());

    // This disposable copy gives the same approved declaration a different real executable path.
    // No process is started; the assertion concerns the immutable plan and original native lifetime.
    let alternate = profile.path().join("alternate-service.exe");
    std::fs::copy(std::env::current_exe().unwrap(), &alternate).unwrap();
    manager
        .update_setting(
            id,
            Scope::Project,
            "serverPath",
            Some(serde_json::json!(alternate)),
        )
        .unwrap();
    assert!(manager.language_services().is_empty());
    assert!(before.is_active());
    demand_support::ready(&mut manager, id, role());
    let changed = manager.language_services()[&key].as_ref().unwrap().clone();
    assert!(!Arc::ptr_eq(&before, &changed));
    assert!(
        !before.is_active(),
        "a changed plan must seal retained old transport handles"
    );
    assert!(changed.is_active());
    assert_eq!(manager.instance_id(id), Some(owner.as_str()));
    manager.disable(id).unwrap();
    assert!(!changed.is_active());
    assert_eq!(manager.resource_count(), 0);
}
