//! Resource-only language services retain sealed lifecycle authority without allocating a guest WASM.
#[path = "support/demand.rs"]
mod demand_support;
use plugin_runtime::{
    DemandReason, LanguageFixOrigin, LanguageRole, Manager, Package,
    plugin_protocol::{
        Environment, api,
        diagnostics::Fix,
        workspace::{self, Change},
    },
};
use std::collections::BTreeMap;

/// Public native plan admission accepts whole cross-document proposals only with the original
/// workspace read grant; malformed late targets cannot allocate a partial editor request.
#[test]
fn native_cross_document_plans_require_workspace_grant_and_reject_invalid_suffix() {
    for readable in [false, true] {
        let workspace = tempfile::tempdir().unwrap();
        let profile = tempfile::tempdir().unwrap();
        let mut permissions = vec!["process.service.analysis", "editor.read", "editor.write"];
        if readable { permissions.push("workspace.read"); }
        let manifest = serde_json::json!({
            "id":"cross-source", "name":"Cross source", "version":"0.1.0", "protocol":7,
            "api":{"base":"^1","required":{"language.lsp":"^1.4","process":"^1","editor.edit":"^1.1","workspace.files":"^1.3"}},
            "permissions":permissions,"contributions":"plugin.toml","storage_limit":1024,
            "services":{"analysis":{"program":std::env::current_exe().unwrap(),"args":[]}},
            "language_servers":[{"id":"analysis","language":"unknown-cross-source","service":"analysis"}]
        });
        let package = Package::from_files(BTreeMap::from([
            ("manifest.json".into(), serde_json::to_vec(&manifest).unwrap()),
            ("plugin.toml".into(), b"[plugin]\nid='cross-source'\nname='Cross source'\nversion='0.1.0'\nhost_version='^0.1'\n".to_vec()),
        ])).unwrap();
        let mut manager = Manager::open(profile.path().into(), Environment { workspace:workspace.path().display().to_string(),..Default::default() }).unwrap();
        manager.install(&package, package.manifest.permissions.clone()).unwrap();
        demand_support::ready(&mut manager,"cross-source",DemandReason::Language { provider:"cross-source/analysis".into(),role:LanguageRole::Lsp });
        let owner = LanguageFixOrigin::Lsp(manager.language_services()["cross-source/analysis"].as_ref().unwrap().clone());
        let (document, mut fix) = native_proposal();
        let mut other = fix.plan.changes[0].clone();
        let Change::EditDocument { document: target,.. } = &mut other else { unreachable!() };
        target.id = "other-source".into();
        target.path = "other.source".into();
        fix.plan.changes.push(other);
        fix.plan.conditions = vec![document.clone(), api::DocumentVersion { id:"readonly-input".into(),path:"types.source".into(),revision:7 }];
        let result = manager.submit_language_fix(&owner,document.clone(),fix.clone());
        if readable {
            result.unwrap();
            let requests = manager.take_editor_requests();
            assert_eq!(requests.len(),1);
            let api::EditorOperation::Workspace { operation:workspace::Operation::ApplyPlan { plan } } = requests[0].1.operation() else { panic!("complete native plan") };
            assert_eq!(plan.changes.len(),2);
            assert_eq!(plan.conditions.len(),2);
            requests[0].1.cancel_from_host(api::CancelMode::TryTerminate);
        } else {
            assert_eq!(result.unwrap_err().code,api::ErrorCode::PermissionDenied);
            assert!(manager.take_editor_requests().is_empty());
        }
        let mut duplicate = fix.clone();
        duplicate.plan.conditions.push(document.clone());
        assert_eq!(manager.submit_language_fix(&owner,document.clone(),duplicate).unwrap_err().code,api::ErrorCode::InvalidRequest);
        assert!(manager.take_editor_requests().is_empty());
        let Change::EditDocument { document: target,.. } = &mut fix.plan.changes[1] else { unreachable!() };
        target.path = "../escape.source".into();
        assert_eq!(manager.submit_language_fix(&owner,document,fix).unwrap_err().code,api::ErrorCode::InvalidPath);
        assert!(manager.take_editor_requests().is_empty());
        manager.shutdown();
    }
}

