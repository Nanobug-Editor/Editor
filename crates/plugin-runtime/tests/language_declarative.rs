//! Resource-only language services retain sealed lifecycle authority without allocating a guest WASM.
use plugin_runtime::{
    LanguageFixOrigin, Manager, Package,
    plugin_protocol::{
        Environment, api,
        diagnostics::Fix,
        workspace::{self, Change},
    },
};
use std::collections::BTreeMap;

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
    let original = manager.language_services()["resource-language/analysis"]
        .as_ref()
        .unwrap()
        .clone();
    let authority = original.authority().clone();
    assert!(authority.is_live());
    assert_eq!(
        original.capability("language.lsp").unwrap().to_string(),
        "1.4.0"
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
