//! Unknown-language packages verify explicit membership and whole-context source version rejection.
use super::context::{ContextCapture, ContextSource};
use super::*;
use gpui_base::input::Rope;
use plugin_runtime::{
    DemandReason, LanguageRole, Manager, Package,
    plugin_protocol::{self as protocol, language::DocumentContext},
};
use std::collections::BTreeMap;

/// Graph-selected dirty dependencies synchronize before analysis; unrelated text stays host-local,
/// and another consumer changing only the dependency invalidates the original target's result.
#[test]
#[ignore = "build native lsp_fixture before running the real framed context snapshot regression"]
fn related_native_sources_are_explicit_and_auxiliary_changes_revoke_results() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    // The running test binary fixes the source snapshot, Cargo target and profile of this fixture.
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/lsp_fixture.exe")
        .canonicalize()
        .unwrap();
    let log = workspace.path().join("wire.jsonl");
    let manifest = json!({
        "id":"related-sources","name":"Related sources","version":"0.1.0","protocol":7,"contributions":"plugin.toml","storage_limit":1024,
        "api":{"base":"^1","required":{"language.lsp":"^1.5","language.control":"^1","process":"^1"}},
        "permissions":["process.service.analysis","editor.read","workspace.read"],
        "services":{"analysis":{"program":executable,"args":[log]}},
        "language_servers":[{"id":"analysis","language":"unfamiliar","service":"analysis","related_documents":"fixture/context/documents",
            "control_methods":{"fixture/context/documents":protocol::language_context::related_documents_method(),
                "fixture/barrier":{"parameters":{"type":"null"},"result":{"type":"null"}}}}]
    });
    let package = Package::from_files(BTreeMap::from([
        ("manifest.json".into(),serde_json::to_vec(&manifest).unwrap()),
        ("plugin.toml".into(),b"[plugin]\nid='related-sources'\nname='Related sources'\nversion='0.1.0'\nhost_version='^0.1'\n".to_vec()),
    ])).unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        protocol::Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let availability = manager.availability("related-sources").unwrap();
    let document = DocumentContext {
        path: "target.novel".into(),
        generation: availability.configuration_generation(),
    };
    let request = manager
        .begin_demand(
            &availability,
            DemandReason::LanguageDocument {
                provider: "related-sources/analysis".into(),
                role: LanguageRole::Lsp,
                document: document.clone(),
            },
            10_000,
        )
        .unwrap();
    let prepared = manager.take_demand_preparation().unwrap().run();
    manager.commit_demand(prepared).unwrap();
    assert!(matches!(
        request.status(),
        protocol::api::RequestUpdate::Completed { result: Ok(_) }
    ));
    let plan = manager
        .language_document_service("related-sources/analysis", &document)
        .unwrap()
        .unwrap();
    let server = Arc::new(LanguageServer::from_service(plan).unwrap());
    let sources = ["target", "related", "unrelated"]
        .into_iter()
        .map(|id| {
            let path = format!("{id}.novel");
            ContextSource {
                document: protocol::api::DocumentVersion {
                    id: id.into(),
                    path: path.clone(),
                    revision: if id == "related" { 29 } else { 1 },
                },
                uri: file_uri(&workspace.path().join(path)).unwrap(),
                language: "unfamiliar".into(),
                text: Rope::from(format!("{id} = unsaved")),
            }
        })
        .collect::<Vec<_>>();
    let capture =
        || ContextCapture::new(workspace.path().into(), 1, "target".into(), sources.clone());
    let epoch = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let first = capture().with_guard(epoch.clone());
    let target = server.open_document(sources[0].uri.clone());
    server
        .with_context_snapshot(&first, || {
            let metadata = first
                .wire_sources()
                .expect("graph members synchronized before control");
            server.control_request_at_version(
                "fixture/barrier",
                json!({"arguments":null,"_nanobug":{"sources":metadata}}),
                target.clone(),
                &sources[0].text.to_string(),
                &sources[0].document,
                &RequestCancellation::default(),
            )?;
            server.definitions_for(
                target.clone(),
                sources[0].text.to_string(),
                Position::new(0, 0),
            )
        })
        .unwrap();
    let members = first
        .documents()
        .unwrap()
        .iter()
        .map(|document| document.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(members, vec!["related", "target"]);
    let frames = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let opens = frames
        .iter()
        .filter(|frame| frame["method"] == "textDocument/didOpen")
        .collect::<Vec<_>>();
    let metadata = frames
        .iter()
        .find(|frame| frame["method"] == "fixture/barrier")
        .expect("source-bound control uses the same native connection");
    let metadata: Vec<protocol::language_control::SourceMetadata> =
        serde_json::from_value(metadata["params"]["_nanobug"]["sources"].clone()).unwrap();
    assert_eq!(
        metadata
            .iter()
            .map(|source| source.document.id.as_str())
            .collect::<Vec<_>>(),
        vec!["related", "target"]
    );
    assert_eq!(metadata[0].document.revision, 29);
    let related_open = opens
        .iter()
        .find(|frame| {
            frame["params"]["textDocument"]["uri"].as_str() == Some(metadata[0].uri.as_str())
        })
        .expect("metadata URI must name an actual synchronized source");
    assert_eq!(
        serde_json::json!(metadata[0].version),
        related_open["params"]["textDocument"]["version"],
        "metadata uses the actual didOpen watermark, not an inferred version"
    );
    assert_ne!(
        metadata[0].version as u64, metadata[0].document.revision,
        "public auxiliary revision remains separate from wire watermarks"
    );
    assert_eq!(
        opens.len(),
        2,
        "the provider selected exactly one dirty dependency"
    );
    assert!(
        !opens
            .iter()
            .any(|frame| frame["params"]["textDocument"]["text"] == "unrelated = unsaved")
    );
    let stale = capture();
    let result = server.with_context_snapshot(&stale, || {
        let related = server.open_document(sources[1].uri.clone());
        server.control_request_for(
            "fixture/barrier",
            json!({"arguments":null}),
            related,
            "related = changed",
            &RequestCancellation::default(),
        )?;
        Ok(())
    });
    assert_eq!(
        result
            .unwrap_err()
            .downcast_ref::<protocol::api::Failure>()
            .unwrap()
            .code,
        protocol::api::ErrorCode::StaleRevision
    );
    assert!(
        stale.documents().is_none(),
        "an invalidated context cannot issue a UI result seal"
    );
    let origin = first.fix_origin(&server);
    assert!(first.is_live() && origin.is_live());
    epoch.store(false, std::sync::atomic::Ordering::Release);
    assert!(
        !first.is_live() && !origin.is_live(),
        "file/configuration epoch revokes the captured result and its native edit authority"
    );
    assert!(
        server.is_active(),
        "result revocation must not retire the project transport"
    );
    manager.disable("related-sources").unwrap();
}
