//! Actual candidate ZIP admission distinguishes stale, fresh, retired and malformed form callbacks.
use super::*;
use std::{path::Path, process::Command};

/// Private repository config and Git arguments never touch user repositories or global settings.
fn git(root: &Path, args: &[&str]) {
    super::super::super::git_fixture_environment::reject_repository_overrides()
        .expect("fixture environment must not redirect repository-local Git state");
    super::super::super::git_fixture_environment::require_private_configuration()
        .expect("Worker fixture requires its isolated test child");
    let output = Command::new("git")
        .current_dir(root)
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "fixture Git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn wait_for(worker: &Worker, predicate: impl Fn(&Published) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !predicate(&worker.state.lock().unwrap()) {
        assert!(
            Instant::now() < deadline,
            "actor form publication timed out"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn wait_receipt<T>(mut receipt: futures::channel::oneshot::Receiver<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(value) = receipt.try_recv().unwrap() {
            return value;
        }
        assert!(Instant::now() < deadline, "actor form receipt timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn value(state: &Published) -> String {
    let node = state.views["git/staging"]
        .active_node("commit-message")
        .unwrap();
    let ui::Kind::Textarea(input) = &node.kind else {
        panic!("candidate multiline form")
    };
    input.value.clone()
}

fn send(worker: &Worker, epoch: u64, revision: u64, action: ui::Action) -> FormReceipt {
    let (reply, receipt) = futures::channel::oneshot::channel();
    worker
        .tx
        .send(Work::FormChange {
            plugin: "git".into(),
            epoch,
            panel: "staging".into(),
            event: ui::UiEvent {
                revision,
                node: "commit-message".into(),
                action,
            },
            reply,
        })
        .unwrap();
    wait_receipt(receipt)
}

/// The production actor calls public Manager::event for the independent package. A stale text
/// callback cannot mutate the guest; a current Change is acknowledged only after its guest runs.
#[test]
#[ignore = "requires independent Git candidate ZIP; set NANOBUG_GIT_CANDIDATE"]
fn actual_manager_form_receipts_preserve_scene_and_incarnation_admission() {
    if !super::super::super::git_fixture_environment::run_isolated_test(
        "extensions::worker::form_delivery::tests::actual_manager_form_receipts_preserve_scene_and_incarnation_admission",
    ) {
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let workspace = directory.path().join("worktree");
    std::fs::create_dir(&workspace).unwrap();
    git(&workspace, &["init", "-b", "main"]);
    std::fs::write(workspace.join("tracked.txt"), "base\n").unwrap();
    git(&workspace, &["add", "--", "tracked.txt"]);
    git(
        &workspace,
        &[
            "-c",
            "user.name=Form fixture",
            "-c",
            "user.email=form@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "base",
        ],
    );
    let package = Package::read(Path::new(
        &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate ZIP required"),
    ))
    .unwrap();
    let root = directory.path().join("plugins");
    let environment = Environment {
        workspace: workspace.display().to_string(),
        ..Default::default()
    };
    let mut manager = Manager::open(root.clone(), environment.clone()).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    drop(manager);
    let worker = Worker::start_background(root, environment, true);
    wait_for(&worker, |state| state.ready);
    let availability = worker.state.lock().unwrap().availabilities["git"].clone();
    let mut demand = worker
        .demand(
            availability,
            plugin_runtime::DemandReason::View("staging".into()),
            None,
        )
        .unwrap();
    futures::executor::block_on(demand.wait()).unwrap();
    wait_for(&worker, |state| {
        state.views.contains_key("git/staging")
            && state.processes.get("git").copied().unwrap_or(0) == 0
    });
    let (epoch, revision, initial) = {
        let state = worker.state.lock().unwrap();
        (
            state.instance_epochs["git"],
            state.views["git/staging"].revision,
            value(&state),
        )
    };
    let stale = send(
        &worker,
        epoch,
        revision.wrapping_sub(1),
        ui::Action::Change("stale draft".into()),
    );
    let after_stale = value(&worker.state.lock().unwrap());
    let revision = worker.state.lock().unwrap().views["git/staging"].revision;
    let delivered = send(
        &worker,
        epoch,
        revision,
        ui::Action::Change("Receipt 中文".into()),
    );
    wait_for(&worker, |state| value(state) == "Receipt 中文");
    let revision = worker.state.lock().unwrap().views["git/staging"].revision;
    let retired = send(
        &worker,
        epoch.wrapping_add(1),
        revision,
        ui::Action::Change("retired draft".into()),
    );
    let rejected = send(&worker, epoch, revision, ui::Action::Click);
    let final_value = value(&worker.state.lock().unwrap());
    let (shutdown, retired_actor) = futures::channel::oneshot::channel();
    worker.tx.send(Work::Shutdown(Some(shutdown))).unwrap();
    wait_receipt(retired_actor).unwrap();
    assert_eq!(stale, FormReceipt::Stale);
    assert_eq!(
        after_stale, initial,
        "the rejected old scene cannot reach the guest"
    );
    assert_eq!(delivered, FormReceipt::Delivered);
    assert_eq!(retired, FormReceipt::Retired);
    assert_eq!(
        rejected,
        FormReceipt::Rejected,
        "business Click cannot enter the Change retry channel"
    );
    assert_eq!(final_value, "Receipt 中文");
}

/// The same Worker factory must ignore inherited executable defaults from its first init. Its
/// actual actor test above keeps Manager children inside the same private process environment.
#[test]
fn worker_fixture_ignores_external_hooks_monitors_and_templates() {
    super::super::super::git_fixture_environment::check_external_configuration(
        "extensions::worker::form_delivery::tests::worker_fixture_ignores_external_hooks_monitors_and_templates",
        |_| {
            let repository = tempfile::tempdir().unwrap();
            let root = repository.path();
            git(root, &["init", "-b", "main"]);
            std::fs::write(root.join("tracked.txt"), b"owned Worker fixture\n").unwrap();
            git(root, &["add", "--", "tracked.txt"]);
            git(
                root,
                &[
                    "-c",
                    "user.name=Worker fixture",
                    "-c",
                    "user.email=worker@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "commit",
                    "-m",
                    "configuration isolation",
                ],
            );
            git(root, &["status", "--porcelain"]);
            assert!(!root.join(".git/template-marker.txt").exists());
            assert_eq!(
                std::fs::read(root.join("tracked.txt")).unwrap(),
                b"owned Worker fixture\n"
            );
        },
    );
}
