//! A renewed source waits for abandoned detached work without borrowing its publication authority.
use super::*;

/// Editing an original source can cancel cold preparation before the next source revision demands
/// the same provider. The new wait must retain its own budget and start only after the old job exits.
#[test]
fn a_new_source_waits_for_cancelled_preparation_to_retire() {
    renew_after_retirement(true);
}

/// A host worker may discard a preparation instead of returning it; its final lease still unblocks
/// the renewed source. No completion depends on a callback from the already dropped worker.
#[test]
fn dropping_abandoned_preparation_unblocks_a_new_source() {
    renew_after_retirement(false);
}

/// Both public worker completion paths preserve the same independent original-demand contract.
fn renew_after_retirement(returned: bool) {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("one.novel"), "original").unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    let package = fixed_package();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let availability = manager.availability("novel-contexts").unwrap();
    let document = DocumentContext {
        path: "one.novel".into(),
        generation: availability.configuration_generation(),
    };
    let reason = || DemandReason::LanguageDocument {
        provider: "novel-contexts/analysis".into(),
        role: LanguageRole::Lsp,
        document: document.clone(),
    };
    let original = manager
        .begin_demand(&availability, reason(), 10_000)
        .unwrap();
    let abandoned = manager.take_demand_preparation().unwrap();
    original.cancel().unwrap();
    let renewed = manager
        .begin_demand(&availability, reason(), 10_000)
        .expect("a current source waits instead of inheriting transient retirement failure");
    assert!(manager.take_demand_preparation().is_none());
    if returned {
        assert!(
            manager.commit_demand(abandoned.run()).is_err(),
            "the abandoned job cannot regain publication authority from the renewed source"
        );
    } else {
        drop(abandoned);
    }
    assert!(
        manager
            .language_document_service("novel-contexts/analysis", &document)
            .is_none()
    );
    let current = manager
        .take_demand_preparation()
        .expect("retirement releases the renewed source's independent preparation");
    manager.commit_demand(current.run()).unwrap();
    assert!(matches!(
        original.status(),
        plugin_runtime::RequestUpdate::Cancelled { .. }
    ));
    assert!(matches!(
        renewed.status(),
        plugin_runtime::RequestUpdate::Completed { result: Ok(_) }
    ));
    assert!(
        manager
            .language_document_service("novel-contexts/analysis", &document)
            .unwrap()
            .unwrap()
            .is_active()
    );
}

/// Deferred work uses the same global reservation budget and source-close cancellation as running
/// work; otherwise repeated edits could accumulate invisible waits or reactivate a closed document.
#[test]
fn deferred_source_waits_are_bounded_and_cancelled_when_the_source_closes() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("one.novel"), "original").unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    let package = fixed_package();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let availability = manager.availability("novel-contexts").unwrap();
    let document = DocumentContext {
        path: "one.novel".into(),
        generation: availability.configuration_generation(),
    };
    let reason = || DemandReason::LanguageDocument {
        provider: "novel-contexts/analysis".into(),
        role: LanguageRole::Lsp,
        document: document.clone(),
    };
    let original = manager
        .begin_demand(&availability, reason(), 10_000)
        .unwrap();
    let abandoned = manager.take_demand_preparation().unwrap();
    original.cancel().unwrap();
    let mut waiting = (0..64)
        .map(|_| {
            manager
                .begin_demand(&availability, reason(), 10_000)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let failure = manager
        .begin_demand(&availability, reason(), 10_000)
        .err()
        .unwrap();
    assert_eq!(
        failure.code,
        plugin_runtime::plugin_protocol::api::ErrorCode::LimitExceeded
    );
    waiting[0].cancel().unwrap();
    waiting.push(
        manager
            .begin_demand(&availability, reason(), 10_000)
            .unwrap(),
    );
    manager.release_language_document("novel-contexts/analysis", &document);
    for request in waiting {
        assert!(matches!(
            request.status(),
            plugin_runtime::RequestUpdate::Cancelled { .. }
        ));
    }
    drop(abandoned);
    assert!(manager.take_demand_preparation().is_none());
    assert!(manager.language_services().is_empty());
    assert!(manager.published_entries()[0].error.is_none());
}
