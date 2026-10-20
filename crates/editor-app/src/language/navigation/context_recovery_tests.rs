//! Real framed recovery must retire old analysis results even when public sources are unchanged.
//! This independent candidate is registered only when its native fixture is ready for an actual RED run.
use super::*;
use plugin_runtime::{
    DemandReason, LanguageRole, Manager, Package,
    plugin_protocol::{self as protocol, language::DocumentContext},
};
use std::collections::BTreeMap;

/// A completed capture belongs to its original pipe, rather than the surviving service plan.
/// Identical source revisions in the replacement pipe cannot make an old UI result current again.
#[test]
#[ignore = "build the matching native lsp_fixture before the real connection-incarnation regression"]
fn old_context_capture_is_not_live_after_native_pipe_recovery() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let log = profile.path().join("wire.jsonl");
    let package = graph_package(&log);
    let mut manager = Manager::open(
        profile.path().join("runtime"),
        protocol::Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let availability = manager.availability("recovery-graph").unwrap();
    let context = DocumentContext {
        path: "target.novel".into(),
        generation: availability.configuration_generation(),
    };
    let demand = manager
        .begin_demand(
            &availability,
            DemandReason::LanguageDocument {
                provider: "recovery-graph/analysis".into(),
                role: LanguageRole::Lsp,
                document: context.clone(),
            },
            10_000,
        )
        .unwrap();
    let prepared = manager.take_demand_preparation().unwrap().run();
    manager.commit_demand(prepared).unwrap();
    let protocol::api::RequestUpdate::Completed { result } = demand.status() else {
        panic!("native graph demand did not complete")
    };
    manager
        .admit_receipt(&availability, &result.unwrap())
        .unwrap();
    let plan = manager
        .language_document_service("recovery-graph/analysis", &context)
        .unwrap()
        .unwrap();
    let server = LanguageServer::from_service(plan.clone()).unwrap();
    let sources = sources(workspace.path());
    let capture = || {
        ContextCapture::new(
            workspace.path().into(),
            availability.configuration_generation(),
            "target".into(),
            sources.clone(),
        )
    };
    let document = server.open_document(sources[0].uri.clone());
    let analyze = |capture: &ContextCapture| {
        server.with_context_snapshot(capture, || {
            server.definitions_for(
                document.clone(),
                sources[0].text.to_string(),
                Position::new(0, 0),
            )
        })
    };
    let old = capture();
    assert_eq!(analyze(&old).unwrap().len(), 1);
    assert!(
        old.is_live(),
        "the first result must really have been sealed"
    );
    let old_origin = old.fix_origin(&server);
    assert!(old_origin.is_live());
    assert_eq!(plan.process_count(), 1);

    // Public plan-scoped stop terminates this process only. The adapter must observe real EOF on
    // the next ordinary request; no injected error or transport-state mutation supplies the failure.
    plan.stop_processes();
    assert_eq!(plan.process_count(), 0);
    assert!(
        server
            .definitions_for(
                document.clone(),
                sources[0].text.to_string(),
                Position::new(0, 0),
            )
            .is_err(),
        "an interrupted operation must fail instead of being replayed"
    );
    assert_eq!(server.recovery_state(), Some(RecoveryState::WaitingRetry));
    server.prepare_until_ready().unwrap();
    let fresh = capture();
    assert_eq!(analyze(&fresh).unwrap().len(), 1);
    assert_eq!(
        old.documents(),
        fresh.documents(),
        "only transport incarnation changes in this regression"
    );
    let old_is_live = old.is_live();
    let fresh_is_live = fresh.is_live();
    let old_origin_is_live = old_origin.is_live();
    assert!(server.is_active() && plan.is_active());
    assert_eq!(plan.process_count(), 1);
    let frames = std::fs::read_to_string(log)
        .unwrap()
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        frames
            .iter()
            .filter(|frame| frame["method"] == "initialize")
            .count(),
        2,
        "the public service must launch exactly one replacement native pipe"
    );
    manager.disable("recovery-graph").unwrap();
    assert!(
        fresh_is_live,
        "the replacement pipe must produce a valid fresh result"
    );
    assert!(
        !old_is_live,
        "a result from the terminated pipe must stay stale after native recovery"
    );
    assert!(
        !old_origin_is_live,
        "a pre-failure native edit origin cannot adopt the replacement pipe"
    );
}

/// An unfamiliar resource-only package exercises ordinary grants and graph control negotiation.
/// The native server selects IDs explicitly; neither the host nor this test installs a language rule.
fn graph_package(log: &Path) -> Package {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/lsp_fixture.exe")
        .canonicalize()
        .expect("matching native lsp_fixture");
    let manifest = json!({
        "id":"recovery-graph","name":"Recovery graph","version":"0.1.0","protocol":7,
        "contributions":"plugin.toml","storage_limit":1024,
        "api":{"base":"^1","required":{"language.lsp":"^1.5","language.control":"^1","process":"^1"}},
        "permissions":["process.service.analysis","editor.read","workspace.read"],
        "services":{"analysis":{"program":executable,"args":[log]}},
        "language_servers":[{"id":"analysis","language":"unfamiliar-recovery","service":"analysis",
            "related_documents":"fixture/context/documents",
            "control_methods":{"fixture/context/documents":protocol::language_context::related_documents_method()}}]
    });
    Package::from_files(BTreeMap::from([
        ("manifest.json".into(), serde_json::to_vec(&manifest).unwrap()),
        ("plugin.toml".into(), b"[plugin]\nid='recovery-graph'\nname='Recovery graph'\nversion='0.1.0'\nhost_version='^0.1'\n".to_vec()),
    ]))
    .unwrap()
}

/// Dirty immutable snapshots keep the same public IDs and revisions across the actual reconnect.
fn sources(workspace: &Path) -> Vec<ContextSource> {
    ["target", "related"]
        .into_iter()
        .map(|id| {
            let path = format!("{id}.novel");
            ContextSource {
                document: protocol::api::DocumentVersion {
                    id: id.into(),
                    path: path.clone(),
                    revision: 7,
                },
                uri: file_uri(&workspace.join(path)).unwrap(),
                language: "unfamiliar-recovery".into(),
                text: Rope::from(format!("{id} = unsaved")),
            }
        })
        .collect()
}