/// Dropped background work is observable as a terminal request, and its reservation permits a new attempt.
#[test]
fn dropping_taken_or_finished_preparation_releases_original_wait_and_capacity() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let manifest = serde_json::json!({
        "id":"dropped-plan", "name":"Dropped plan", "version":"0.1.0", "protocol":7,
        "api":{"base":"^1", "required":{"language.lsp":"^1", "process":"^1"}},
        "contributions":"plugin.toml", "permissions":["process.service.analysis"], "storage_limit":1024,
        "services":{"analysis":{"program":std::env::current_exe().unwrap(),"args":[]}},
        "language_servers":[{"id":"analysis","language":"independent","service":"analysis"}]
    });
    let package = Package::from_files(BTreeMap::from([
        ("manifest.json".into(), serde_json::to_vec(&manifest).unwrap()),
        ("plugin.toml".into(), b"[plugin]\nid = \"dropped-plan\"\nname = \"Dropped plan\"\nversion = \"0.1.0\"\nhost_version = \"^0.1\"\n".to_vec()),
    ])).unwrap();
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
    let original = manager.availability("dropped-plan").unwrap();
    for finish_preparation in [false, true] {
        let request = manager
            .begin_demand(
                &original,
                DemandReason::Language {
                    provider: "dropped-plan/analysis".into(),
                    role: LanguageRole::Lsp,
                },
                10_000,
            )
            .unwrap();
        let job = manager.take_demand_preparation().unwrap();
        if finish_preparation {
            drop(job.run());
        } else {
            drop(job);
        }
        manager.poll();
        assert!(
            matches!(
                request.status(),
                api::RequestUpdate::Completed {
                    result: Err(api::Failure {
                        code: api::ErrorCode::OperationFailed,
                        ..
                    }),
                }
            ),
            "dropping owned preparation must not strand its original waiter"
        );
        assert!(manager.instance_id("dropped-plan").is_none());
        assert!(manager.language_services().is_empty());
        assert_eq!(manager.resource_count(), 0);
    }
    // Both abandoned jobs released their slots; an explicit later demand remains possible.
    demand_support::ready(
        &mut manager,
        "dropped-plan",
        DemandReason::Language {
            provider: "dropped-plan/analysis".into(),
            role: LanguageRole::Lsp,
        },
    );
    assert!(manager.language_services()["dropped-plan/analysis"].is_ok());
    manager.disable("dropped-plan").unwrap();
    manager.enable("dropped-plan").unwrap();
    let next = manager.availability("dropped-plan").unwrap();
    let shutdown_wait = manager
        .begin_demand(
            &next,
            DemandReason::Language {
                provider: "dropped-plan/analysis".into(),
                role: LanguageRole::Lsp,
            },
            10_000,
        )
        .unwrap();
    manager.shutdown();
    // Request observation cannot depend on another actor tick after that actor has shut down.
    assert!(matches!(
        shutdown_wait.status(),
        api::RequestUpdate::Cancelled {
            reason: api::ErrorCode::InvalidHandle,
            ..
        }
    ));
    assert_eq!(manager.resource_count(), 0);
}

/// Cold catalogs do not even prepare fixed native plans; one original selected role prepares only itself.
#[test]
fn cold_language_catalog_materializes_only_the_selected_role() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let manifest = serde_json::json!({
        "id":"cold-language", "name":"Cold language", "version":"0.1.0", "protocol":7,
        "api":{"base":"^1", "required":{"language.lsp":"^1", "language.formatting":"^1", "process":"^1"}},
        "contributions":"plugin.toml", "permissions":["process.service.analysis"], "storage_limit":1024,
        "services":{"analysis":{"program":std::env::current_exe().unwrap(),"args":[]}},
        "language_servers":[
            {"id":"analysis","language":"independent","service":"analysis"},
            {"id":"formatter","language":"independent","service":"analysis","primary":false,"formatting":true}
        ]
    });
    let package = Package::from_files(BTreeMap::from([
        ("manifest.json".into(), serde_json::to_vec(&manifest).unwrap()),
        ("plugin.toml".into(), b"[plugin]\nid = \"cold-language\"\nname = \"Cold language\"\nversion = \"0.1.0\"\nhost_version = \"^0.1\"\n".to_vec()),
    ])).unwrap();
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
    assert!(
        manager.language_services().is_empty(),
        "metadata discovery must remain cold"
    );
    let original = manager.availability("cold-language").unwrap();
    let request = manager
        .begin_demand(
            &original,
            DemandReason::Language {
                provider: "cold-language/formatter".into(),
                role: LanguageRole::Formatter,
            },
            10_000,
        )
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let receipt = loop {
        manager.poll();
        match request.status() {
            api::RequestUpdate::Completed { result } => break result.unwrap(),
            api::RequestUpdate::Cancelled { reason, .. } => panic!("unexpected {reason:?}"),
            _ => {}
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    };
    manager.admit_receipt(&original, &receipt).unwrap();
    assert!(
        receipt.owner().is_none(),
        "a native plan needs no fake lifecycle component"
    );
    let services = manager.language_services();
    assert_eq!(
        services.keys().cloned().collect::<Vec<_>>(),
        vec!["cold-language/formatter"]
    );
    assert!(services.values().all(Result::is_ok));
    assert_eq!(
        manager.resource_count(),
        0,
        "preparing a fixed plan does not launch its process"
    );
    manager.disable("cold-language").unwrap();
    assert!(manager.language_services().is_empty());
    assert!(manager.admit_receipt(&original, &receipt).is_err());
}

