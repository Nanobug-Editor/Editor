//! Real actor receipts distinguish recoverable persistence failures from successful retirement.

use super::*;

/// Wait only for an observable production publication, without replacing the runtime transport.
fn wait_for(worker: &Worker, ready: impl Fn(&Published) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !ready(&worker.state.lock().unwrap()) {
        assert!(Instant::now() < deadline, "worker publication timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A corrupt cold store starts no guest, so closing it remains possible after initialization ends.
#[test]
fn shutdown_after_failed_cold_start_has_no_resources_to_wait_for() {
    let directory = tempfile::tempdir().unwrap();
    // A directory in place of the registry gives a real, deterministic filesystem load failure.
    std::fs::create_dir(directory.path().join("registry.json")).unwrap();
    let worker = Worker::start_background(directory.path().into(), Environment::default(), true);
    wait_for(&worker, |state| state.status.is_some());
    let receipt = worker.request_shutdown().unwrap();
    futures::executor::block_on(receipt).unwrap().unwrap();
    // Retry sees the same completed lifetime; it cannot require a now-disconnected actor.
    futures::executor::block_on(worker.request_shutdown().unwrap())
        .unwrap()
        .unwrap();
}

/// A real guest's failed checkpoint is reported, its instance survives, and an ordinary retry retires it.
#[cfg(windows)]
#[test]
#[ignore = "opt-in frozen Package/WASM fixture; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn shutdown_checkpoint_failure_retains_actor_for_retry() {
    use std::os::windows::fs::OpenOptionsExt as _;
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("plugins");
    let environment = Environment {
        workspace: directory
            .path()
            .canonicalize()
            .unwrap()
            .display()
            .to_string(),
        ..Default::default()
    };
    let package = crate::test_packages::package("capability-example");
    let id = package.manifest.id.clone();
    let panel = package.manifest.panels.first().unwrap().id.clone();
    let state_file =
        Manager::persisted_data_directory(&root, &package.manifest, &environment.workspace)
            .unwrap()
            .parent()
            .unwrap()
            .join("state.json");
    let mut manager = Manager::open(root.clone(), environment.clone()).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    drop(manager);
    let worker = Worker::start_background(root.clone(), environment.clone(), true);
    wait_for(&worker, |state| state.ready);
    let availability = worker.state.lock().unwrap().availabilities[&id].clone();
    let mut demand = worker
        .demand(
            availability,
            plugin_runtime::DemandReason::View(panel),
            None,
        )
        .unwrap();
    futures::executor::block_on(demand.wait()).unwrap();
    wait_for(&worker, |state| state.instance_ids.contains_key(&id));

    // Preserve readable last-good bytes while denying atomic replacement through the Windows kernel.
    std::fs::create_dir_all(state_file.parent().unwrap()).unwrap();
    std::fs::write(&state_file, b"last-good-checkpoint").unwrap();
    let locked = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&state_file)
        .unwrap();
    let original_instance = worker.state.lock().unwrap().instance_ids[&id].clone();
    let failed = futures::executor::block_on(worker.request_shutdown().unwrap()).unwrap();
    assert!(
        failed.as_ref().is_err_and(|message| message.contains(&id)),
        "checkpoint must report its owner: {failed:?}"
    );
    wait_for(&worker, |state| {
        state
            .status
            .as_ref()
            .is_some_and(|status| status.message.contains(&id))
    });
    let retained_instance = worker.state.lock().unwrap().instance_ids[&id].clone();
    assert_eq!(
        retained_instance, original_instance,
        "a failed checkpoint cannot discard the live guest"
    );
    assert_eq!(std::fs::read(&state_file).unwrap(), b"last-good-checkpoint");
    drop(locked);

    futures::executor::block_on(worker.request_shutdown().unwrap())
        .unwrap()
        .unwrap();
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(state_file).unwrap()).unwrap();
    assert!(
        saved.get("data").is_some(),
        "retry persists the real guest's snapshot"
    );
    // Successful completion also releases the real manager's store owner, allowing a fresh project.
    let mut reopened = Manager::open(root, environment).unwrap();
    reopened.shutdown_checked().unwrap();
}
