//! Unknown-language packages exercise document-scoped plans through the public manager seam.
#[path = "language_contexts/compiled.rs"]
mod compiled;
#[path = "language_contexts/retirement.rs"]
mod retirement;
#[path = "support/demand.rs"]
mod demand_support;
use plugin_runtime::{
    DemandReason, LanguageRole, Manager, Package,
    plugin_protocol::{Environment, language::DocumentContext},
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};

/// Related membership has one public bounded readonly shape and cannot select a broader method.
#[test]
fn related_document_graph_requires_the_fixed_public_contract() {
    let original = fixed_package();
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&original.files["manifest.json"]).unwrap();
    manifest["api"]["required"]["language.control"] = serde_json::json!("^1");
    manifest["permissions"].as_array_mut().unwrap().extend([
        serde_json::json!("editor.read"),
        serde_json::json!("workspace.read"),
    ]);
    manifest["language_servers"][0]["related_documents"] =
        serde_json::json!("novel/project/documents");
    manifest["language_servers"][0]["control_methods"] = serde_json::json!({
        "novel/project/documents":plugin_runtime::plugin_protocol::language_context::related_documents_method()
    });
    let inspect = |manifest: &serde_json::Value| {
        let mut files = original.files.clone();
        files.insert(
            "manifest.json".into(),
            serde_json::to_vec(manifest).unwrap(),
        );
        Package::from_files(files)
    };
    assert!(inspect(&manifest).is_ok());
    manifest["language_servers"][0]["control_methods"]["novel/project/documents"]["result"] =
        serde_json::json!({"type":"null"});
    assert!(
        inspect(&manifest).is_err(),
        "membership cannot accept an unrelated result shape"
    );
}

/// Fixed declarations still bind documents explicitly, share one unchanged context and retire together.
#[test]
fn document_demands_reuse_a_context_without_merging_document_identities() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("one.novel"), "one").unwrap();
    std::fs::write(workspace.path().join("two.novel"), "two").unwrap();
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
    let documents = [
        DocumentContext {
            path: "one.novel".into(),
            generation: 1,
        },
        DocumentContext {
            path: "two.novel".into(),
            generation: 1,
        },
    ];
    for document in &documents {
        demand_support::ready(
            &mut manager,
            "novel-contexts",
            DemandReason::LanguageDocument {
                provider: "novel-contexts/analysis".into(),
                role: LanguageRole::Lsp,
                document: document.clone(),
            },
        );
    }
    let first = manager
        .language_document_service("novel-contexts/analysis", &documents[0])
        .unwrap()
        .unwrap();
    let second = manager
        .language_document_service("novel-contexts/analysis", &documents[1])
        .unwrap()
        .unwrap();
    assert!(
        Arc::ptr_eq(&first, &second),
        "one compatible context owns one transport"
    );
    assert_eq!(
        manager.language_document_service_key("novel-contexts/analysis", &documents[0]),
        manager.language_document_service_key("novel-contexts/analysis", &documents[1])
    );
    assert_eq!(
        manager
            .language_document_context("novel-contexts/analysis", &documents[0])
            .unwrap()
            .key,
        "workspace"
    );
    manager.disable("novel-contexts").unwrap();
    assert!(!first.is_active());
    assert!(
        manager
            .language_document_service("novel-contexts/analysis", &documents[0])
            .is_none()
    );
}