/// A public declarative package prepares a fixed native plan and revokes same-ID old lifetimes.
/// This test does not launch the executable; actual stdio/input behavior is exercised by the app fixtures.
#[test]
fn declarative_language_plan_needs_no_empty_wasm_and_retires_original_authority() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let executable = std::env::current_exe().unwrap();
    let manifest = serde_json::json!({
        "id":"resource-language", "name":"Resource language", "version":"0.1.0", "protocol":7,
        "api":{"base":"^1", "required":{"language.lsp":"^1.4", "process":"^1", "editor.edit":"^1.1", "workspace.files":"^1.3"}},
        "contributions":"plugin.toml", "permissions":["process.service.analysis", "editor.read", "editor.write"], "storage_limit":1024,
        "services":{"analysis":{"program":executable,"args":[]}},
        "language_servers":[{"id":"analysis","language":"independent","service":"analysis"}]
    });
    let package = Package::from_files(BTreeMap::from([
        ("manifest.json".into(), serde_json::to_vec(&manifest).unwrap()),
        ("plugin.toml".into(), b"[plugin]\nid = \"resource-language\"\nname = \"Resource language\"\nversion = \"0.1.0\"\nhost_version = \"^0.1\"\n".to_vec()),
    ])).unwrap();
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
    assert!(
        manager.live.is_empty(),
        "Resource-only admission creates no fake guest"
    );
    // An actual selected analyzer, rather than catalog discovery, owns preparation.
    demand_support::ready(
        &mut manager,
        "resource-language",
        DemandReason::Language {
            provider: "resource-language/analysis".into(),
            role: LanguageRole::Lsp,
        },
    );
    let original = manager.language_services()["resource-language/analysis"]
        .as_ref()
        .unwrap()
        .clone();
    let authority = original.authority().clone();
    assert!(authority.is_live());
    assert_eq!(
        original.capability("language.lsp").unwrap().to_string(),
        "1.5.0"
    );
    let requests = exercise_native_request_ownership(&mut manager, &original);
    manager.disable("resource-language").unwrap();
    assert_eq!(manager.resource_count(), 0);
    for request in requests {
        assert!(request.status().is_terminal());
        assert!(
            !request.begin(),
            "An already published request cannot enter after retirement"
        );
        assert!(
            !request
                .workspace_request()
                .unwrap()
                .batch()
                .unwrap()
                .is_live()
        );
    }
    assert!(!original.is_active());
    assert!(
        !authority.is_live(),
        "An old native seal cannot authorize late actions"
    );
    manager.enable("resource-language").unwrap();
    demand_support::ready(
        &mut manager,
        "resource-language",
        DemandReason::Language {
            provider: "resource-language/analysis".into(),
            role: LanguageRole::Lsp,
        },
    );
    let replacement = manager.language_services()["resource-language/analysis"]
        .as_ref()
        .unwrap()
        .clone();
    assert!(!std::sync::Arc::ptr_eq(&original, &replacement));
    assert!(replacement.authority().is_live());
    assert!(!authority.is_live());
    manager.uninstall("resource-language", false).unwrap();
    assert!(!replacement.is_active());
    assert!(!replacement.authority().is_live());

    // Shutdown must revoke published native handles before Manager Drop, even with no Main WASM.
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    demand_support::ready(
        &mut manager,
        "resource-language",
        DemandReason::Language {
            provider: "resource-language/analysis".into(),
            role: LanguageRole::Lsp,
        },
    );
    let closing = manager.language_services()["resource-language/analysis"]
        .as_ref()
        .unwrap()
        .clone();
    let (document, fix) = native_proposal();
    manager
        .submit_language_fix(&LanguageFixOrigin::Lsp(closing.clone()), document, fix)
        .unwrap();
    let published = manager.take_editor_requests().pop().unwrap().1;
    let batch = published
        .workspace_request()
        .unwrap()
        .batch()
        .unwrap()
        .clone();
    manager.shutdown();
    assert!(!closing.is_active());
    assert!(!closing.authority().is_live());
    assert!(published.status().is_terminal());
    assert!(!published.begin());
    assert!(!batch.is_live());
    assert_eq!(manager.resource_count(), 0);
}

