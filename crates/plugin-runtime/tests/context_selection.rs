//! Actual SDK packages exercise context registration before any implementation-private assertions.
#[path = "support/demand.rs"]
mod demand_support;
use plugin_runtime::{
    DemandReason, Manager, Package,
    plugin_protocol::{Environment, ui},
};
use serde_json::{Value, json};

/// One original context reference survives an exact repeat, while generations and scopes are sealed.
#[test]
#[ignore = "build context-selection through --plugin-package and set NANOBUG_CONTEXT_SELECTION_PACKAGE"]
fn independent_package_registers_owned_context_references_and_rejects_stale_generations() {
    let input =
        std::env::var_os("NANOBUG_CONTEXT_SELECTION_PACKAGE").expect("current SDK fixture package");
    let package = Package::read(std::path::Path::new(&input)).unwrap();
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir(workspace.path().join("left")).unwrap();
    let profile = tempfile::tempdir().unwrap();
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
    demand_support::ready(&mut manager, id, DemandReason::Command("probe".into()));
    let descriptor = json!({"provider":"analysis","key":"project-left","generation":1,"project_root":"left","data":{"config":"left/project.json"}});
    let first = probe(
        &mut manager,
        id,
        json!({"kind":"register","contexts":[descriptor.clone()]}),
    );
    assert!(
        first.get("Ok").is_some(),
        "public SDK registration must succeed: {first}"
    );
    let reference = &first["Ok"]["LanguageContexts"][0];
    let token = reference["token"]
        .as_str()
        .expect("host-issued opaque token");
    assert_ne!(token, "left");
    assert_eq!(reference["descriptor"], descriptor);
    assert_eq!(
        probe(
            &mut manager,
            id,
            json!({"kind":"register","contexts":[descriptor.clone()]})
        ),
        first
    );
    assert_eq!(
        probe(&mut manager, id, json!({"kind":"describe","context":token})),
        first
    );
    let mut next = descriptor;
    next["generation"] = json!(2);
    let replacement = probe(
        &mut manager,
        id,
        json!({"kind":"register","contexts":[next]}),
    );
    assert!(
        replacement.get("Ok").is_some(),
        "new configuration must register: {replacement}"
    );
    assert_ne!(replacement["Ok"]["LanguageContexts"][0]["token"], token);
    let current = replacement["Ok"]["LanguageContexts"][0]["token"]
        .as_str()
        .unwrap();
    let stale = probe(&mut manager, id, json!({"kind":"describe","context":token}));
    assert_eq!(
        stale["Err"]["code"], "invalid_handle",
        "old generation must be revoked: {stale}"
    );
    // Reject a malformed tail before revoking the valid generation at the start of the same batch.
    let mut candidate = replacement["Ok"]["LanguageContexts"][0]["descriptor"].clone();
    candidate["generation"] = json!(3);
    let mut invalid = candidate.clone();
    invalid["key"] = json!("outside");
    invalid["project_root"] = json!("../outside");
    let rejected = probe(
        &mut manager,
        id,
        json!({"kind":"register","contexts":[candidate,invalid]}),
    );
    assert_eq!(rejected["Err"]["code"], "invalid_request");
    assert_eq!(
        probe(
            &mut manager,
            id,
            json!({"kind":"describe","context":current})
        ),
        replacement,
        "invalid batch must not publish or revoke a valid prefix"
    );

    // A second independently admitted identity runs the same guest bytes and sees the same registry.
    // A token's presence in that registry must not transfer the original plugin's authority.
    let mut files = package.files.clone();
    let peer = "peer-context-fixture";
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["id"] = json!(peer);
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let contributions = String::from_utf8(files["plugin.toml"].clone())
        .unwrap()
        .replace(id, peer);
    files.insert("plugin.toml".into(), contributions.into_bytes());
    let peer_package = Package::from_files(files).unwrap();
    manager
        .install(&peer_package, peer_package.manifest.permissions.clone())
        .unwrap();
    demand_support::ready(&mut manager, peer, DemandReason::Command("probe".into()));
    let foreign = probe(
        &mut manager,
        peer,
        json!({"kind":"describe","context":current}),
    );
    assert_eq!(
        foreign["Err"]["code"], "invalid_handle",
        "foreign package cannot describe an owned reference: {foreign}"
    );

    let other = tempfile::tempdir().unwrap();
    std::fs::create_dir(other.path().join("left")).unwrap();
    manager
        .select_workspace(
            Environment {
                workspace: other.path().display().to_string(),
                ..Default::default()
            },
            true,
        )
        .unwrap();
    demand_support::ready(&mut manager, id, DemandReason::Command("probe".into()));
    let foreign_scope = probe(
        &mut manager,
        id,
        json!({"kind":"describe","context":current}),
    );
    assert_eq!(
        foreign_scope["Err"]["code"], "invalid_handle",
        "same package in another live workspace cannot borrow the reference: {foreign_scope}"
    );
    manager
        .select_workspace(
            Environment {
                workspace: workspace.path().display().to_string(),
                ..Default::default()
            },
            true,
        )
        .unwrap();
    assert_eq!(
        probe(
            &mut manager,
            id,
            json!({"kind":"describe","context":current})
        ),
        replacement,
        "routing another live scope cannot retire the original context"
    );
    assert!(
        std::fs::read_dir(workspace.path().join("left"))
            .unwrap()
            .next()
            .is_none()
    );
    manager
        .checkpoint()
        .expect("independent context provider supports lifecycle checkpoints");
    manager.disable(id).unwrap();
    assert!(!manager.live.contains_key(id));
    manager.shutdown_checked().unwrap();
}

/// Read the ordinary published native scene; this is not a host-only testing operation.
fn probe(manager: &mut Manager, id: &str, operation: Value) -> Value {
    manager.invoke_command(id, "probe", operation).unwrap();
    let mut result = None;
    manager.live[id].views["status"].root.visit(&mut |node| {
        if let ui::Kind::Text { text } = &node.kind {
            result = serde_json::from_str(text).ok();
        }
    });
    result.expect("fixture publishes the public response")
}
