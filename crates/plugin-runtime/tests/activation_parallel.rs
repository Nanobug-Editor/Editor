//! Native consumers can finish while unrelated startup preparations retain their own worker leases.
use plugin_runtime::{
    DemandReason, DemandRequest, LanguageRole, Manager, Package,
    plugin_protocol::{Environment, api},
};
use std::collections::BTreeMap;

#[path = "support/packages.rs"]
mod frozen_packages;

/// Use the real SDK component for panels/background purposes and declarative language providers.
/// Taken leases model slow work without downloading or sleeping.
fn provider(id: &str, language: bool) -> Package {
    if !language {
        let mut files = frozen_packages::package("capability-example-0.19.0").files;
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&files["manifest.json"]).unwrap();
        manifest["id"] = id.into();
        manifest["name"] = id.into();
        manifest["activation"] =
            serde_json::json!({"background":{"purpose":"Observe this independent test provider"}});
        manifest["api"]["required"]["lifecycle.activation"] = "^1".into();
        files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
        files.insert(
            "plugin.toml".into(),
            format!("[plugin]\nid='{id}'\nname='{id}'\nversion='0.19.0'\nhost_version='^0.1'\n")
                .into_bytes(),
        );
        return Package::from_files(files).unwrap();
    }
    let manifest = serde_json::json!({
        "id":id,"name":id,"version":"0.1.0","protocol":7,
        "api":{"base":"^1","required":{"lifecycle.activation":"^1","language.lsp":"^1","process":"^1"}},
        "contributions":"plugin.toml","permissions":["process.service.analysis"],"storage_limit":1024,
        "services":{"analysis":{"program":std::env::current_exe().unwrap(),"args":[]}},
        "language_servers":[{"id":"analysis","language":"independent","service":"analysis"}]
    });
    Package::from_files(BTreeMap::from([
        (
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        ),
        (
            "plugin.toml".into(),
            format!("[plugin]\nid='{id}'\nname='{id}'\nversion='0.1.0'\nhost_version='^0.1'\n")
                .into_bytes(),
        ),
    ]))
    .unwrap()
}

/// Cold SDK compilation has its own bounded deadline; concurrency assertions retain actual
/// leases and do not depend on preparation finishing within an arbitrary millisecond threshold.
fn demand(manager: &mut Manager, id: &str, reason: DemandReason) -> DemandRequest {
    let token = manager.availability(id).unwrap();
    manager.begin_demand(&token, reason, 120_000).unwrap()
}

/// Three independent background or language jobs remain unfinished while a fourth queued observer
/// leaves capacity for a native panel. A dropped peer must not retire the visible owner.
#[test]
#[ignore = "actual frozen SDK component; see fixtures/packages/README.md"]
fn interactive_view_finishes_beside_slow_background_and_language_preparations() {
    for language in [false, true] {
        let profile = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let mut manager = Manager::open(
            profile.path().into(),
            Environment {
                workspace: workspace.path().display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        for id in ["slow-a", "slow-b", "slow-c", "slow-d", "visible-z"] {
            let package = provider(id, language && id != "visible-z");
            manager
                .install(&package, package.manifest.permissions.clone())
                .unwrap();
        }
        let reason = |id: &str| {
            if language {
                DemandReason::Language {
                    provider: format!("{id}/analysis"),
                    role: LanguageRole::Lsp,
                }
            } else {
                DemandReason::Background
            }
        };
        let mut waits = Vec::new();
        let mut jobs = Vec::new();
        for id in ["slow-a", "slow-b", "slow-c"] {
            waits.push(demand(&mut manager, id, reason(id)));
            jobs.push(
                manager
                    .take_demand_preparation()
                    .expect("independent startup jobs can run concurrently"),
            );
        }
        waits.push(demand(&mut manager, "slow-d", reason("slow-d")));
        assert!(
            manager.take_demand_preparation().is_none(),
            "startup work filled the native interaction's last preparation slot (language={language})"
        );
        let visible = demand(
            &mut manager,
            "visible-z",
            DemandReason::View("welcome".into()),
        );
        let job = manager
            .take_demand_preparation()
            .expect("visible panel has independent capacity");
        let receipt = manager.commit_demand(job.run()).unwrap();
        assert_eq!(receipt.availability().plugin(), "visible-z");
        assert!(matches!(
            visible.status(),
            api::RequestUpdate::Completed { result: Ok(_) }
        ));
        assert!(manager.live["visible-z"].views.contains_key("welcome"));
        assert!(
            waits.iter().all(|wait| !wait.status().is_terminal()),
            "visible content must finish while all unrelated preparations are still pending"
        );
        drop(jobs.pop());
        // Pruning an abandoned job releases only that peer. Keep the remaining owned jobs
        // outstanding so failure isolation is checked before any unrelated preparation finishes.
        drop(manager.take_demand_preparation());
        assert!(matches!(
            waits[2].status(),
            api::RequestUpdate::Completed { result: Err(_) }
        ));
        assert!(!waits[0].status().is_terminal());
        assert!(manager.live["visible-z"].views.contains_key("welcome"));
        for wait in &waits {
            if !wait.status().is_terminal() {
                wait.cancel().unwrap();
            }
        }
        drop(jobs);
        manager.shutdown_checked().unwrap();
    }
}

/// Lexically earlier background observations cannot take precedence over an already shown view.
#[test]
#[ignore = "actual frozen SDK component; see fixtures/packages/README.md"]
fn visible_view_precedes_queued_background_preparation() {
    let profile = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    for id in ["background-a", "visible-z"] {
        let package = provider(id, false);
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
    }
    let background = demand(&mut manager, "background-a", DemandReason::Background);
    let visible = demand(
        &mut manager,
        "visible-z",
        DemandReason::View("welcome".into()),
    );
    let job = manager.take_demand_preparation().unwrap();
    let receipt = manager.commit_demand(job.run()).unwrap();
    assert_eq!(receipt.availability().plugin(), "visible-z");
    assert!(matches!(
        visible.status(),
        api::RequestUpdate::Completed { result: Ok(_) }
    ));
    assert!(!background.status().is_terminal());
    background.cancel().unwrap();
    manager.shutdown_checked().unwrap();
}
