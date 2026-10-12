//! Real SDK service consumers discover cold providers and retain their original queued authority.
/// Immutable SDK inputs bind exact versions and hashes, independently of generated target outputs.
#[path = "support/packages.rs"]
mod frozen_packages;
use plugin_runtime::{
    DemandReason, Manager, Package,
    plugin_protocol::{Environment, api, ui::Kind},
};
use serde_json::json;

#[path = "support/service_packages.rs"]
#[allow(dead_code)]
mod service_packages;

#[path = "support/interactive_packages.rs"]
#[allow(dead_code)]
mod interactive_packages;

#[path = "service_activation/nested.rs"]
mod nested;

/// A cold debug request must survive the period before its selected SDK provider is born.
/// This fixture deliberately lacks a debug implementation: its eventual UnsupportedOperation
/// proves a real guest answered, rather than an invented success or premature owner retirement.
#[test]
#[ignore = "opt-in fixed SDK/WASM packages; rebuild sources: crates/plugin-runtime/tests/fixtures/community; see fixtures/packages/README.md"]
fn cold_debug_start_waits_for_its_original_provider_birth() {
    let profile = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let mut files = service_packages::pure_provider("cold-debug-provider").files;
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["plugin_services"] =
        json!({"provides":{"debug.session":plugin_runtime::plugin_protocol::debug::declaration()}});
    // The independently declared contract also needs all of its actual method permissions.
    let mut permissions: std::collections::BTreeSet<String> =
        serde_json::from_value(manifest["permissions"].clone()).unwrap();
    for method in plugin_runtime::plugin_protocol::debug::declaration()
        .methods
        .values()
    {
        permissions.extend(method.permissions.iter().cloned());
    }
    manifest["permissions"] = serde_json::to_value(permissions).unwrap();
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let package = Package::from_files(files).unwrap();
    let mut manager = Manager::open(
        profile.path().to_owned(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let request = manager
        .begin_debug_call("start", json!({"program":"fixture-only","args":[]}))
        .unwrap();
    manager.poll();
    assert!(
        !request.status().is_terminal(),
        "cold owner was retired before birth: {:?}",
        request.status()
    );
    wait(&mut manager, |_| request.status().is_terminal());
    assert!(
        manager.instance_id("cold-debug-provider").is_some(),
        "the original provider must actually receive the method"
    );
    assert!(
        matches!(request.status(), api::RequestUpdate::Completed { result: Err(error) }
        if error.code == api::ErrorCode::UnsupportedOperation),
        "unexpected receipt: {:?}",
        request.status()
    );
    manager.shutdown();
}

/// A native launch resolves a cold provider, then its retained session shares the same exact first birth.
/// The real controlled Windows child exits; provider selection cannot turn the pending launch into a default.
#[test]
#[cfg(windows)]
#[ignore = "opt-in fixed SDK/WASM packages; rebuild sources: crates/plugin-runtime/tests/fixtures/community; see fixtures/packages/README.md"]
fn cold_native_execution_binds_its_pending_session_once() {
    let profile = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        profile.path().to_owned(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    let package = interactive_packages::provider("cold-runner");
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    manager
        .set_service_provider(
            api::InstanceScope::Workspace,
            plugin_runtime::plugin_protocol::settings::Scope::Project,
            interactive_packages::CONTRACT,
            Some("cold-runner"),
        )
        .unwrap();
    assert!(manager.instance_id("cold-runner").is_none());
    let session = manager
        .start_execution(plugin_runtime::RunRequest {
            program: std::path::Path::new(&std::env::var("SystemRoot").unwrap())
                .join("System32/cmd.exe")
                .display()
                .to_string(),
            args: vec!["/C".into(), "exit".into(), "0".into()],
            cwd: Some(workspace.path().display().to_string()),
            name: None,
            env: vec![],
        })
        .unwrap();
    assert_eq!(session.plugin(), "cold-runner");
    assert!(
        manager.instance_id("cold-runner").is_none(),
        "admission cannot run a guest inline"
    );
    assert_eq!(
        session.snapshot().state,
        plugin_runtime::ExecutionState::Starting
    );
    wait(&mut manager, |_| session.update().is_terminal());
    assert!(
        matches!(
            session.update(),
            api::RequestUpdate::Completed { result: Ok(_) }
        ),
        "actual launch failed: {:?}",
        session.update()
    );
    assert_eq!(
        manager.instance_id("cold-runner"),
        Some(session.provider_instance())
    );
    wait(&mut manager, |_| !session.snapshot().state.is_active());
    assert_eq!(
        session.snapshot().state,
        plugin_runtime::ExecutionState::Exited
    );
    manager.disable("cold-runner").unwrap();
    assert_eq!(manager.resource_count(), 0);
    manager.shutdown();
}

/// Installation publishes service metadata; opening a reference must not materialize its provider.
/// A subsequent actual call binds first birth once, and cancellation cannot execute the old call.
#[test]
#[ignore = "opt-in fixed SDK/WASM packages; rebuild sources: crates/plugin-runtime/tests/fixtures/community; see fixtures/packages/README.md"]
fn cold_service_metadata_and_first_call_preserve_reference_and_original_deadline() {
    let profile = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        profile.path().to_owned(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    let provider = service_packages::package("cold-provider", true, false, "1.0.0");
    let consumer = service_packages::package("service-consumer", false, false, "^1");
    for package in [&provider, &consumer] {
        manager
            .install(package, package.manifest.permissions.clone())
            .unwrap();
    }
    let choice = manager
        .service_choices()
        .into_iter()
        .find(|choice| choice.contract == "example.echo")
        .expect("cold providers must participate in the real service selection registry");
    assert_eq!(choice.candidates, ["cold-provider"]);
    assert!(manager.instance_id("cold-provider").is_none());
    assert!(manager.instance_id("service-consumer").is_none());

    // The consumer's real command is an explicit demand. Its required dependency is metadata only.
    activate(
        &mut manager,
        &consumer,
        DemandReason::Command("service-open".into()),
    );
    manager
        .invoke_command("service-consumer", "service-open", json!("example.echo"))
        .unwrap();
    assert!(
        manager.instance_id("cold-provider").is_none(),
        "Open and required dependency checks stay cold"
    );
    assert_eq!(status(&manager), "Service opened");
    manager
        .invoke_command(
            "service-consumer",
            "service-call",
            json!({"method":"echo","value":"expired","timeout_ms":20}),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(30));
    manager.poll();
    assert!(
        matches!(
            serde_json::from_str::<api::RequestUpdate<serde_json::Value>>(&status(&manager))
                .unwrap(),
            api::RequestUpdate::Cancelled {
                reason: api::ErrorCode::TimedOut,
                effect: api::CancellationEffect::NotExecuted
            }
        ),
        "the original service deadline is never reset by activation"
    );
    assert!(manager.instance_id("cold-provider").is_none());
    manager
        .invoke_command(
            "service-consumer",
            "service-call",
            json!({"method":"echo","value":"cancelled","timeout_ms":120000}),
        )
        .unwrap();
    manager
        .invoke_command("service-consumer", "service-cancel", json!(null))
        .unwrap();
    manager.poll();
    assert!(matches!(
        serde_json::from_str::<api::RequestUpdate<serde_json::Value>>(&status(&manager)).unwrap(),
        api::RequestUpdate::Cancelled {
            reason: api::ErrorCode::Cancelled,
            effect: api::CancellationEffect::NotExecuted
        }
    ));
    assert!(manager.instance_id("cold-provider").is_none());

    // Poll creates a real inner waiter after this tick's preparation stage. Taking and finishing
    // its public job fixes the race deterministically: outer cancellation happens only when the
    // complete candidate is already waiting for final admission, without sleeps or test-only APIs.
    manager
        .invoke_command(
            "service-consumer",
            "service-call",
            json!({"method":"echo","value":"cancel-ready","timeout_ms":120000}),
        )
        .unwrap();
    manager.poll();
    let prepared = manager
        .take_demand_preparation()
        .expect("the actual broker call must queue activation")
        .run();
    manager
        .invoke_command("service-consumer", "service-cancel", json!(null))
        .unwrap();
    assert!(
        manager.commit_demand(prepared).is_err(),
        "outer cancellation must revoke a completed candidate before owner publication"
    );
    manager.poll();
    assert!(matches!(
        serde_json::from_str::<api::RequestUpdate<serde_json::Value>>(&status(&manager)).unwrap(),
        api::RequestUpdate::Cancelled {
            reason: api::ErrorCode::Cancelled,
            effect: api::CancellationEffect::NotExecuted
        }
    ));
    assert!(
        manager.instance_id("cold-provider").is_none(),
        "cancelled outer service cannot publish Main"
    );

    // Even an explicit choice of the same package changes the original selection revision. The
    // pending call must release its activation wait before another Manager poll can publish an owner.
    manager
        .invoke_command(
            "service-consumer",
            "service-call",
            json!({"method":"echo","value":"old-choice","timeout_ms":120000}),
        )
        .unwrap();
    manager.poll();
    manager
        .set_service_provider(
            api::InstanceScope::Workspace,
            plugin_runtime::plugin_protocol::settings::Scope::User,
            "example.echo",
            Some("cold-provider"),
        )
        .unwrap();
    manager.poll();
    assert!(status(&manager).contains("invalid_handle"));
    assert!(manager.instance_id("cold-provider").is_none());
    manager
        .invoke_command("service-consumer", "service-open", json!("example.echo"))
        .unwrap();
    assert_eq!(status(&manager), "Service opened");
    manager
        .invoke_command(
            "service-consumer",
            "service-call",
            json!({"method":"echo","value":"original","timeout_ms":120000}),
        )
        .unwrap();
    assert_eq!(status(&manager), "Accepted");
    assert!(
        manager.instance_id("cold-provider").is_none(),
        "guest enqueue cannot compile a peer inline"
    );
    wait(&mut manager, |manager| status(manager).contains("original"));
    let owner = manager.instance_id("cold-provider").unwrap().to_owned();

    // The previously opened reference survives its one first birth and sends the next literal value.
    manager
        .invoke_command(
            "service-consumer",
            "service-call",
            json!({"method":"source","value":"second","timeout_ms":30000}),
        )
        .unwrap();
    wait(&mut manager, |manager| {
        status(manager).contains("service-consumer")
    });
    assert_eq!(manager.instance_id("cold-provider"), Some(owner.as_str()));

    // Retirement is irreversible for an open reference, even if the same package is enabled again.
    manager.disable("cold-provider").unwrap();
    manager.enable("cold-provider").unwrap();
    manager
        .invoke_command(
            "service-consumer",
            "service-call",
            json!({"method":"echo","value":"late","timeout_ms":30000}),
        )
        .unwrap();
    assert!(status(&manager).contains("InvalidHandle"));
    assert!(manager.instance_id("cold-provider").is_none());
    manager.shutdown();
}

/// A call queued before HotApply cannot borrow the new configuration to start a cold provider.
/// A fresh call through the same open reference captures the new configuration and may start it.
#[test]
#[ignore = "opt-in fixed SDK/WASM packages; rebuild sources: crates/plugin-runtime/tests/fixtures/community; see fixtures/packages/README.md"]
fn queued_cold_service_keeps_its_configuration_while_open_reference_survives_hot_apply() {
    let original = service_packages::package("hot-provider", true, false, "1.0.0");
    let current = frozen_packages::package("capability-example-0.19.1");
    let mut files = original.files;
    // Reuse the current independently SDK-built guest; only generic service and Hot declarations vary.
    files.insert(
        original.manifest.component.unwrap(),
        current.files[current.manifest.component.as_ref().unwrap()].clone(),
    );
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["settings_hook"] = json!(true);
    manifest["api"]["required"]["configuration"] = json!("^1.1");
    manifest["settings"]["label"]["apply"] = json!("hot_apply");
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let provider = Package::from_files(files).unwrap();
    let consumer = service_packages::package("service-consumer", false, false, "^1");
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
    for package in [&provider, &consumer] {
        manager
            .install(package, package.manifest.permissions.clone())
            .unwrap();
    }
    activate(
        &mut manager,
        &consumer,
        DemandReason::Command("service-open".into()),
    );
    manager
        .invoke_command("service-consumer", "service-open", json!("example.echo"))
        .unwrap();
    manager
        .invoke_command(
            "service-consumer",
            "service-call",
            json!({"method":"echo","value":"old-hot-intent","timeout_ms":120000}),
        )
        .unwrap();
    assert_eq!(status(&manager), "Accepted");
    // No Manager poll has started the demand: configuration revocation must also reach broker-only queues.
    manager
        .update_setting(
            "hot-provider",
            plugin_runtime::plugin_protocol::settings::Scope::Project,
            "label",
            Some(json!("new label")),
        )
        .unwrap();
    manager.poll();
    assert!(status(&manager).contains("invalid_handle"));
    assert!(manager.instance_id("hot-provider").is_none());
    manager
        .invoke_command(
            "service-consumer",
            "service-call",
            json!({"method":"echo","value":"after-hot","timeout_ms":120000}),
        )
        .unwrap();
    wait(&mut manager, |manager| {
        status(manager).contains("after-hot")
    });
    let owner = manager.instance_id("hot-provider").unwrap().to_owned();
    manager
        .update_setting(
            "hot-provider",
            plugin_runtime::plugin_protocol::settings::Scope::Project,
            "label",
            Some(json!("another label")),
        )
        .unwrap();
    manager
        .invoke_command(
            "service-consumer",
            "service-call",
            json!({"method":"echo","value":"same-open","timeout_ms":30000}),
        )
        .unwrap();
    wait(&mut manager, |manager| {
        status(manager).contains("same-open")
    });
    assert_eq!(manager.instance_id("hot-provider"), Some(owner.as_str()));
    manager.shutdown();
}

/// Use the same public demand queue as a native command, with its independent bounded deadline.
fn activate(manager: &mut Manager, package: &Package, reason: DemandReason) {
    let token = manager.availability(&package.manifest.id).unwrap();
    let demand = manager.begin_demand(&token, reason, 120_000).unwrap();
    wait(manager, |_| demand.status().is_terminal());
    let api::RequestUpdate::Completed {
        result: Ok(receipt),
    } = demand.status()
    else {
        panic!("the consumer demand failed: {:?}", demand.status());
    };
    manager.admit_receipt(&token, &receipt).unwrap();
}

/// Observe the actual SDK guest's native status view, rather than a host-only synthetic result.
fn status(manager: &Manager) -> String {
    let Kind::Text { text } = &manager.live["service-consumer"].views["welcome"].root.kind else {
        panic!("expected a real consumer status");
    };
    text.clone()
}

/// Drive one existing manager until the real asynchronous seam produces its observable result.
fn wait(manager: &mut Manager, ready: impl Fn(&Manager) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    while !ready(manager) {
        manager.poll();
        assert!(
            std::time::Instant::now() < deadline,
            "service demand deadline: {}",
            manager
                .instance_id("service-consumer")
                .map(|_| status(manager))
                .unwrap_or_default()
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
