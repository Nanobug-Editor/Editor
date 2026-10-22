//! Independent service packages must negotiate and validate contracts before any provider executes.
use plugin_runtime::plugin_protocol::service::{
    Contract, Declarations, Dependency, Method, Schema,
};
use plugin_runtime::{
    Manager, Package,
    plugin_protocol::{Environment, settings::Scope, ui::Kind},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// A dependency names a contract and version range; neither schema nor authority comes from the caller payload.
#[test]
fn service_contract_rejects_unbounded_shapes_and_malformed_names() {
    let method = Method {
        parameters: Schema::String { max_bytes: 64 },
        result: Schema::String { max_bytes: 64 },
        permissions: Default::default(),
    };
    let mut declarations = Declarations {
        provides: BTreeMap::from([(
            "example.echo".into(),
            Contract {
                collection: false,
                preparation: Default::default(),
                enabled_setting: None,
                platforms: Vec::new(),
                version: "1.0.0".parse().unwrap(),
                methods: BTreeMap::from([("echo".into(), method.clone())]),
            },
        )]),
        requires: BTreeMap::from([(
            "example.format".into(),
            Dependency {
                collection: false,
                version: "^1".parse().unwrap(),
                optional: true,
                methods: BTreeMap::from([("echo".into(), method)]),
            },
        )]),
    };
    assert!(declarations.validate().is_ok());
    declarations
        .provides
        .get_mut("example.echo")
        .unwrap()
        .methods
        .get_mut("echo")
        .unwrap()
        .parameters = Schema::String {
        max_bytes: usize::MAX,
    };
    assert!(declarations.validate().is_err());
    assert!(
        Schema::String { max_bytes: 4 }
            .accepts(&serde_json::json!("中文"))
            .is_err()
    );
}

#[path = "support/service_packages.rs"]
mod service_packages;
use service_packages::package;

/// Cold service discovery preserves platform and per-project configuration gates after the merge.
/// The real frozen component is admitted, but these catalog/settings operations never demand it.
#[test]
#[ignore = "opt-in frozen Package/WASM fixture; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn cold_service_publication_respects_platform_and_original_project_settings() {
    let profile = tempfile::tempdir().unwrap();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let mut files = package("gated-provider", true, false, "1.0.0").files;
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["api"]["required"]["dependencies"] = json!(">=1.2,<2");
    manifest["settings"]["gate.enabled"] = json!({
        "title":"Enable shared provider", "value_type":{"kind":"boolean"},
        "default":true, "scope":"project"
    });
    let contract = &mut manifest["plugin_services"]["provides"]["example.echo"];
    contract["enabled_setting"] = json!("gate.enabled");
    contract["platforms"] = json!([plugin_runtime::development::platform()]);
    let mut unsupported = contract.clone();
    unsupported["platforms"] = json!([if cfg!(target_os = "windows") {
        "linux-x86_64"
    } else {
        "windows-x86_64"
    }]);
    manifest["plugin_services"]["provides"]["unavailable.echo"] = unsupported;
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let package = Package::from_files(files).unwrap();
    let environment = |workspace: &std::path::Path| Environment {
        workspace: workspace.display().to_string(),
        ..Default::default()
    };
    let mut manager = Manager::open(profile.path().into(), environment(first.path())).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let choices = manager.service_choices();
    assert!(
        choices
            .iter()
            .any(|choice| choice.contract == "example.echo"
                && choice.candidates == ["gated-provider"])
    );
    assert!(
        !choices
            .iter()
            .any(|choice| choice.contract == "unavailable.echo")
    );
    manager
        .update_setting(
            "gated-provider",
            Scope::Project,
            "gate.enabled",
            Some(json!(false)),
        )
        .unwrap();
    assert!(
        !manager
            .service_choices()
            .iter()
            .any(|choice| choice.contract == "example.echo"
                && choice.candidates.contains(&"gated-provider".into()))
    );
    // A confirmed override belongs to its captured project, not the next selected workspace.
    manager
        .switch_workspace(environment(second.path()), true)
        .unwrap();
    assert!(
        manager
            .service_choices()
            .iter()
            .any(|choice| choice.contract == "example.echo"
                && choice.candidates.contains(&"gated-provider".into()))
    );
    manager
        .switch_workspace(environment(first.path()), true)
        .unwrap();
    assert!(
        !manager
            .service_choices()
            .iter()
            .any(|choice| choice.contract == "example.echo"
                && choice.candidates.contains(&"gated-provider".into()))
    );
    assert!(
        manager.live.is_empty(),
        "discovery/configuration must remain cold"
    );
    assert_eq!(manager.resource_count(), 0);
    manager.shutdown();
}