/// A newer configuration revokes an older wait before its detached result can reach publication.
#[test]
fn late_context_preparation_cannot_replace_a_newer_document_intent() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("one.novel"), "one").unwrap();
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
    let original = manager.availability("novel-contexts").unwrap();
    let first = DocumentContext {
        path: "one.novel".into(),
        generation: 1,
    };
    let next = DocumentContext {
        path: "one.novel".into(),
        generation: 2,
    };
    let old_wait = manager
        .begin_demand(
            &original,
            DemandReason::LanguageDocument {
                provider: "novel-contexts/analysis".into(),
                role: LanguageRole::Lsp,
                document: first.clone(),
            },
            10_000,
        )
        .unwrap();
    let old_prepared = manager.take_demand_preparation().unwrap().run();
    let new_wait = manager
        .begin_demand(
            &original,
            DemandReason::LanguageDocument {
                provider: "novel-contexts/analysis".into(),
                role: LanguageRole::Lsp,
                document: next.clone(),
            },
            10_000,
        )
        .unwrap();
    assert!(matches!(
        old_wait.status(),
        plugin_runtime::RequestUpdate::Cancelled {
            reason: plugin_runtime::plugin_protocol::api::ErrorCode::InvalidHandle,
            ..
        }
    ));
    manager.commit_demand(old_prepared).unwrap();
    let new_prepared = manager.take_demand_preparation().unwrap().run();
    manager.commit_demand(new_prepared).unwrap();
    assert!(matches!(
        new_wait.status(),
        plugin_runtime::RequestUpdate::Completed { result: Ok(_) }
    ));
    assert!(
        manager
            .language_document_service("novel-contexts/analysis", &first)
            .is_none()
    );
    assert!(
        manager
            .language_document_service("novel-contexts/analysis", &next)
            .unwrap()
            .is_ok()
    );
}

/// Abandoned first preparation preserves installed recognition and permits a later real demand.
#[test]
fn cancelled_first_preparation_keeps_static_catalog_and_can_prepare_again() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("one.novel"), "one").unwrap();
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
        generation: 1,
    };
    let reason = || DemandReason::LanguageDocument {
        provider: "novel-contexts/analysis".into(),
        role: LanguageRole::Lsp,
        document: document.clone(),
    };
    let wait = manager
        .begin_demand(&availability, reason(), 10_000)
        .unwrap();
    let prepared = manager.take_demand_preparation().unwrap().run();
    wait.cancel().unwrap();
    assert!(
        manager.commit_demand(prepared).is_err(),
        "the original cancelled result cannot publish"
    );
    let entry = manager
        .published_entries()
        .into_iter()
        .find(|entry| entry.manifest.id == "novel-contexts")
        .unwrap();
    assert!(
        entry.error.is_none(),
        "cancelling one document must not hide installed recognition: {:?}",
        entry.error
    );
    assert_eq!(
        manager.availability("novel-contexts").unwrap(),
        availability
    );
    demand_support::ready(&mut manager, "novel-contexts", reason());
    assert!(
        manager
            .language_document_service("novel-contexts/analysis", &document)
            .unwrap()
            .unwrap()
            .is_active()
    );
}

/// A cancelled source cannot borrow another source's live waiter to publish its own prepared role.
#[test]
fn cancelled_document_is_not_published_by_another_waiter() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    for name in ["one.novel", "two.novel"] {
        std::fs::write(workspace.path().join(name), name).unwrap();
    }
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
    let original = manager.availability("novel-contexts").unwrap();
    let first = DocumentContext {
        path: "one.novel".into(),
        generation: 1,
    };
    let second = DocumentContext {
        path: "two.novel".into(),
        generation: 1,
    };
    let waits = [&first, &second].map(|document| {
        manager
            .begin_demand(
                &original,
                DemandReason::LanguageDocument {
                    provider: "novel-contexts/analysis".into(),
                    role: LanguageRole::Lsp,
                    document: document.clone(),
                },
                10_000,
            )
            .unwrap()
    });
    let prepared = manager.take_demand_preparation().unwrap().run();
    waits[0].cancel().unwrap();
    manager.commit_demand(prepared).unwrap();
    assert!(
        manager
            .language_document_service("novel-contexts/analysis", &first)
            .is_none()
    );
    assert!(
        manager
            .language_document_service("novel-contexts/analysis", &second)
            .unwrap()
            .is_ok()
    );
    let service = manager
        .language_document_service("novel-contexts/analysis", &second)
        .unwrap()
        .unwrap();
    manager.release_language_document("novel-contexts/analysis", &second);
    assert!(
        !service.is_active(),
        "last live document releases the unique context lease"
    );
    assert!(
        manager.language_services().is_empty(),
        "cancelled reservations are reclaimed"
    );
}

