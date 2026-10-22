//! A completed native result is still checked at the final guest-notification boundary.
use super::*;
use plugin_protocol::{api::ResourceHandle, service::Schema};

/// Selection invalidation after completion rejects both a fresh status read and an already
/// collected notification, without rewriting the completed operation or revoking its owner.
#[test]
fn completed_control_success_is_rejected_after_its_source_choice_changes() {
    let owner = Arc::new(AtomicBool::new(true));
    let selection = Arc::new(AtomicBool::new(true));
    let call = EditorRequest::new(
        ResourceHandle {
            instance: "owner".into(),
            scope: "scope".into(),
            resource: 1,
        },
        EditorOperation::ReadSelection,
        String::new(),
        Default::default(),
        10_000,
        owner.clone(),
        None,
    )
    .with_language_control(LanguageControlMetadata {
        selected_context: None,
        selection_guard: Some(selection.clone()),
        contexts: Default::default(),
        collection_guards: Vec::new(),
        provider: "generic/engine".into(),
        owner: "owner".into(),
        consumer: "caller".into(),
        borrower: "caller".into(),
        scope: "scope".into(),
        binding_document: None,
        bindings: Default::default(),
        response: Schema::Boolean,
        imports: Vec::new(),
    });
    call.finish(Ok(EditorValue::Language(serde_json::json!(true))));
    let collected = call.update().1;
    assert!(matches!(
        &collected,
        RequestUpdate::Completed { result: Ok(_) }
    ));
    selection.store(false, Ordering::Release);
    for update in [call.status(), call.check_language_update(collected)] {
        assert!(matches!(
            update,
            RequestUpdate::Completed {
                result: Err(Failure {
                    code: ErrorCode::InvalidHandle,
                    ..
                })
            }
        ));
    }
    assert!(
        owner.load(Ordering::Acquire),
        "choice does not retire the shared process owner"
    );
    assert!(
        matches!(
            call.completion.status(),
            RequestUpdate::Completed { result: Ok(_) }
        ),
        "delivery rejection does not roll back the completed native operation"
    );
}

/// A successful refresh reports its own new choice, then rejects a late ACK if another choice
/// supersedes it. Its original CAS guard must not cancel its own successful commit.
#[test]
fn completed_refresh_ack_uses_its_post_commit_choice_epoch() {
    let workspace = tempfile::tempdir().unwrap();
    for name in ["left", "right"] {
        std::fs::create_dir(workspace.path().join(name)).unwrap();
    }
    let owner = Arc::new(AtomicBool::new(true));
    let registry = crate::LanguageContexts::default();
    let references = registry
        .register(
            "owner",
            "scope",
            workspace.path(),
            ["left", "right"]
                .map(|key| plugin_protocol::language_context::Descriptor {
                    provider: "engine".into(),
                    key: key.into(),
                    generation: 1,
                    project_root: key.into(),
                    data: serde_json::Value::Null,
                })
                .to_vec(),
            EditorAuthority::new(owner.clone(), None),
        )
        .unwrap();
    registry
        .select(&references[0].token, "owner", "scope", "a.ts")
        .unwrap();
    let target = registry
        .resolve(&references[1].token, "owner", "scope")
        .unwrap();
    let call = EditorRequest::new(
        ResourceHandle {
            instance: "owner".into(),
            scope: "scope".into(),
            resource: 2,
        },
        EditorOperation::RefreshLanguageContext {
            provider: "analysis".into(),
            document: plugin_protocol::api::DocumentVersion {
                id: "native-a".into(),
                path: "a.ts".into(),
                revision: 1,
            },
        },
        workspace.path().display().to_string(),
        Default::default(),
        10_000,
        owner,
        None,
    )
    .with_language_refresh(registry.clone(), target);
    call.commit_language_refresh().unwrap();
    call.finish(Ok(EditorValue::Unit));
    let collected = call.update().1;
    assert!(
        matches!(&collected, RequestUpdate::Completed { result: Ok(_) }),
        "the refresh cannot revoke its own success"
    );
    registry
        .select(&references[0].token, "owner", "scope", "a.ts")
        .unwrap();
    assert!(
        matches!(
            call.check_language_update(collected),
            RequestUpdate::Completed {
                result: Err(Failure {
                    code: ErrorCode::InvalidHandle,
                    ..
                })
            }
        ),
        "a late refresh ACK cannot advance the superseded preference intent"
    );
}