fn install(manager: &mut Manager, package: Package) {
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    // These existing authority/cleanup scenarios observe the real status view. Showing that view
    // is their explicit demand; separate service_activation tests assert installation stays cold.
    demand(manager, &package.manifest.id).unwrap();
}
/// Drive a public view demand without turning installation or metadata discovery into activation.
fn demand(
    manager: &mut Manager,
    id: &str,
) -> Result<(), plugin_runtime::plugin_protocol::api::Failure> {
    let token = manager.availability(id)?;
    let request = manager.begin_demand(
        &token,
        plugin_runtime::DemandReason::View("welcome".into()),
        120_000,
    )?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        manager.poll();
        match request.status() {
            plugin_runtime::plugin_protocol::api::RequestUpdate::Completed { result } => {
                return result.and_then(|receipt| manager.admit_receipt(&token, &receipt));
            }
            plugin_runtime::plugin_protocol::api::RequestUpdate::Cancelled { reason, .. } => {
                return Err(plugin_runtime::plugin_protocol::api::Failure::new(
                    reason,
                    "Service fixture demand ended",
                ));
            }
            _ => {}
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Service fixture activation timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
fn text(manager: &Manager, id: &str) -> String {
    let document = manager.live[id].views["welcome"].as_ref();
    let Kind::Text { text } = &document.root.kind else {
        panic!("Expected native status")
    };
    text.clone()
}
fn command(manager: &mut Manager, id: &str, command: &str, args: Value) -> String {
    manager.invoke_command(id, command, args).unwrap();
    text(manager, id)
}
fn call(manager: &mut Manager, method: &str, value: Value, timeout: u32) -> String {
    command(
        manager,
        "service-consumer",
        "service-call",
        json!({"method":method,"value":value,"timeout_ms":timeout}),
    )
}

/// A deferred provider keeps the request pending and later completes the same public consumer task.
#[test]
#[ignore = "build capability-example through the current public SDK first"]
fn a_provider_can_reply_after_its_initial_wasm_callback_returns() {
    let temp = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(temp.path().join("plugins"), Environment::default()).unwrap();
    install(
        &mut manager,
        package("deferred-provider", true, false, "1.0.0"),
    );
    install(
        &mut manager,
        package("service-consumer", false, false, "^1"),
    );
    command(
        &mut manager,
        "service-consumer",
        "service-open",
        json!("example.echo"),
    );
    assert_eq!(
        call(&mut manager, "defer", json!("pending"), 30000),
        "Accepted"
    );
    for _ in 0..3 {
        manager.poll();
    }
    let pending: plugin_runtime::plugin_protocol::api::RequestUpdate<Value> =
        serde_json::from_str(&text(&manager, "service-consumer")).unwrap();
    assert!(
        !pending.is_terminal(),
        "omitting the immediate reply must retain a bounded pending request: {pending:?}"
    );
    assert_eq!(
        command(
            &mut manager,
            "deferred-provider",
            "service-reply-deferred",
            json!("later result")
        ),
        "Replied"
    );
    manager.poll();
    let completed: plugin_runtime::plugin_protocol::api::RequestUpdate<Value> =
        serde_json::from_str(&text(&manager, "service-consumer")).unwrap();
    assert!(
        matches!(completed, plugin_runtime::plugin_protocol::api::RequestUpdate::Completed { result: Ok(value) } if value == json!("later result"))
    );
    assert!(
        command(
            &mut manager,
            "deferred-provider",
            "service-replay-reply",
            json!("duplicate")
        )
        .contains("InvalidHandle")
    );
}

/// Reply authority comes from the retained provider invocation, not a consumer's services.call grant.
#[test]
#[ignore = "build capability-example through the current public SDK first"]
fn a_pure_provider_can_complete_its_deferred_reply_without_consumer_authority() {
    let temp = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(temp.path().join("plugins"), Environment::default()).unwrap();
    install(
        &mut manager,
        service_packages::pure_provider("pure-provider"),
    );
    install(
        &mut manager,
        package("service-consumer", false, false, "^1"),
    );
    command(
        &mut manager,
        "service-consumer",
        "service-open",
        json!("example.echo"),
    );
    call(&mut manager, "defer", json!("pending"), 30000);
    manager.poll();
    assert_eq!(
        command(
            &mut manager,
            "pure-provider",
            "service-reply-deferred",
            json!("done")
        ),
        "Replied"
    );
    manager.poll();
    assert!(text(&manager, "service-consumer").contains("done"));
}

/// A delegated event from B cannot consume the invocation retained for source A.
#[test]
#[ignore = "build capability-example through the current public SDK first"]
fn a_deferred_reply_cannot_borrow_another_sources_invocation() {
    let temp = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(temp.path().join("plugins"), Environment::default()).unwrap();
    for (id, provider) in [
        ("deferred-provider", true),
        ("service-consumer", false),
        ("other-consumer", false),
    ] {
        install(
            &mut manager,
            package(id, provider, false, if provider { "1.0.0" } else { "^1" }),
        );
    }
    command(
        &mut manager,
        "service-consumer",
        "service-open",
        json!("example.echo"),
    );
    command(
        &mut manager,
        "other-consumer",
        "service-open",
        json!("example.echo"),
    );
    call(&mut manager, "defer", json!("pending"), 30000);
    manager.poll();
    command(
        &mut manager,
        "other-consumer",
        "service-call",
        json!({"method":"reply-retained","value":"from B"}),
    );
    manager.poll();
    assert!(
        text(&manager, "other-consumer").contains("PermissionDenied"),
        "another source must not complete A's invocation"
    );
    assert_eq!(
        command(
            &mut manager,
            "deferred-provider",
            "service-reply-deferred",
            json!("from A")
        ),
        "Replied"
    );
    manager.poll();
    assert!(text(&manager, "service-consumer").contains("from A"));
}

/// Late results, invalid schemas and retired sources cannot reopen a completed consumer task.
#[test]
#[ignore = "build capability-example through the current public SDK first"]
fn deferred_replies_release_their_slots_on_cancel_timeout_and_source_retirement() {
    use plugin_runtime::plugin_protocol::api::RequestUpdate;
    let temp = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(temp.path().join("plugins"), Environment::default()).unwrap();
    install(
        &mut manager,
        package("deferred-provider", true, false, "1.0.0"),
    );
    install(
        &mut manager,
        package("service-consumer", false, false, "^1"),
    );
    command(
        &mut manager,
        "service-consumer",
        "service-open",
        json!("example.echo"),
    );
    let resources = manager.live["deferred-provider"].resource_count();
    call(&mut manager, "defer", json!("pending"), 30000);
    manager.poll();
    assert!(
        command(
            &mut manager,
            "deferred-provider",
            "service-reply-deferred",
            json!(42)
        )
        .contains("InvalidRequest")
    );
    assert_eq!(
        manager.live["deferred-provider"].resource_count(),
        resources + 1
    );
    assert_eq!(
        command(
            &mut manager,
            "deferred-provider",
            "service-reply-deferred",
            json!("corrected")
        ),
        "Replied"
    );
    manager.poll();
    assert_eq!(
        manager.live["deferred-provider"].resource_count(),
        resources
    );
    assert!(
        command(
            &mut manager,
            "deferred-provider",
            "service-replay-reply",
            json!("duplicate")
        )
        .contains("InvalidHandle")
    );
    for timeout in [30000, 50] {
        call(&mut manager, "defer", json!("pending"), timeout);
        manager.poll();
        if timeout == 30000 {
            command(
                &mut manager,
                "service-consumer",
                "service-cancel",
                Value::Null,
            );
        } else {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        for _ in 0..3 {
            manager.poll();
        }
        let terminal = text(&manager, "service-consumer");
        let result: RequestUpdate<Value> = serde_json::from_str(&terminal).unwrap();
        assert!(matches!(result, RequestUpdate::Cancelled { .. }));
        assert_eq!(
            manager.live["deferred-provider"].resource_count(),
            resources
        );
        assert!(
            command(
                &mut manager,
                "deferred-provider",
                "service-replay-reply",
                json!("late")
            )
            .contains("InvalidHandle")
        );
        manager.poll();
        assert_eq!(text(&manager, "service-consumer"), terminal);
    }
    call(&mut manager, "defer", json!("pending"), 30000);
    manager.poll();
    manager.disable("service-consumer").unwrap();
    manager.poll();
    assert_eq!(
        manager.live["deferred-provider"].resource_count(),
        resources
    );
    manager.enable("service-consumer").unwrap();
    demand(&mut manager, "service-consumer").unwrap();
    command(
        &mut manager,
        "service-consumer",
        "service-open",
        json!("example.echo"),
    );
    call(&mut manager, "defer", json!("new incarnation"), 30000);
    manager.poll();
    let pending: RequestUpdate<Value> =
        serde_json::from_str(&text(&manager, "service-consumer")).unwrap();
    assert!(
        !pending.is_terminal(),
        "source retirement must notify the provider to release its local slot: {pending:?}"
    );
    assert_eq!(
        command(
            &mut manager,
            "deferred-provider",
            "service-reply-deferred",
            json!("new result")
        ),
        "Replied"
    );
    manager.poll();
    assert_eq!(
        manager.live["deferred-provider"].resource_count(),
        resources
    );
}

/// Switching a host choice changes new references, never the meaning of an existing opaque reference.
#[test]
#[ignore = "build capability-example through the public SDK first"]
fn unique_and_selected_providers_are_interchangeable_and_old_references_expire() {
    let temp = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(temp.path().join("plugins"), Environment::default()).unwrap();
    install(&mut manager, package("z-provider-a", true, false, "1.0.0"));
    install(
        &mut manager,
        package("service-consumer", false, false, "^1"),
    );
    assert_eq!(
        command(
            &mut manager,
            "service-consumer",
            "service-open",
            json!("example.echo")
        ),
        "Service opened"
    );
    assert_eq!(
        call(&mut manager, "echo", json!("hello"), 30000),
        "Accepted"
    );
    manager.poll();
    assert!(text(&manager, "service-consumer").contains("z-provider-a:hello"));
    install(&mut manager, package("z-provider-b", true, false, "1.1.0"));
    assert!(call(&mut manager, "echo", json!("stale"), 30000).contains("InvalidHandle"));
    assert!(
        command(
            &mut manager,
            "service-consumer",
            "service-open",
            json!("example.echo")
        )
        .contains("Conflict")
    );
    manager
        .set_service_provider(
            plugin_runtime::plugin_protocol::api::InstanceScope::Workspace,
            Scope::User,
            "example.echo",
            Some("z-provider-b"),
        )
        .unwrap();
    assert_eq!(
        command(
            &mut manager,
            "service-consumer",
            "service-open",
            json!("example.echo")
        ),
        "Service opened"
    );
    call(&mut manager, "echo", json!("hello"), 30000);
    manager.poll();
    assert!(text(&manager, "service-consumer").contains("z-provider-b:hello"));
    // Returning to the same provider cannot resurrect references invalidated by an earlier choice.
    for provider in ["z-provider-a", "z-provider-b"] {
        manager
            .set_service_provider(
                plugin_runtime::plugin_protocol::api::InstanceScope::Workspace,
                Scope::User,
                "example.echo",
                Some(provider),
            )
            .unwrap();
    }
    assert!(call(&mut manager, "echo", json!("resurrected"), 30000).contains("InvalidHandle"));
    command(
        &mut manager,
        "service-consumer",
        "service-open",
        json!("example.echo"),
    );
    // Exit settles queued calls before any provider callback and leaves the consumer alive.
    call(&mut manager, "echo", json!("pending"), 30000);
    manager.disable("z-provider-b").unwrap();
    manager.poll();
    assert!(text(&manager, "service-consumer").contains("invalid_handle"));
    manager.enable("z-provider-b").unwrap();
    assert!(call(&mut manager, "echo", json!("old reference"), 30000).contains("InvalidHandle"));
    assert_eq!(
        manager.service_choices()[0].selected.as_deref(),
        Some("z-provider-b")
    );
    drop(manager);
    let mut manager = Manager::open(temp.path().join("plugins"), Environment::default()).unwrap();
    assert!(
        manager
            .published_entries()
            .iter()
            .any(|entry| entry.manifest.id == "service-consumer" && entry.enabled),
        "required consumer remains discoverable independently of its later-sorted cold provider"
    );
    assert!(
        manager.live.is_empty(),
        "reopen is static discovery, not a demand"
    );
    assert_eq!(
        manager.service_choices()[0].selected.as_deref(),
        Some("z-provider-b")
    );
}

/// Missing required dependencies stop activation; optional or incompatible services fail explicitly at open.
#[test]
#[ignore = "build capability-example through the public SDK first"]
fn missing_and_incompatible_services_do_not_activate_required_consumers() {
    let temp = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(temp.path().join("plugins"), Environment::default()).unwrap();
    let required = package("required-consumer", false, false, "^1");
    manager
        .install(&required, required.manifest.permissions.clone())
        .unwrap();
    assert!(
        demand(&mut manager, "required-consumer").is_err(),
        "required dependency fails at the actual demand"
    );
    assert!(manager.installed.contains_key("required-consumer"));
    assert!(!manager.live.contains_key("required-consumer"));
    install(&mut manager, package("service-consumer", false, true, "^1"));
    assert!(
        command(
            &mut manager,
            "service-consumer",
            "service-open",
            json!("example.echo")
        )
        .contains("CapabilityUnavailable")
    );
    install(&mut manager, package("provider-v2", true, false, "2.0.0"));
    assert!(
        command(
            &mut manager,
            "service-consumer",
            "service-open",
            json!("example.echo")
        )
        .contains("CapabilityUnavailable")
    );
    assert!(
        demand(&mut manager, "required-consumer").is_err(),
        "incompatible dependencies cannot become a live provider"
    );
}

/// Reuse the common request gate for cancellation, expiry, typed results and isolated provider failure.
#[test]
#[ignore = "build capability-example through the public SDK first"]
fn service_calls_preserve_source_and_cannot_borrow_provider_authority_or_reenter() {
    let temp = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(temp.path().join("plugins"), Environment::default()).unwrap();
    install(&mut manager, package("provider-a", true, false, "1.0.0"));
    install(
        &mut manager,
        package("service-consumer", false, false, "^1"),
    );
    command(
        &mut manager,
        "service-consumer",
        "service-open",
        json!("example.echo"),
    );
    assert!(call(&mut manager, "echo", json!(42), 30000).contains("InvalidRequest"));
    call(&mut manager, "echo", json!("cancel"), 30000);
    assert!(
        command(
            &mut manager,
            "service-consumer",
            "service-cancel",
            Value::Null
        )
        .contains("NotExecuted")
    );
    manager.poll();
    assert!(text(&manager, "service-consumer").contains("Cancelled"));
    call(&mut manager, "echo", json!("timeout"), 1);
    std::thread::sleep(std::time::Duration::from_millis(10));
    manager.poll();
    assert!(text(&manager, "service-consumer").contains("timed_out"));
    for operation in [
        json!({"method":"open_workspace"}),
        json!({"method":"open_data"}),
    ] {
        call(&mut manager, "probe", json!(operation.to_string()), 30000);
        manager.poll();
        assert!(text(&manager, "service-consumer").contains("permission_denied"));
    }
    call(&mut manager, "source", json!(""), 30000);
    manager.poll();
    assert!(text(&manager, "service-consumer").contains("service-consumer|"));
    call(&mut manager, "cycle", json!(""), 30000);
    manager.poll();
    assert!(text(&manager, "service-consumer").contains("Conflict"));
    call(&mut manager, "bad-result", json!(""), 30000);
    manager.poll();
    assert!(text(&manager, "service-consumer").contains("invalid_request"));
    call(&mut manager, "trap", json!(""), 30000);
    manager.poll();
    assert!(
        text(&manager, "service-consumer").contains("operation_failed"),
        "{}",
        text(&manager, "service-consumer")
    );
    assert!(manager.live["provider-a"].views.is_empty());
    assert!(!manager.live["service-consumer"].views.is_empty());
    manager.uninstall("service-consumer", true).unwrap();
    manager.uninstall("provider-a", true).unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// Delegated resources can be released, continuations stay restricted, and source exit seals native work.
#[test]
#[ignore = "build capability-example through the public SDK first"]
fn delegated_resources_and_async_continuations_follow_the_original_source_lifetime() {
    use plugin_runtime::plugin_protocol::api;
    let temp = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        temp.path().join("plugins"),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    install(&mut manager, package("provider-a", true, false, "1.0.0"));
    install(
        &mut manager,
        package("service-consumer", false, false, "^1"),
    );
    command(
        &mut manager,
        "service-consumer",
        "service-open",
        json!("example.echo"),
    );
    let resources = manager.live["provider-a"].resource_count();
    call(&mut manager, "read-close", json!(""), 30000);
    manager.poll();
    assert!(text(&manager, "service-consumer").contains("Resources released"));
    assert_eq!(manager.live["provider-a"].resource_count(), resources);
    call(&mut manager, "editor-cancel", json!(""), 30000);
    manager.poll();
    assert!(text(&manager, "service-consumer").contains("NotExecuted"));
    assert_eq!(manager.live["provider-a"].resource_count(), resources);
    call(&mut manager, "editor-continuation", json!(""), 30000);
    manager.poll();
    let request = manager
        .live
        .get_mut("provider-a")
        .unwrap()
        .take_editor_requests()
        .pop()
        .unwrap();
    assert!(request.begin());
    request.finish(Ok(api::EditorValue::Directory { path: "src".into() }));
    let completed_authority = request.authority().clone();
    manager.poll();
    assert!(text(&manager, "provider-a").contains("PermissionDenied"));
    // A completed native resource can survive its transient task deadline, but not its source.
    request.expire(std::time::Instant::now() + std::time::Duration::from_secs(60));
    assert!(completed_authority.is_live());
    call(&mut manager, "editor-continuation", json!(""), 30000);
    manager.poll();
    let request = manager
        .live
        .get_mut("provider-a")
        .unwrap()
        .take_editor_requests()
        .pop()
        .unwrap();
    assert!(request.begin());
    manager.disable("service-consumer").unwrap();
    assert!(
        !completed_authority.is_live(),
        "retiring the original caller revokes completed native resources"
    );
    assert!(
        !request.enter_side_effect(),
        "revoked source cannot execute already-published editor work"
    );
    assert!(matches!(
        request.status(),
        api::RequestUpdate::Cancelled { .. }
    ));
    request.finish(Ok(api::EditorValue::Directory {
        path: "late".into(),
    }));
    manager.poll();
    assert_eq!(manager.live["provider-a"].resource_count(), resources);
    // Project-only disable must revoke previously published work without waiting for a poll.
    manager
        .set_project_enabled("service-consumer", true)
        .unwrap();
    // Enabling retains a cold installation; the status view supplies this scenario's next demand.
    demand(&mut manager, "service-consumer").unwrap();
    command(
        &mut manager,
        "service-consumer",
        "service-open",
        json!("example.echo"),
    );
    call(&mut manager, "editor-continuation", json!(""), 30000);
    manager.poll();
    let request = manager
        .live
        .get_mut("provider-a")
        .unwrap()
        .take_editor_requests()
        .pop()
        .unwrap();
    assert!(request.begin());
    manager
        .set_project_enabled("service-consumer", false)
        .unwrap();
    assert!(!request.enter_side_effect());
    manager.poll();
    // Configuration hot replacement retires the previous source incarnation through the same gate.
    manager
        .set_project_enabled("service-consumer", true)
        .unwrap();
    // Re-enabling changes availability without eagerly creating a replacement instance.
    demand(&mut manager, "service-consumer").unwrap();
    command(
        &mut manager,
        "service-consumer",
        "service-open",
        json!("example.echo"),
    );
    call(&mut manager, "editor-continuation", json!(""), 30000);
    manager.poll();
    let request = manager
        .live
        .get_mut("provider-a")
        .unwrap()
        .take_editor_requests()
        .pop()
        .unwrap();
    assert!(request.begin());
    manager
        .update_setting(
            "service-consumer",
            Scope::User,
            "label",
            Some(json!("new instance")),
        )
        .unwrap();
    assert!(!request.enter_side_effect());
    manager.poll();
    assert_eq!(manager.live["provider-a"].resource_count(), resources);
    // A fresh delegated request also retains the provider's own instance boundary.
    // The fixture keeps the legacy Restart policy, so its new instance also needs an explicit demand.
    demand(&mut manager, "service-consumer").unwrap();
    command(
        &mut manager,
        "service-consumer",
        "service-open",
        json!("example.echo"),
    );
    call(&mut manager, "editor-continuation", json!(""), 30000);
    manager.poll();
    let request = manager
        .live
        .get_mut("provider-a")
        .unwrap()
        .take_editor_requests()
        .pop()
        .unwrap();
    assert!(request.begin());
    request.finish(Ok(api::EditorValue::Directory { path: "src".into() }));
    let provider_authority = request.authority().clone();
    manager.poll();
    assert!(provider_authority.is_live());
    manager.disable("provider-a").unwrap();
    assert!(
        !provider_authority.is_live(),
        "retiring the provider revokes completed native resources"
    );
}
