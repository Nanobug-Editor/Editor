//! Actual SDK packages separate static availability from demand-owned business execution.
use plugin_runtime::{
    DemandReason, Manager, Package,
    plugin_protocol::{Environment, api},
};

/// The previously released protocol-7 guest has no activation declaration; it also defaults to demand.
#[test]
#[ignore = "requires the SDK-built capability-example ZIP in target/plugin-api-test"]
fn installation_enable_and_reopen_keep_old_sdk_business_dormant() {
    let profile = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let environment = Environment {
        workspace: workspace.path().display().to_string(),
        ..Default::default()
    };
    let package = Package::read(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/plugin-api-test/capability-example.zip"),
    )
    .unwrap();
    let id = package.manifest.id.clone();
    let mut manager = Manager::open(profile.path().to_owned(), environment.clone()).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let published = manager.published_entries();
    assert!(published[0].enabled);
    assert!(
        published[0]
            .manifest
            .commands
            .iter()
            .any(|command| command.id == "echo")
    );
    assert!(
        published[0]
            .manifest
            .panels
            .iter()
            .any(|panel| panel.id == "welcome")
    );
    assert!(
        manager.instance_id(&id).is_none(),
        "installation must not activate business"
    );
    assert_eq!(
        manager.resource_count(),
        0,
        "cold metadata owns no running resources"
    );

    // Enabling records preference, and reopening republishes entries; neither is a demand.
    manager.disable(&id).unwrap();
    manager.enable(&id).unwrap();
    assert!(manager.instance_id(&id).is_none());
    drop(manager);
    let manager = Manager::open(profile.path().to_owned(), environment).unwrap();
    assert!(manager.published_entries()[0].enabled);
    assert!(manager.instance_id(&id).is_none());
    assert_eq!(manager.resource_count(), 0);
}