/// Additional document languages and declared custom control methods are package-validated together.
#[test]
fn custom_control_contracts_reject_reserved_transport_and_write_authority() {
    let original = fixed_package();
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&original.files["manifest.json"]).unwrap();
    manifest["language_servers"][0]["languages"] = serde_json::json!(["novel-secondary"]);
    manifest["api"]["required"]["language.control"] = serde_json::json!("^1.0");
    manifest["language_servers"][0]["control_methods"] = serde_json::json!({
        "novel/project/resolve": { "parameters":{"type":"null"}, "result":{"type":"null"} }
    });
    let inspect = |manifest: &serde_json::Value| {
        let mut files = original.files.clone();
        files.insert(
            "manifest.json".into(),
            serde_json::to_vec(manifest).unwrap(),
        );
        Package::from_files(files)
    };
    assert!(inspect(&manifest).is_ok());
    for method in [
        "initialize",
        "shutdown",
        "exit",
        "textDocument/didOpen",
        "workspace/applyEdit",
        "$/cancelRequest",
    ] {
        let mut invalid = manifest.clone();
        invalid["language_servers"][0]["control_methods"] = serde_json::json!({
            method: { "parameters":{"type":"null"}, "result":{"type":"null"} }
        });
        assert!(inspect(&invalid).is_err(), "reserved {method}");
    }
    for permission in ["editor.write", "process.exec", "ui.panels"] {
        let mut invalid = manifest.clone();
        invalid["language_servers"][0]["control_methods"]["novel/project/resolve"]["permissions"] =
            serde_json::json!([permission]);
        assert!(
            inspect(&invalid).is_err(),
            "write or process authority cannot cross the control channel"
        );
    }
    manifest["language_servers"][0]["languages"] = serde_json::json!(["novel-project-language"]);
    assert!(
        inspect(&manifest).is_err(),
        "primary language must not be repeated"
    );
}

/// Selected document demand prepares fixed, verified private bytes; later offline demands reuse them.
#[test]
fn on_demand_language_tools_are_prepared_only_for_a_real_document() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let artifact = source.path().join("immutable-server.bin");
    // This test exercises dependency preparation, not process launch. The separate native fixture
    // executes a real stdio server; immutable dummy bytes here make the cache effect easy to observe.
    let payload = b"immutable preparation fixture";
    std::fs::write(&artifact, payload).unwrap();
    std::fs::write(workspace.path().join("one.novel"), "one").unwrap();
    let original = fixed_package();
    let mut files = original.files;
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["api"]["required"]["dependencies"] = serde_json::json!("^1.2");
    manifest["permissions"]
        .as_array_mut()
        .unwrap()
        .push("dependencies.prepare".into());
    manifest["services"]["analysis"] = serde_json::json!({
        "program":"novel-private-server", "on_demand":true, "installation": {
            "artifacts":[{"id":"native", "version":"fixed-1", "platform":format!("{}-{}",std::env::consts::OS,std::env::consts::ARCH),
                "sha256":format!("{:x}",Sha256::digest(payload)), "source":{"kind":"local","path":artifact},
                "format":{"kind":"file","path":"server.exe"}}], "executable":"native/server.exe"
        }
    });
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let package = Package::from_files(files).unwrap();
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
        !profile.path().join("dependencies/versions").exists(),
        "installing a provider must not prepare its on-demand tools"
    );
    let document = DocumentContext {
        path: "one.novel".into(),
        generation: 1,
    };
    let reason = || DemandReason::LanguageDocument {
        provider: "novel-contexts/analysis".into(),
        role: LanguageRole::Lsp,
        document: document.clone(),
    };
    demand_support::ready(&mut manager, "novel-contexts", reason());
    assert!(
        manager
            .language_document_service("novel-contexts/analysis", &document)
            .unwrap()
            .is_ok()
    );
    assert_eq!(
        std::fs::read_dir(profile.path().join("dependencies/versions"))
            .unwrap()
            .count(),
        1
    );
    std::fs::remove_file(artifact).unwrap();
    manager.disable("novel-contexts").unwrap();
    manager.enable("novel-contexts").unwrap();
    demand_support::ready(&mut manager, "novel-contexts", reason());
    assert!(
        manager
            .language_document_service("novel-contexts/analysis", &document)
            .unwrap()
            .is_ok(),
        "a complete cache entry needs no source or network"
    );
}

