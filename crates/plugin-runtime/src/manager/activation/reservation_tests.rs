//! Owned work must retain or renew the original source intent after its cache entry is withdrawn.
use super::*;
use plugin_protocol::{Environment, language::DocumentContext};
use std::time::{Duration, Instant};

/// Cache withdrawal during a queued job cannot strand its still-live source behind repeated jobs.
#[test]
fn queued_source_request_renews_a_withdrawn_reservation_before_rejoining() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("Main.java"), "class Main {}").unwrap();
    let manifest = serde_json::json!({
        "id":"queued-source","name":"Queued source","version":"0.1.0","protocol":7,
        "api":{"base":"^1","required":{"language.lsp":">=1.9,<2","process":"^1"}},
        "contributions":"plugin.toml","permissions":["process.service.analysis"],"storage_limit":1024,
        "services":{"analysis":{"program":std::env::current_exe().unwrap(),"args":[]}},
        "language_servers":[{"id":"analysis","language":"independent","service":"analysis"}]
    });
    let package=crate::Package::from_files(BTreeMap::from([
        ("manifest.json".into(),serde_json::to_vec(&manifest).unwrap()),
        ("plugin.toml".into(),b"[plugin]\nid = \"queued-source\"\nname = \"Queued source\"\nversion = \"0.1.0\"\nhost_version = \"^0.1\"\n".to_vec()),
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
    let availability = manager.availability("queued-source").unwrap();
    let document = DocumentContext {
        path: "Main.java".into(),
        generation: availability.configuration_generation(),
    };
    let request = manager
        .begin_demand(
            &availability,
            DemandReason::LanguageDocument {
                provider: "queued-source/analysis".into(),
                role: LanguageRole::Lsp,
                document: document.clone(),
            },
            5_000,
        )
        .unwrap();
    let owned = manager.take_demand_preparation().unwrap();
    // A changed domain fingerprint withdraws this same pending cache entry in the real actor.
    // Keep the owned work and its caller alive, independently from that cache lifetime.
    manager.language_services.clear();
    manager.commit_demand(owned.run()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        manager.poll();
        if let api::RequestUpdate::Completed { result } = request.status() {
            result.unwrap();
            break;
        }
        assert!(
            Instant::now() < deadline,
            "The original source waiter was stranded after cache withdrawal: {:?}",
            request.status()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        manager
            .language_document_service("queued-source/analysis", &document)
            .unwrap()
            .unwrap()
            .is_active()
    );
    manager.shutdown_checked().unwrap();
}