/// Two consumers share first birth, while withdrawing one original wait cannot cancel the other.
#[test]
#[ignore = "requires the SDK-built capability-example ZIP in target/plugin-api-test"]
fn first_command_demand_coalesces_without_changing_static_identity() {
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
    let package = Package::read(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/plugin-api-test/capability-example.zip"),
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let id = &package.manifest.id;
    let original = manager.availability(id).unwrap();
    let withdrawn = manager
        .begin_demand(&original, DemandReason::Command("echo".into()), 120_000)
        .unwrap();
    let waiting = manager
        .begin_demand(&original, DemandReason::Command("echo".into()), 120_000)
        .unwrap();
    assert!(
        manager.instance_id(id).is_none(),
        "enqueueing does not run business on the caller"
    );
    assert!(matches!(
        withdrawn.cancel().unwrap(),
        api::CancellationEffect::NotExecuted
    ));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let receipt = loop {
        manager.poll();
        match waiting.status() {
            api::RequestUpdate::Completed { result } => break result.unwrap(),
            api::RequestUpdate::Cancelled { reason, .. } => {
                panic!("unexpected cancellation {reason:?}")
            }
            _ => {}
        }
        assert!(
            std::time::Instant::now() < deadline,
            "first demand must finish"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert!(receipt.owner().is_some());
    assert_eq!(manager.instance_id(id), receipt.owner());
    assert_eq!(
        manager.availability(id).unwrap(),
        original,
        "first birth does not revoke static intent"
    );
    manager.admit_receipt(&original, &receipt).unwrap();
    manager.disable(id).unwrap();
    manager.enable(id).unwrap();
    assert!(
        manager.admit_receipt(&original, &receipt).is_err(),
        "disable/enable cannot revive a receipt"
    );
    assert_ne!(manager.availability(id).unwrap(), original);
}

/// Real-package waits share one budget, and withdrawing/expiring all of them keeps business dormant.
#[test]
#[ignore = "requires the SDK-built capability-example ZIP in target/plugin-api-test"]
fn pending_waiter_budget_deadlines_and_scope_aba_never_activate_abandoned_intent() {
    let profile = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let environment = Environment {
        workspace: workspace.path().display().to_string(),
        ..Default::default()
    };
    let package = Package::read(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/plugin-api-test/capability-example.zip"),
    )
    .unwrap();
    let mut manager = Manager::open(profile.path().into(), environment.clone()).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let id = &package.manifest.id;
    let original = manager.availability(id).unwrap();
    let waits = (0..64)
        .map(|_| {
            manager
                .begin_demand(&original, DemandReason::Command("echo".into()), 10_000)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let exhausted = manager.begin_demand(&original, DemandReason::Command("echo".into()), 10_000);
    assert!(matches!(
        exhausted,
        Err(api::Failure {
            code: api::ErrorCode::LimitExceeded,
            ..
        })
    ));
    drop(waits);
    manager.poll();
    assert!(manager.instance_id(id).is_none());
    assert_eq!(manager.resource_count(), 0);

    let expired = manager
        .begin_demand(&original, DemandReason::View("welcome".into()), 1)
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    assert!(matches!(
        expired.status(),
        api::RequestUpdate::Cancelled {
            reason: api::ErrorCode::TimedOut,
            ..
        }
    ));
    manager.poll();
    assert!(manager.instance_id(id).is_none());

    // Reopening the exact same canonical path is a new scope lease rather than a reusable old token.
    let closing = manager
        .begin_demand(&original, DemandReason::Command("echo".into()), 10_000)
        .unwrap();
    manager.close_workspace(&environment.workspace).unwrap();
    manager.switch_workspace(environment, true).unwrap();
    manager.poll();
    assert!(closing.status().is_terminal());
    assert!(manager.instance_id(id).is_none());
    assert_ne!(manager.availability(id).unwrap(), original);
    assert_eq!(manager.resource_count(), 0);
}

/// Explicit continuous purpose uses the same first-birth queue; absence of it remains demand-only.
#[test]
#[ignore = "requires the SDK-built capability-example ZIP in target/plugin-api-test"]
fn explicit_background_purpose_activates_only_after_trusted_polling() {
    let original = Package::read(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/plugin-api-test/capability-example.zip"),
    )
    .unwrap();
    let mut files = original.files;
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["activation"] = serde_json::json!({ "background": { "purpose": "Maintain the explicitly enabled monitor" } });
    manifest["api"]["required"]["lifecycle.activation"] = serde_json::json!("^1");
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let package = Package::from_files(files).unwrap();
    let profile = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let environment = Environment {
        workspace: workspace.path().display().to_string(),
        ..Default::default()
    };
    let mut manager = Manager::open(profile.path().into(), environment).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    assert!(
        manager.instance_id(&package.manifest.id).is_none(),
        "installation still prepares without Activate"
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    while manager.instance_id(&package.manifest.id).is_none() {
        manager.poll();
        assert!(
            std::time::Instant::now() < deadline,
            "declared continuous purpose must become ready"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    manager.set_workspace_trust(false).unwrap();
    assert!(manager.instance_id(&package.manifest.id).is_none());
    manager.poll();
    assert_eq!(manager.resource_count(), 0);
}

/// An unavailable selected role exposes its error without invoking unrelated guest business activation.
#[test]
#[ignore = "requires the SDK-built capability-example ZIP in target/plugin-api-test"]
fn failed_selected_language_plan_does_not_activate_business_or_hide_its_error() {
    let original = Package::read(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/plugin-api-test/capability-example.zip"),
    )
    .unwrap();
    let mut files = original.files;
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["api"]["required"]["language.lsp"] = serde_json::json!("^1");
    manifest["api"]["required"]["process"] = serde_json::json!("^1");
    manifest["permissions"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!("process.service.unavailable"));
    manifest["services"]["unavailable"] = serde_json::json!({
        "program":"nanobug-test-missing-selected-service", "args":[],
    });
    manifest["language_servers"] = serde_json::json!([
        {"id":"analysis", "language":"independent", "service":"unavailable"},
    ]);
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    // The old actual guest reports an Activate fault if incorrectly reached, making that side effect observable.
    files.insert("migration-policy.txt".into(), b"activate-fail".to_vec());
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
    let token = manager.availability(id).unwrap();
    let request = manager
        .begin_demand(
            &token,
            DemandReason::Language {
                provider: key.clone(),
                role: plugin_runtime::LanguageRole::Lsp,
            },
            120_000,
        )
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    while !request.status().is_terminal() {
        manager.poll();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(matches!(
        request.status(),
        api::RequestUpdate::Completed { result: Err(_) }
    ));
    assert!(manager.instance_id(id).is_none());
    assert!(
        manager
            .language_services()
            .get(&key)
            .is_some_and(Result::is_err),
        "the selected plan error remains observable"
    );
    assert!(
        manager
            .runtime_logs()
            .records(id)
            .iter()
            .all(|record| record.source != "wasm/activate"),
        "failing discovery cannot invoke Activate merely to report its failure"
    );
    assert_eq!(manager.resource_count(), 0);
}
