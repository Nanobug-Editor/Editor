//! Native cross-document admission preserves the complete plan and rejects invalid suffixes atomically.
use super::*;

/// Public native plan admission accepts whole cross-document proposals only with the original
/// workspace read grant; malformed late targets cannot allocate a partial editor request.
#[test]
fn native_cross_document_plans_require_workspace_grant_and_reject_invalid_suffix() {
    for readable in [false, true] {
        let workspace = tempfile::tempdir().unwrap();
        let profile = tempfile::tempdir().unwrap();
        let mut manager = manager(profile.path(), workspace.path(), readable);
        let owner = LanguageFixOrigin::Lsp(
            manager.language_services()["cross-source/analysis"]
                .as_ref()
                .unwrap()
                .clone(),
        );
        let (document, mut fix) = native_proposal();
        let mut other = fix.plan.changes[0].clone();
        let Change::EditDocument {
            document: target, ..
        } = &mut other
        else {
            unreachable!()
        };
        target.id = "other-source".into();
        target.path = "other.source".into();
        fix.plan.changes.push(other);
        fix.plan.conditions = vec![
            document.clone(),
            api::DocumentVersion {
                id: "readonly-input".into(),
                path: "types.source".into(),
                revision: 7,
            },
        ];
        let result = manager.submit_language_fix(&owner, document.clone(), fix.clone());
        if readable {
            result.unwrap();
            let requests = manager.take_editor_requests();
            assert_eq!(requests.len(), 1);
            let api::EditorOperation::Workspace {
                operation: workspace::Operation::ApplyPlan { plan },
            } = requests[0].1.operation()
            else {
                panic!("complete native plan")
            };
            assert_eq!(plan.changes.len(), 2);
            assert_eq!(plan.conditions.len(), 2);
            requests[0]
                .1
                .cancel_from_host(api::CancelMode::TryTerminate);
        } else {
            assert_eq!(result.unwrap_err().code, api::ErrorCode::PermissionDenied);
            assert!(manager.take_editor_requests().is_empty());
        }
        let mut duplicate = fix.clone();
        duplicate.plan.conditions.push(document.clone());
        assert_eq!(
            manager
                .submit_language_fix(&owner, document.clone(), duplicate)
                .unwrap_err()
                .code,
            api::ErrorCode::InvalidRequest
        );
        assert!(manager.take_editor_requests().is_empty());
        let Change::EditDocument {
            document: target, ..
        } = &mut fix.plan.changes[1]
        else {
            unreachable!()
        };
        target.path = "../escape.source".into();
        assert_eq!(
            manager
                .submit_language_fix(&owner, document, fix)
                .unwrap_err()
                .code,
            api::ErrorCode::InvalidPath
        );
        assert!(manager.take_editor_requests().is_empty());
        manager.shutdown();
    }
}

/// Prepare an ordinary resource-only analyzer in `workspace` with isolated `profile` data.
/// `readable` adds only workspace read consent; the returned manager has no spawned process.
fn manager(profile: &std::path::Path, workspace: &std::path::Path, readable: bool) -> Manager {
    let mut permissions = vec!["process.service.analysis", "editor.read", "editor.write"];
    if readable {
        permissions.push("workspace.read");
    }
    let manifest = serde_json::json!({
        "id":"cross-source", "name":"Cross source", "version":"0.1.0", "protocol":7,
        "api":{"base":"^1","required":{"language.lsp":"^1.4","process":"^1","editor.edit":"^1.1","workspace.files":"^1.4"}},
        "permissions":permissions,"contributions":"plugin.toml","storage_limit":1024,
        "services":{"analysis":{"program":std::env::current_exe().unwrap(),"args":[]}},
        "language_servers":[{"id":"analysis","language":"unknown-cross-source","service":"analysis"}]
    });
    let package = Package::from_files(BTreeMap::from([
        ("manifest.json".into(), serde_json::to_vec(&manifest).unwrap()),
        ("plugin.toml".into(), b"[plugin]\nid='cross-source'\nname='Cross source'\nversion='0.1.0'\nhost_version='^0.1'\n".to_vec()),
    ])).unwrap();
    let mut manager = Manager::open(
        profile.into(),
        Environment {
            workspace: workspace.display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    demand_support::ready(
        &mut manager,
        "cross-source",
        DemandReason::Language {
            provider: "cross-source/analysis".into(),
            role: LanguageRole::Lsp,
        },
    );
    manager
}
