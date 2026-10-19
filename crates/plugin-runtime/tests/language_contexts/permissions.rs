//! A provider's read grant cannot authorize another plugin's related-document request.
use plugin_runtime::{
    DemandReason, Manager, Package,
    plugin_protocol::{Environment, api, language_context},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{Duration, Instant},
};

#[path = "../support/demand.rs"]
mod demand;

/// The public broker preserves the original restricted caller even when the owner can read the
/// workspace. Toggling one request flag must not silently borrow that owner's stronger grant.
#[test]
#[ignore = "build native-consumers >=0.1.2 with host --plugin-package and set NANOBUG_NATIVE_CONSUMERS_PACKAGE"]
fn related_snapshot_requires_the_original_callers_workspace_read() {
    let input = std::env::var_os("NANOBUG_NATIVE_CONSUMERS_PACKAGE")
        .map(PathBuf::from)
        .expect("NANOBUG_NATIVE_CONSUMERS_PACKAGE");
    let component = Package::read(&input).unwrap().files["plugin.wasm"].clone();
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("source.novel"), "source text").unwrap();
    std::fs::write(workspace.path().join("unrelated.novel"), "private sibling").unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    for package in packages(&component) {
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
    }
    demand::ready(
        &mut manager,
        "restricted-reader",
        DemandReason::Command("snapshot".into()),
    );
    // An ordinary source-bound request uses only the declared editor.read and proves that the
    // unknown SDK caller, service resolution and native envelope all reached their actual seam.
    let (ordinary, ordinary_requests) = invoke(&mut manager, false);
    assert_eq!(ordinary["state"], "complete", "{ordinary}");
    assert_eq!(ordinary_requests, 1);
    let owner = manager.instance_id("privileged-reader").unwrap().to_owned();
    let (related, related_requests) = invoke(&mut manager, true);
    assert_eq!(
        related_requests, 0,
        "an ungranted original caller must not publish a request that can capture sibling sources: {related}"
    );
    assert_eq!(related["state"], "failed", "{related}");
    let failure: api::Failure = serde_json::from_value(related["error"].clone()).unwrap();
    assert_eq!(failure.code, api::ErrorCode::PermissionDenied);
    assert_eq!(
        manager.instance_id("privileged-reader"),
        Some(owner.as_str())
    );
    assert_eq!(
        std::fs::read_to_string(workspace.path().join("unrelated.novel")).unwrap(),
        "private sibling"
    );
    manager.shutdown();
}

/// The same method schema deliberately delegates only editor.read; graph discovery separately
/// declares its full canonical contract so a missing provider capability cannot mask the failure.
fn packages(component: &[u8]) -> Vec<Package> {
    let method = json!({"parameters":{"type":"record","fields":{
        "document":{"type":"record","fields":{
            "path":{"type":"string","max_bytes":4096},
            "generation":{"type":"integer","min":0,"max":9_007_199_254_740_991_i64}},"optional":[]},
        "context_snapshot":{"type":"boolean"}},"optional":[]},
        "result":{"type":"null"},"permissions":["editor.read"]});
    let base = json!({"version":"0.1.0","protocol":7,"component":"plugin.wasm","storage_limit":16384,
        "api":{"base":"^1","required":{"plugin.services":"^1.1","language.control":"^1","language.lsp":"^1.5","process":"^1"}}});
    let mut owner = base.clone();
    owner["id"] = json!("privileged-reader");
    owner["name"] = json!("Unknown reader owning workspace consent");
    owner["permissions"] = json!([
        "services.call",
        "editor.read",
        "workspace.read",
        "process.service.analysis"
    ]);
    owner["plugin_services"] =
        json!({"provides":{"novel.engine":{"version":"1.0.0","methods":{"snapshot":method}}}});
    owner["services"] = json!({"analysis":{"program":std::env::current_exe().unwrap(),"args":[]}});
    owner["language_servers"] = json!([{"id":"analysis","language":"unfamiliar-graph","service":"analysis",
        "related_documents":"novel/related","control_methods":{
            "novel/related":language_context::related_documents_method(),
            "novel/snapshot":{"parameters":{"type":"null"},"result":{"type":"null"},"permissions":["editor.read"]}}}]);
    let mut caller = base;
    caller["id"] = json!("restricted-reader");
    caller["name"] = json!("Unknown original caller without workspace consent");
    caller["permissions"] = json!(["services.call", "editor.read"]);
    caller["commands"] = json!([{"id":"snapshot","title":"Request a source snapshot"}]);
    caller["plugin_services"] =
        json!({"requires":{"novel.engine":{"version":"^1","methods":{"snapshot":method}}}});
    [owner, caller]
        .into_iter()
        .map(|manifest| {
            Package::from_files(BTreeMap::from([
                (
                    "manifest.json".into(),
                    serde_json::to_vec(&manifest).unwrap(),
                ),
                ("plugin.wasm".into(), component.to_vec()),
            ]))
            .unwrap()
        })
        .collect()
}

/// Complete any erroneously admitted host envelope normally, so the RED signal is authority
/// admission rather than a synthetic dispatcher denial, timeout or missing native implementation.
fn invoke(manager: &mut Manager, related: bool) -> (Value, usize) {
    manager
        .invoke_command(
            "restricted-reader",
            "snapshot",
            json!({
                "document":{"path":"source.novel","generation":0},"context_snapshot":related
            }),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut emitted = 0;
    loop {
        manager.poll();
        for (_, request) in manager.take_editor_requests() {
            emitted += 1;
            assert!(request.begin());
            assert_eq!(
                request.language_control_metadata().unwrap().consumer,
                manager.instance_id("restricted-reader").unwrap()
            );
            request.finish(Ok(api::EditorValue::Language(Value::Null)));
        }
        let snapshot = manager
            .live
            .get_mut("restricted-reader")
            .unwrap()
            .snapshot()
            .unwrap();
        let receipt = serde_json::from_str::<Value>(&snapshot.data).unwrap()["receipt"].clone();
        if matches!(receipt["state"].as_str(), Some("complete" | "failed")) {
            return (receipt, emitted);
        }
        assert!(
            Instant::now() < deadline,
            "original snapshot request did not finish: {receipt}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