/// Unknown-language discovery chooses two roots/candidates and retires only the changed generation.
#[test]
#[ignore = "build project-context through current host --plugin-package --debug, then run --ignored"]
fn real_sdk_hook_routes_subprojects_and_retires_only_changed_context() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    for root in ["first", "second"] {
        std::fs::create_dir(workspace.path().join(root)).unwrap();
        std::fs::write(workspace.path().join(root).join("source.novel"), root).unwrap();
    }
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/project-context/project-context-fixture-0.1.0.zip");
    let original =
        Package::read(&fixture).expect("build the real SDK project-context fixture first");
    let mut files = original.files;
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    // Startup remains fixed package data. The test never executes these plans; real LSP transport
    // is covered separately, while args distinguish both candidate plans without a host language branch.
    for service in ["first", "second"] {
        manifest["services"][service]["program"] = std::env::current_exe()
            .unwrap()
            .display()
            .to_string()
            .into();
    }
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let package = Package::from_files(files).unwrap();
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
    let first = DocumentContext {
        path: "first/source.novel".into(),
        generation: 1,
    };
    let second = DocumentContext {
        path: "second/source.novel".into(),
        generation: 1,
    };
    let demand = |document: DocumentContext| DemandReason::LanguageDocument {
        provider: "project-context-fixture/analysis".into(),
        role: LanguageRole::Lsp,
        document,
    };
    demand_support::ready(
        &mut manager,
        "project-context-fixture",
        demand(first.clone()),
    );
    demand_support::ready(
        &mut manager,
        "project-context-fixture",
        demand(second.clone()),
    );
    let first_plan = manager
        .language_document_service("project-context-fixture/analysis", &first)
        .unwrap()
        .unwrap();
    let second_plan = manager
        .language_document_service("project-context-fixture/analysis", &second)
        .unwrap()
        .unwrap();
    assert_eq!(
        first_plan.root,
        workspace.path().join("first").canonicalize().unwrap()
    );
    assert_eq!(
        second_plan.root,
        workspace.path().join("second").canonicalize().unwrap()
    );
    assert!(!Arc::ptr_eq(&first_plan, &second_plan));
    assert_eq!(
        manager
            .language_document_context("project-context-fixture/analysis", &first)
            .unwrap()
            .key,
        "first"
    );
    let changed = DocumentContext {
        generation: 2,
        ..first.clone()
    };
    demand_support::ready(
        &mut manager,
        "project-context-fixture",
        demand(changed.clone()),
    );
    let replacement = manager
        .language_document_service("project-context-fixture/analysis", &changed)
        .unwrap()
        .unwrap();
    assert!(!first_plan.is_active());
    assert!(replacement.is_active());
    assert!(Arc::ptr_eq(
        &second_plan,
        &manager
            .language_document_service("project-context-fixture/analysis", &second)
            .unwrap()
            .unwrap()
    ));
    manager.release_language_document("project-context-fixture/analysis", &first);
    assert!(
        replacement.is_active(),
        "releasing an old intent cannot detach its replacement"
    );
    manager.disable("project-context-fixture").unwrap();
    assert!(!replacement.is_active() && !second_plan.is_active());
}

/// Build an independently declared language; the host has no knowledge of its identifier or executable.
fn fixed_package() -> Package {
    let manifest = serde_json::json!({
        "id":"novel-contexts", "name":"Novel contexts", "version":"0.1.0", "protocol":7,
        "api":{"base":"^1", "required":{"language.lsp":"^1.5", "process":"^1"}},
        "contributions":"plugin.toml", "permissions":["process.service.analysis"], "storage_limit":1024,
        "services":{"analysis":{"program":std::env::current_exe().unwrap(),"args":[]}},
        "language_servers":[{"id":"analysis","language":"novel-project-language","service":"analysis"}]
    });
    Package::from_files(BTreeMap::from([
        ("manifest.json".into(), serde_json::to_vec(&manifest).unwrap()),
        ("plugin.toml".into(), b"[plugin]\nid = \"novel-contexts\"\nname = \"Novel contexts\"\nversion = \"0.1.0\"\nhost_version = \"^0.1\"\n".to_vec()),
    ])).unwrap()
}