/// The public actor transfers one owned plan exactly once. These checks exercise admission and
/// cancellation only; actual text, preview, confirmation and Undo are the separate native UI case.
fn exercise_native_request_ownership(
    manager: &mut Manager,
    service: &std::sync::Arc<plugin_runtime::LanguageService>,
) -> Vec<plugin_runtime::EditorRequest> {
    let (document, fix) = native_proposal();
    let origin = LanguageFixOrigin::Lsp(service.clone());
    let mut requests = Vec::new();
    for _ in 0..8 {
        let api::Value::BatchAccepted { request, batch } = manager
            .submit_language_fix(&origin, document.clone(), fix.clone())
            .unwrap()
        else {
            panic!("A native proposal has the ordinary request and retained batch handles")
        };
        let delivered = manager.take_editor_requests();
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].0, "resource-language");
        assert_eq!(delivered[0].1.handle(), &request);
        assert_eq!(
            delivered[0]
                .1
                .workspace_request()
                .unwrap()
                .batch()
                .unwrap()
                .handle(),
            &batch
        );
        assert!(
            delivered[0]
                .1
                .workspace_request()
                .unwrap()
                .can_edit_documents()
        );
        assert!(
            !delivered[0]
                .1
                .workspace_request()
                .unwrap()
                .can_read_documents()
        );
        assert!(
            delivered[0]
                .1
                .workspace_request()
                .unwrap()
                .files()
                .is_empty()
        );
        assert!(
            manager.take_editor_requests().is_empty(),
            "Delivery cannot duplicate an effect"
        );
        requests.push(delivered.into_iter().next().unwrap().1);
    }
    assert_eq!(manager.resource_count(), 16);
    assert_eq!(
        manager
            .submit_language_fix(&origin, document.clone(), fix.clone())
            .unwrap_err()
            .code,
        api::ErrorCode::LimitExceeded
    );
    let removed = requests.remove(0);
    let recovery = removed
        .workspace_request()
        .unwrap()
        .batch()
        .unwrap()
        .clone();
    removed.cancel_from_host(api::CancelMode::TryTerminate);
    manager.poll();
    assert_eq!(
        manager.resource_count(),
        15,
        "Terminal transient slots are reclaimed"
    );
    assert!(
        recovery.is_live(),
        "Cancelling the wait does not discard a retained receipt"
    );
    drop(removed);
    drop(recovery);
    manager.poll();
    assert_eq!(
        manager.resource_count(),
        14,
        "Last native holder releases its owner-only receipt"
    );
    manager.submit_language_fix(&origin, document, fix).unwrap();
    requests.extend(
        manager
            .take_editor_requests()
            .into_iter()
            .map(|(_, request)| request),
    );
    assert_eq!(requests.len(), 8);
    assert!(
        manager.live.is_empty(),
        "Neither quota recovery nor issue creates an empty guest"
    );
    requests
}

/// Shared admission input remains a complete public proposal rather than a fabricated request,
/// service seal, native mutation or Undo implementation.
fn native_proposal() -> (api::DocumentVersion, Fix) {
    let document = api::DocumentVersion {
        id: "native-source-incarnation".into(),
        path: "source.independent".into(),
        revision: 7,
    };
    let fix = Fix {
        title: "Native proposal".into(),
        plan: workspace::Plan {
            title: "Native proposal".into(),
            origin: Some(document.clone()),
            conditions: Vec::new(),
            changes: vec![Change::EditDocument {
                document: document.clone(),
                edits: vec![api::TextEdit {
                    range: api::TextRange { start: 0, end: 0 },
                    text: "new".into(),
                }],
                selection: api::TextRange { start: 3, end: 3 },
            }],
        },
    };
    (document, fix)
}
