//! Real independent packages publish workspace presentation through the public manager boundary.
use plugin_runtime::{
    DemandReason, Manager, Package,
    plugin_protocol::{Environment, api},
};
use serde_json::json;
use std::collections::BTreeMap;
#[path = "support/demand.rs"]
mod demand_support;

/// A non-Git review provider colors files, owns a title entry and withdraws every retained clone.
#[test]
#[ignore = "Build fixtures/workspace-presentation with --plugin-cargo and set NANOBUG_PRESENTATION_COMPONENT"]
fn workspace_presentation_publishes_and_retires_through_the_public_manager() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("review.txt"), "pending review").unwrap();
    let package = fixture();
    let mut manager = Manager::open(
        root.path().join("profile"),
        Environment {
            workspace: root.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    demand_support::ready(
        &mut manager,
        "review-queue",
        DemandReason::Command("publish".into()),
    );
    let call = manager
        .invoke_typed_command("review-queue", "publish", json!(null), 30_000)
        .unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(35);
    loop {
        manager.poll();
        if let api::RequestUpdate::Completed { result } = call.status() {
            result.unwrap();
            break;
        }
        assert!(
            std::time::Instant::now() < until,
            "Review publication did not finish: {:?}",
            call.status()
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let resource = manager.live["review-queue"]
        .workspace_presentations()
        .pop()
        .expect("published review collection");
    let snapshot = resource.snapshot();
    assert_eq!(snapshot.files["review.txt"].tooltip.en, "Awaiting review");
    assert_eq!(snapshot.entries[0].label.en, "Review queue (1)");
    let command = resource
        .command(snapshot.revision, "queue", Some("next"))
        .unwrap();
    assert_eq!(command.command, "next");
    manager.disable("review-queue").unwrap();
    assert!(!resource.is_live());
    assert_eq!(
        resource
            .command(snapshot.revision, "queue", Some("next"))
            .unwrap_err()
            .code,
        api::ErrorCode::StaleRevision
    );
}

/// Invalid paths are atomic; unrelated text changes preserve queued meaning, actual completion revokes it.
#[test]
#[ignore = "Build fixtures/workspace-presentation with --plugin-cargo and set NANOBUG_PRESENTATION_COMPONENT"]
fn workspace_presentation_rejects_paths_and_preserves_only_unchanged_queued_actions() {
    let root = tempfile::tempdir().unwrap();
    let package = fixture();
    let mut manager = Manager::open(
        root.path().join("profile"),
        Environment {
            workspace: root.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    demand_support::ready(
        &mut manager,
        "review-queue",
        DemandReason::Command("publish".into()),
    );
    invoke(&mut manager, "publish").unwrap();
    let resource = manager.live["review-queue"]
        .workspace_presentations()
        .pop()
        .unwrap();
    let initial = resource.snapshot();
    let authority = resource
        .callback_authority(initial.revision, "queue", Some("next"))
        .unwrap();
    assert_eq!(
        invoke(&mut manager, "invalid").unwrap_err().code,
        api::ErrorCode::InvalidPath
    );
    assert_eq!(resource.snapshot().file_revision, initial.file_revision);
    assert_eq!(resource.snapshot().files, initial.files);
    invoke(&mut manager, "rename").unwrap();
    assert_eq!(resource.snapshot().entries[0].label.en, "Review tasks (1)");
    assert!(
        authority.is_live(),
        "An unchanged captured action survives a label update"
    );
    assert_eq!(
        resource
            .command(initial.revision, "queue", Some("next"))
            .unwrap_err()
            .code,
        api::ErrorCode::StaleRevision
    );
    invoke(&mut manager, "next").unwrap();
    assert!(resource.snapshot().files.is_empty());
    assert_eq!(resource.snapshot().entries[0].label.en, "Review tasks (0)");
    assert!(
        !authority.is_live(),
        "Completing the review disables and revokes its queued action"
    );
    invoke(&mut manager, "publish").unwrap();
    let reopened = resource.snapshot();
    let new_authority = resource
        .callback_authority(reopened.revision, "queue", Some("next"))
        .unwrap();
    assert!(
        new_authority.is_live(),
        "Re-enabling creates fresh authority even while the old caller retains its revoked seal"
    );
    assert!(
        !authority.is_live(),
        "A new valid action must never revive its retired predecessor"
    );
    manager.shutdown_checked().unwrap();
    assert!(!resource.is_live());
}

/// Await the real typed completion rather than assuming a fixed number of polls executes a command.
fn invoke(manager: &mut Manager, command: &str) -> Result<serde_json::Value, api::Failure> {
    let call = manager.invoke_typed_command("review-queue", command, json!(null), 30_000)?;
    let until = std::time::Instant::now() + std::time::Duration::from_secs(35);
    loop {
        manager.poll();
        match call.status() {
            api::RequestUpdate::Completed { result } => return result,
            api::RequestUpdate::Cancelled { reason, .. } => {
                return Err(api::Failure::new(reason, "Review command cancelled"));
            }
            _ => {}
        }
        assert!(
            std::time::Instant::now() < until,
            "Review command did not finish"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Assemble the actual review-domain SDK consumer without depending on any Git-specific host path.
fn fixture() -> Package {
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/workspace-presentation/manifest.json"
    ))
    .unwrap();
    Package::from_files(BTreeMap::from([
        ("manifest.json".into(), serde_json::to_vec(&manifest).unwrap()),
        ("fixture.wasm".into(), std::fs::read(std::env::var_os("NANOBUG_PRESENTATION_COMPONENT").expect("NANOBUG_PRESENTATION_COMPONENT")).unwrap()),
        ("review.svg".into(), br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M2 8l4 4 8-8" fill="none" stroke="currentColor"/></svg>"##.to_vec()),
    ])).unwrap()
}
