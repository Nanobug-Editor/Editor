//! Real SDK guests exercise bounded faults and manual recovery through the public package manager.
#[path = "support/demand.rs"]
mod demand_support;
/// Read fixed, hash-checked Package inputs without depending on removed plugin source or distribution payloads.
#[path = "support/packages.rs"]
mod test_packages;
use plugin_runtime::{
    DemandReason, Manager, Package,
    plugin_protocol::{Environment, ui::Kind},
};
use serde_json::json;
#[path = "support/service_packages.rs"]
mod service_packages;

/// Recover one faulting instance without restarting the manager or disturbing a healthy installed peer.
#[test]
#[ignore = "build the SDK capability-example first"]
fn wasm_fault_is_scoped_visible_and_independently_restartable() {
    let directory = tempfile::tempdir().unwrap();
    let package = Package::read(&test_packages::archive("capability-example-0.19.0")).unwrap();
    let workspace = directory.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::write(workspace.join("source.txt"), "workspace").unwrap();
    let root = directory.path().join("plugins");
    let mut manager = Manager::open(
        root.clone(),
        Environment {
            workspace: workspace.display().to_string(),
            ..Environment::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    // Installation publishes cold metadata; the original command, not enablement, owns first birth.
    demand_support::ready(
        &mut manager,
        "capability-example",
        DemandReason::Command("scope-write".into()),
    );
    manager
        .invoke_command(
            "capability-example",
            "scope-write",
            json!({"text":"recoverable"}),
        )
        .unwrap();
    let peer = service_packages::package("healthy-peer", true, true, "1.0.0");
    manager
        .install(&peer, peer.manifest.permissions.clone())
        .unwrap();
    demand_support::ready(
        &mut manager,
        "healthy-peer",
        DemandReason::Command("echo".into()),
    );
    manager
        .invoke_command("healthy-peer", "echo", json!("healthy peer"))
        .unwrap();
    let healthy_view = manager.live["healthy-peer"].views["welcome"].clone();
    manager
        .invoke_command("capability-example", "active-directory", json!(null))
        .unwrap();
    let pending = manager
        .live
        .get_mut("capability-example")
        .unwrap()
        .take_editor_requests()
        .pop()
        .unwrap();
    assert!(pending.begin());
    let started = std::time::Instant::now();
    assert!(
        manager
            .invoke_command("capability-example", "fault-spin", json!(null))
            .is_err()
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    let report = manager.diagnostics("capability-example");
    assert!(
        report
            .iter()
            .any(|report| report.operation.contains("fault-spin")
                && report.message.contains("budget"))
    );
    assert!(manager.live["capability-example"].views.is_empty());
    assert!(!pending.enter_side_effect());
    assert!(std::sync::Arc::ptr_eq(
        &healthy_view,
        manager.live["healthy-peer"].views.values().next().unwrap()
    ));
    assert_eq!(manager.live["capability-example"].resource_count(), 0);
    manager.restart_plugin("capability-example").unwrap();
    demand_support::ready(
        &mut manager,
        "capability-example",
        DemandReason::Command("fault-memory".into()),
    );
    assert!(
        manager
            .invoke_command("capability-example", "fault-memory", json!(null))
            .is_err()
    );
    assert!(
        manager
            .diagnostics("capability-example")
            .iter()
            .any(|report| report.operation.contains("fault-memory"))
    );
    manager.restart_plugin("capability-example").unwrap();
    demand_support::ready(
        &mut manager,
        "capability-example",
        DemandReason::Command("active-directory".into()),
    );
    manager
        .invoke_command("capability-example", "active-directory", json!(null))
        .unwrap();
    let document = manager.live["capability-example"]
        .views
        .values()
        .next()
        .unwrap()
        .as_ref();
    let Kind::Text { text } = &document.root.kind else {
        panic!("native text expected")
    };
    assert!(text.contains("Accepted"));
    // Explicit restart resets the cold owner. Actual first use validates executable assets; a
    // missing component fails that original demand without discarding the committed private data.
    let component = root
        .join("packages")
        .join("capability-example")
        .join(&manager.installed["capability-example"].digest)
        .join("capability-example.wasm");
    let bytes = std::fs::read(&component).unwrap();
    std::fs::remove_file(&component).unwrap();
    manager.restart_plugin("capability-example").unwrap();
    assert!(manager.instance_id("capability-example").is_none());
    let missing = demand_support::try_ready(
        &mut manager,
        "capability-example",
        DemandReason::Command("scope-read".into()),
    )
    .unwrap_err();
    assert!(
        manager.installed["capability-example"]
            .error
            .as_ref()
            .unwrap()
            .contains(&missing.to_string())
    );
    assert_eq!(
        std::fs::read_to_string(
            manager
                .data_directory("capability-example")
                .join("value.txt")
        )
        .unwrap(),
        "recoverable"
    );
    std::fs::write(&component, bytes).unwrap();
    manager.restart_plugin("capability-example").unwrap();
    demand_support::ready(
        &mut manager,
        "capability-example",
        DemandReason::Command("scope-read".into()),
    );
    manager
        .invoke_command("capability-example", "scope-read", json!(null))
        .unwrap();
    let Kind::Text { text } = &manager.live["capability-example"]
        .views
        .values()
        .next()
        .unwrap()
        .as_ref()
        .root
        .kind
    else {
        panic!("recovered text expected")
    };
    assert_eq!(text, "workspace|recoverable");
    assert!(
        std::sync::Arc::ptr_eq(
            &healthy_view,
            manager.live["healthy-peer"].views.get("welcome").unwrap()
        ),
        "peer Store and view survive every unrelated fault and recovery"
    );
    manager
        .invoke_command(
            "healthy-peer",
            "echo",
            json!("still healthy after recovery"),
        )
        .unwrap();
}
