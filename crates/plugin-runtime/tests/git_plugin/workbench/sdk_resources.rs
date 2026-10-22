//! The independent Git consumer proves helper ownership through real WASM and public lifecycle gates.
use super::*;

#[path = "sdk_resources/failures.rs"]
mod failures;

/// A separate fault-only derivative of the independent Git guest traps after normal activation.
/// No test command, plugin-ID branch or fault hook is added to the production host or delivered ZIP.
#[test]
#[ignore = "build the documented fault-only Git derivative; set NANOBUG_GIT_FAULT_CANDIDATE"]
fn git_sdk_trap_retires_the_actual_background_resource_owner() {
    if !run_isolated_test(
        "workbench::sdk_resources::git_sdk_trap_retires_the_actual_background_resource_owner",
    ) {
        return;
    }
    let package = Package::read(Path::new(
        &std::env::var_os("NANOBUG_GIT_FAULT_CANDIDATE")
            .expect("explicit fault-only package required"),
    ))
    .unwrap();
    assert_eq!(package.manifest.version, "0.1.24-trap");
    let repository = repository();
    commit_file(repository.path(), "tracked.txt", "base\n", "base");
    let (_profile, mut manager) = activate(&package, repository.path());
    let identity = manager.instance_id("git").unwrap().to_owned();
    let observations = manager.runtime_observations();
    let before = manager.live["git"].resource_counts();
    assert_eq!((before.timers, before.subscriptions), (1, 1));
    assert!(
        manager
            .event("git", None, api::Notification::Focus(true))
            .is_err()
    );
    assert!(
        manager
            .diagnostics("git")
            .iter()
            .any(|report| report.plugin == "git")
    );
    // Check the trap itself before any explicit disable could conceal a cleanup regression.
    assert_eq!(manager.resource_count(), 0);
    let evidence = observations.snapshot(Some("git"));
    let owner = evidence
        .owners
        .iter()
        .find(|owner| owner.owner.instance == identity && owner.owner.resource_owner)
        .unwrap();
    let after = owner.resources.as_ref().unwrap();
    assert!(after.retired);
    assert_eq!(
        (
            after.handles,
            after.timers,
            after.subscriptions,
            after.projections,
            after.processes
        ),
        (0, 0, 0, 0, 0)
    );
    assert_eq!(after.native_pending, 0);
    assert!(!after.native_cleanup_unconfirmed);
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// Repeated guest activation reuses all background/tree authorities, including while panels hide.
#[test]
#[ignore = "build independent Git 0.1.24; set NANOBUG_GIT_CANDIDATE; real ResourceGroup consumer"]
fn git_sdk_repeated_activation_and_hidden_reopen_keep_one_observer_pair() {
    if !run_isolated_test(
        "workbench::sdk_resources::git_sdk_repeated_activation_and_hidden_reopen_keep_one_observer_pair",
    ) {
        return;
    }
    let repository = repository();
    commit_file(repository.path(), "tracked.txt", "base\n", "base");
    let (_profile, mut manager) = activate(&candidate(), repository.path());
    let before = manager.live["git"].resource_counts();
    assert_eq!((before.timers, before.subscriptions), (1, 1));
    let identity = manager.instance_id("git").unwrap().to_owned();
    for _ in 0..3 {
        manager.live.get_mut("git").unwrap().activate().unwrap();
        idle(&mut manager);
        let current = manager.live["git"].resource_counts();
        assert_eq!((current.timers, current.subscriptions), (1, 1));
        assert_eq!(
            (current.handles, current.projections),
            (before.handles, before.projections)
        );
    }
    for visible in [false, true] {
        for panel in ["staging", "history"] {
            manager
                .event(
                    "git",
                    Some(panel.into()),
                    api::Notification::PanelVisibility {
                        panel: panel.into(),
                        visible,
                    },
                )
                .unwrap();
            idle(&mut manager);
        }
        let current = manager.live["git"].resource_counts();
        assert_eq!((current.timers, current.subscriptions), (1, 1));
        assert_eq!(manager.instance_id("git").unwrap(), identity);
    }
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// Upgrade the frozen old ZIP, reject a malformed candidate, and attribute retirement by incarnation.
#[test]
#[ignore = "build independent Git 0.1.24; set NANOBUG_GIT_CANDIDATE and NANOBUG_GIT_BASELINE"]
fn git_sdk_update_retires_old_owner_and_failed_candidate_preserves_current_data() {
    if !run_isolated_test(
        "workbench::sdk_resources::git_sdk_update_retires_old_owner_and_failed_candidate_preserves_current_data",
    ) {
        return;
    }
    use sha2::Digest as _;
    let old_bytes =
        fs::read(std::env::var_os("NANOBUG_GIT_BASELINE").expect("frozen old ZIP required"))
            .unwrap();
    assert_eq!(
        format!("{:x}", sha2::Sha256::digest(&old_bytes)),
        "7d0ea99e484afafe343902a1cc30baaebbb7a618ae1c4ecee4144579727fe5bd"
    );
    let old = Package::from_bytes(&old_bytes).unwrap();
    let new = candidate();
    let repository = repository();
    commit_file(repository.path(), "tracked.txt", "base\n", "base");
    let (_profile, mut manager) = activate(&old, repository.path());
    let observations = manager.runtime_observations();
    observations.start_capture();
    let old_owner = manager.instance_id("git").unwrap().to_owned();
    let data = manager.data_directory("git");
    fs::create_dir_all(&data).unwrap();
    fs::write(
        data.join("sdk-adoption-sentinel.txt"),
        b"retained managed private data",
    )
    .unwrap();
    let revision = manager.live["git"].views["staging"].revision;
    manager
        .event(
            "git",
            Some("staging".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: "commit-message".into(),
                action: ui::Action::Change("保留的提交草稿 / retained draft".into()),
            }),
        )
        .unwrap();
    manager
        .install(&new, new.manifest.permissions.clone())
        .unwrap();
    idle(&mut manager);
    let current_owner = manager.instance_id("git").unwrap().to_owned();
    assert_ne!(current_owner, old_owner);
    let state = manager.live.get_mut("git").unwrap().snapshot().unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&state.data).unwrap()["commit_message"],
        "保留的提交草稿 / retained draft"
    );
    assert_eq!(manager.installed["git"].manifest.version, "0.1.24");
    assert_eq!(
        fs::read(
            manager
                .data_directory("git")
                .join("sdk-adoption-sentinel.txt")
        )
        .unwrap(),
        b"retained managed private data"
    );
    let counts = manager.live["git"].resource_counts();
    assert_eq!((counts.timers, counts.subscriptions), (1, 1));
    let mut files = new.files.clone();
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["version"] = "0.1.25".into();
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    files.insert(
        new.manifest.component.clone().unwrap(),
        b"\0asm\x0d\0\x01\0invalid component".to_vec(),
    );
    let failed = Package::from_files(files).unwrap();
    assert!(
        manager
            .install(&failed, failed.manifest.permissions.clone())
            .is_err()
    );
    assert_eq!(manager.instance_id("git").unwrap(), current_owner);
    assert_eq!(manager.installed["git"].manifest.version, "0.1.24");
    assert_eq!(
        fs::read(
            manager
                .data_directory("git")
                .join("sdk-adoption-sentinel.txt")
        )
        .unwrap(),
        b"retained managed private data"
    );
    let retained = manager.live.get_mut("git").unwrap().snapshot().unwrap();
    assert_eq!(retained.data, state.data);

    // This candidate is a valid component and passes Package admission. Its missing document
    // negotiation fails only after the timer is accepted in the actual Activate callback.
    let mut files = new.files.clone();
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["version"] = "0.1.26".into();
    manifest["api"]["required"]
        .as_object_mut()
        .unwrap()
        .remove("editor.documents");
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let activation_failure = Package::from_files(files).unwrap();
    let failure = manager
        .install(
            &activation_failure,
            activation_failure.manifest.permissions.clone(),
        )
        .unwrap_err();
    assert!(
        format!("{failure:#}").contains("editor.documents"),
        "the failure must come from actual observer initialization: {failure:#}"
    );
    assert_eq!(manager.installed["git"].manifest.version, "0.1.24");
    let restored = manager.live.get_mut("git").unwrap().snapshot().unwrap();
    assert_eq!(restored.data, state.data);
    assert_eq!(
        fs::read(
            manager
                .data_directory("git")
                .join("sdk-adoption-sentinel.txt")
        )
        .unwrap(),
        b"retained managed private data"
    );
    let restored_owner = manager.instance_id("git").unwrap().to_owned();
    let restored_counts = manager.live["git"].resource_counts();
    assert_eq!(
        (restored_counts.timers, restored_counts.subscriptions),
        (1, 1)
    );
    manager
        .invoke_command("git", "refresh", json!(null))
        .unwrap();
    idle(&mut manager);
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
    let evidence = observations.snapshot(Some("git"));
    for identity in [old_owner, current_owner, restored_owner] {
        let owner = evidence
            .owners
            .iter()
            .find(|owner| owner.owner.instance == identity && owner.owner.resource_owner)
            .expect("the real resource owner must be observed");
        let counts = owner.resources.as_ref().unwrap();
        assert!(counts.retired);
        assert_eq!(
            (
                counts.handles,
                counts.timers,
                counts.subscriptions,
                counts.projections,
                counts.processes,
                counts.requests
            ),
            (0, 0, 0, 0, 0, 0)
        );
        assert_eq!(counts.native_pending, 0);
        assert!(!counts.native_cleanup_unconfirmed);
    }
}
