//! Actual independent SDK guests prove pure direct features and stable collection ownership.
/// Fixed public SDK consumers are test-only inputs, never packaged default providers.
#[path = "support/packages.rs"]
mod frozen_packages;
use plugin_runtime::{
    Manager, Package, RequestCancellation,
    plugin_protocol::{Environment, api, diagnostics, direct, language::SourceSnapshot},
};
/// These exact-version artifacts were built through the public host and checked into the hash ledger.
fn package(id: &str) -> Package {
    frozen_packages::package(&format!("{id}-0.1.0"))
}

/// The normal command dispatch exposes ordinary protocol results without a testing host operation.
fn probe(
    manager: &mut Manager,
    id: &str,
    operation: api::Operation,
) -> Result<api::Value, api::Failure> {
    manager
        .invoke_command(id, "probe", serde_json::to_value(operation).unwrap())
        .unwrap();
    let plugin_runtime::plugin_protocol::ui::Kind::Text { text } =
        &manager.live[id].views.values().next().unwrap().root.kind
    else {
        panic!("The independent probe must publish its ordinary result")
    };
    serde_json::from_str(text).unwrap()
}

/// The caller grants only immutable text/version data to the pure callback, never a native handle.
fn source(text: &str) -> SourceSnapshot {
    SourceSnapshot {
        document: api::DocumentVersion {
            id: "direct-source".into(),
            path: "example.nbexpr".into(),
            revision: 7,
        },
        text: text.into(),
    }
}

/// Three feature results, typed cancellation and replacement use one actual pure provider lifetime.
#[test]
#[ignore = "opt-in fixed SDK/WASM packages; rebuild sources: crates/plugin-runtime/tests/fixtures/community; see fixtures/packages/README.md"]
fn independent_direct_features_cancel_and_retire_without_lsp() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let package = package("expression-language");
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
        "Direct features do not prepare any LSP"
    );
    let provider = manager.direct_language_providers()["expression-language/expressions"]
        .as_ref()
        .unwrap()
        .clone();
    let snapshot = source("sum(中文, 2)\n中文 + 中文\nadd(1, 2)");
    let direct::Result::References { locations } = provider
        .request(
            direct::Feature::References,
            snapshot.clone(),
            4,
            &RequestCancellation::default(),
        )
        .unwrap()
    else {
        panic!("References expected")
    };
    assert_eq!(locations.len(), 3);
    assert_eq!(
        locations[0].position,
        api::TextPosition {
            line: 0,
            character: 4
        }
    );
    assert!(locations.iter().all(|item| item.resource
        == api::ResourceIdentity::Local {
            path: snapshot.document.path.clone()
        }));
    let direct::Result::SignatureHelp { help: Some(help) } = provider
        .request(
            direct::Feature::SignatureHelp,
            snapshot.clone(),
            "sum(中文, ".len(),
            &RequestCancellation::default(),
        )
        .unwrap()
    else {
        panic!("Signature expected")
    };
    assert_eq!(help.active_parameter, Some(1));
    assert_eq!(help.signatures[0].parameters.len(), 2);
    let direct::Result::CodeActions { actions } = provider
        .request(
            direct::Feature::CodeActions,
            snapshot.clone(),
            0,
            &RequestCancellation::default(),
        )
        .unwrap()
    else {
        panic!("Actions expected")
    };
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].plan.origin.as_ref(), Some(&snapshot.document));
    assert!(
        matches!(&actions[0].plan.changes[0], plugin_runtime::plugin_protocol::workspace::Change::EditDocument { document, edits, .. } if document == &snapshot.document && edits[0].text == "sum")
    );
    let cancellation = RequestCancellation::default();
    cancellation.cancel();
    let error = provider
        .request(
            direct::Feature::References,
            snapshot.clone(),
            4,
            &cancellation,
        )
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<api::Failure>().unwrap().code,
        api::ErrorCode::Cancelled
    );
    assert!(
        provider
            .request(
                direct::Feature::References,
                snapshot.clone(),
                4,
                &RequestCancellation::default()
            )
            .is_ok(),
        "Cancellation must not poison the independent executor"
    );
    manager.disable("expression-language").unwrap();
    assert!(!provider.is_active());
    assert!(
        provider
            .request(
                direct::Feature::References,
                snapshot.clone(),
                4,
                &RequestCancellation::default()
            )
            .is_err()
    );
    manager.enable("expression-language").unwrap();
    let replacement = manager.direct_language_providers()["expression-language/expressions"]
        .as_ref()
        .unwrap()
        .clone();
    assert!(!std::sync::Arc::ptr_eq(&provider, &replacement));
    assert!(
        !provider.is_active(),
        "A same-ID replacement never restores the old sealed authority"
    );
    assert!(
        replacement
            .request(
                direct::Feature::References,
                snapshot,
                4,
                &RequestCancellation::default()
            )
            .is_ok()
    );
}

