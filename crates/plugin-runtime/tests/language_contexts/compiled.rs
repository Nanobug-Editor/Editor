//! Warm public language demands keep their original budget without redoing cold code preparation.
use super::*;
use plugin_runtime::plugin_protocol::api;
use std::time::{Duration, Instant};

#[path = "../support/packages.rs"]
mod frozen_packages;

/// An actual SDK guest is already available for commands before two unrelated native roles arrive.
/// Each readonly hook still initializes isolated state and returns its own provider's real proposal.
#[test]
#[ignore = "uses the fixed, hash-checked capability-example SDK component"]
fn warm_guest_discovers_new_language_roles_within_the_original_request_budget() {
    let original = frozen_packages::package("capability-example-0.19.0");
    let mut files = original.files;
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["api"]["required"]["language.lsp"] = serde_json::json!("^1.5");
    manifest["api"]["required"]["process"] = serde_json::json!("^1");
    manifest["permissions"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!("process.service.warm"));
    manifest["services"]["warm"] = serde_json::json!({
        "program":std::env::current_exe().unwrap(),"args":[]
    });
    manifest["language_servers"] = serde_json::json!([
        {"id":"first","language":"unknown-warm-language","service":"warm","hook":true},
        {"id":"second","language":"another-warm-language","service":"warm","hook":true}
    ]);
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let package = Package::from_files(files).unwrap();
    let profile = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(
        workspace.path().join("source.novel"),
        "saved original source",
    )
    .unwrap();
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
    demand_support::ready(
        &mut manager,
        &package.manifest.id,
        DemandReason::Command("echo".into()),
    );
    let owner = manager
        .instance_id(&package.manifest.id)
        .unwrap()
        .to_owned();
    let token = manager.availability(&package.manifest.id).unwrap();
    let document = DocumentContext {
        path: "source.novel".into(),
        generation: token.configuration_generation(),
    };
    let mut plans = Vec::new();
    for provider in ["first", "second"] {
        let key = format!("{}/{provider}", package.manifest.id);
        // Cold compilation in the production failure took 17–30 seconds. A warm readonly hook
        // has no process/dependency work here and must respect its original five-second budget.
        let warm_started = Instant::now();
        let request = manager
            .begin_demand(
                &token,
                DemandReason::LanguageDocument {
                    provider: key.clone(),
                    role: LanguageRole::Lsp,
                    document: document.clone(),
                },
                5_000,
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let receipt = loop {
            manager.poll();
            match request.status() {
                api::RequestUpdate::Completed { result } => {
                    break result.expect("a warm language role must become available");
                }
                api::RequestUpdate::Cancelled { reason, .. } => {
                    panic!("warm language role lost its original budget: {reason:?}")
                }
                _ => {}
            }
            assert!(
                Instant::now() < deadline,
                "the original warm request must terminate"
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        manager.admit_receipt(&token, &receipt).unwrap();
        eprintln!(
            "warm provider {provider} became available in {} ms",
            warm_started.elapsed().as_millis()
        );
        let plan = manager
            .language_document_service(&key, &document)
            .unwrap()
            .unwrap();
        assert_eq!(plan.provider.id, provider);
        assert_eq!(plan.provider.initialization_options["sdkHook"], true);
        assert_eq!(
            manager.instance_id(&package.manifest.id),
            Some(owner.as_str())
        );
        assert!(plans.iter().all(|other| !Arc::ptr_eq(other, &plan)));
        plans.push(plan);
    }
    assert!(
        plans
            .iter()
            .all(|plan| plan.is_active() && plan.process_count() == 0)
    );
    assert_eq!(
        std::fs::read_to_string(workspace.path().join("source.novel")).unwrap(),
        "saved original source"
    );
}
