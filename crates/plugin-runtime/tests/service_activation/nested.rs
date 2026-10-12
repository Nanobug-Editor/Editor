//! Real nested callers exercise cancellation sources independently of the child's own deadline.
/// Immutable SDK inputs bind exact versions and hashes, independently of generated target outputs.
#[path = "../support/packages.rs"]
mod frozen_packages;
use super::*;
use std::time::{Duration, Instant};

/// Cancelled/pending-expired ancestors seal first birth; completed ancestors retain their valid resources.
#[test]
#[ignore = "opt-in fixed demand-relay/capability-example packages; relay source: crates/plugin-runtime/tests/fixtures/demand-relay"]
fn nested_service_ancestors_cancel_before_birth_without_expiring_completed_sources() {
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
    let (inner, relay, consumer) = nested_packages();
    for package in [&inner, &relay, &consumer] {
        manager
            .install(package, package.manifest.permissions.clone())
            .unwrap();
    }
    activate(&mut manager, &relay, DemandReason::View("receipt".into()));
    activate(
        &mut manager,
        &consumer,
        DemandReason::Command("service-open".into()),
    );
    manager
        .invoke_command("service-consumer", "service-open", json!("example.echo"))
        .unwrap();
    let baseline = manager.resource_count();

    // The warm relay defers the outer reply; two public polls create the cold child's actual waiter.
    queue_child(&mut manager, true, 120_000, "cancelled ancestor");
    let prepared = manager
        .take_demand_preparation()
        .expect("nested call must queue the cold provider")
        .run();
    manager
        .invoke_command("service-consumer", "service-cancel", json!(null))
        .unwrap();
    assert!(
        manager.commit_demand(prepared).is_err(),
        "cancelled ancestor cannot publish a child's already prepared Main"
    );
    drain_cancelled(&mut manager, baseline);

    // No parent status/poll observes this deadline before final admission. The child's long
    // independent deadline must still observe an unfinished ancestor's natural expiry.
    let started = Instant::now();
    queue_child(&mut manager, true, 1_000, "expired ancestor");
    let prepared = manager.take_demand_preparation().unwrap().run();
    std::thread::sleep(Duration::from_millis(1_100).saturating_sub(started.elapsed()));
    assert!(
        manager.commit_demand(prepared).is_err(),
        "unobserved ancestor expiry cannot publish the child"
    );
    drain_cancelled(&mut manager, baseline);

    // Immediate reply finishes this ancestor before its old deadline. Resource continuations
    // remain legitimate while the original owners live; a blanket minimum deadline would break this.
    let started = Instant::now();
    let original = manager.availability("inner-provider").unwrap();
    queue_child(&mut manager, false, 1_000, "completed ancestor");
    assert!(matches!(
        serde_json::from_str::<api::RequestUpdate<serde_json::Value>>(&status(&manager)).unwrap(),
        api::RequestUpdate::Completed { result: Ok(_) }
    ));
    let prepared = manager.take_demand_preparation().unwrap().run();
    std::thread::sleep(Duration::from_millis(1_100).saturating_sub(started.elapsed()));
    let receipt = manager.commit_demand(prepared).unwrap();
    manager.admit_receipt(&original, &receipt).unwrap();
    assert_eq!(manager.instance_id("inner-provider"), receipt.owner());
    wait(&mut manager, |manager| {
        let Kind::Text { text } = &manager.live["demand-relay"].views["receipt"].root.kind else {
            panic!("expected actual child receipt");
        };
        text.contains("completed ancestor")
    });
    manager.shutdown();
    assert_eq!(manager.resource_count(), 0);
}

/// Restrict both real components to matching declared methods, rather than inventing host callbacks.
fn nested_packages() -> (Package, Package, Package) {
    let relay = frozen_packages::package("demand-relay-0.1.0");
    let mut inner_files = relay.files.clone();
    let mut inner_manifest: serde_json::Value =
        serde_json::from_slice(&inner_files["manifest.json"]).unwrap();
    inner_manifest["id"] = json!("inner-provider");
    inner_manifest["plugin_services"] = json!({
        "provides":{"example.inner":{"version":"1.0.0","methods":{
            "echo":{"parameters":{"type":"string","max_bytes":2048},
                "result":{"type":"string","max_bytes":4096}}
        }}}
    });
    inner_files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&inner_manifest).unwrap(),
    );
    let inner = Package::from_files(inner_files).unwrap();
    let mut consumer_files =
        service_packages::package("service-consumer", false, false, "^1").files;
    let mut consumer_manifest: serde_json::Value =
        serde_json::from_slice(&consumer_files["manifest.json"]).unwrap();
    consumer_manifest["plugin_services"] = json!({
        "requires":{"example.echo":{"version":"^1","methods":{
            "relay":{"parameters":{"type":"string","max_bytes":2048},
                "result":{"type":"string","max_bytes":4096}}
        }}}
    });
    consumer_files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&consumer_manifest).unwrap(),
    );
    (inner, relay, Package::from_files(consumer_files).unwrap())
}

/// Dispatch the warm ancestor first, then admit its actual child without running the job inline.
fn queue_child(manager: &mut Manager, defer: bool, timeout_ms: u32, value: &str) {
    manager
        .invoke_command(
            "service-consumer",
            "service-call",
            json!({
                "method":"relay", "timeout_ms":timeout_ms,
                "value":json!({"contract":"example.inner","value":value,"defer":defer}).to_string()
            }),
        )
        .unwrap();
    manager.poll();
    manager.poll();
    assert!(manager.instance_id("inner-provider").is_none());
}

/// A cancelled source releases its invocation and child handles, as well as the unpublished candidate.
fn drain_cancelled(manager: &mut Manager, baseline: usize) {
    manager.poll();
    manager.poll();
    assert!(manager.instance_id("inner-provider").is_none());
    assert_eq!(manager.resource_count(), baseline);
}