/// Independent collections reject foreign clear, revoke queued requests and enforce their own quota.
#[test]
#[ignore = "opt-in fixed SDK/WASM packages; rebuild sources: crates/plugin-runtime/tests/fixtures/community; see fixtures/packages/README.md"]
fn independent_diagnostic_handles_reject_foreign_clear_and_revoke_queued_publication() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    for id in ["spell-check", "config-check"] {
        let package = package(id);
        assert!(package.manifest.language_servers.is_empty());
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
    }
    let api::Value::Resource(handle) = probe(
        &mut manager,
        "spell-check",
        api::Operation::Diagnostics {
            operation: diagnostics::Operation::Create {
                label: "Independent owner / 独立来源".into(),
            },
        },
    )
    .unwrap() else {
        panic!("Owned collection expected")
    };
    let document = source("teh").document;
    let clear = api::EditorOperation::Diagnostics {
        publication: diagnostics::Publication::Clear {
            collection: handle.clone(),
            document,
        },
    };
    let operation = api::Operation::Editor {
        operation: clear,
        timeout_ms: 30_000,
    };
    assert_eq!(
        probe(&mut manager, "config-check", operation.clone())
            .unwrap_err()
            .code,
        api::ErrorCode::InvalidHandle
    );
    assert!(matches!(
        probe(&mut manager, "spell-check", operation).unwrap(),
        api::Value::Accepted(_)
    ));
    let request = manager
        .live
        .get_mut("spell-check")
        .unwrap()
        .take_editor_requests()
        .pop()
        .unwrap();
    assert_eq!(request.diagnostic_collection().unwrap().handle(), &handle);
    assert!(matches!(
        probe(
            &mut manager,
            "spell-check",
            api::Operation::CloseResource { handle }
        )
        .unwrap(),
        api::Value::Unit
    ));
    assert!(!request.diagnostic_collection().unwrap().is_live());
    assert!(
        !request.begin(),
        "Revoked queued publications cannot enter native effects"
    );
    assert!(
        matches!(request.status(), api::RequestUpdate::Completed { result: Err(error) } if error.code == api::ErrorCode::InvalidHandle)
    );
    // The initial business collection still owns one slot. Fifteen additional live roots exhaust it.
    for index in 0..15 {
        assert!(matches!(
            probe(
                &mut manager,
                "spell-check",
                api::Operation::Diagnostics {
                    operation: diagnostics::Operation::Create {
                        label: format!("collection-{index}")
                    }
                }
            )
            .unwrap(),
            api::Value::Resource(_)
        ));
    }
    assert_eq!(
        probe(
            &mut manager,
            "spell-check",
            api::Operation::Diagnostics {
                operation: diagnostics::Operation::Create {
                    label: "over quota".into()
                }
            }
        )
        .unwrap_err()
        .code,
        api::ErrorCode::LimitExceeded
    );
    manager.disable("spell-check").unwrap();
    assert!(!request.authority().is_live());
}
