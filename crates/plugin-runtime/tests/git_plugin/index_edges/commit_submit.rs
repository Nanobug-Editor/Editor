//! Real package submission latency and commit/push ordering use only disposable local repositories.
use super::*;

/// Change the actual composer input without involving a history selection or source document.
pub(super) fn draft(manager: &mut Manager, text: &str) {
    let revision = manager.live["git"].views["staging"].revision;
    manager
        .event(
            "git",
            Some("staging".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: "commit-message".into(),
                action: ui::Action::Change(text.into()),
            }),
        )
        .unwrap();
}

/// Deliver a click without answering native enumeration; tests can interleave real external changes.
pub(super) fn start_submit(manager: &mut Manager, node: &str) {
    let revision = manager.live["git"].views["staging"].revision;
    manager
        .event(
            "git",
            Some("staging".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: node.into(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
}

/// Measure input-to-enabled latency, excluding fixture activation and the helper's idle debounce.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE"]
fn git_package_commit_checkbox_to_enabled_latency() {
    if !run_isolated_test(
        "index_edges::commit_submit::git_package_commit_checkbox_to_enabled_latency",
    ) {
        return;
    }
    let repository = fixture(false);
    let root = repository.path();
    std::fs::write(root.join("tracked.txt"), "changed\n").unwrap();
    let (_profile, mut manager) = activate(&candidate(), root);
    draft(&mut manager, "latency fixture");
    let mut timings = vec![];
    for _ in 0..3 {
        let resource = commit_panel::load_tree(&mut manager);
        let node = resource
            .snapshot()
            .nodes
            .values()
            .find(|node| node.label == "tracked.txt")
            .unwrap()
            .id
            .clone();
        assert!(
            manager.live["git"].views["staging"]
                .active_node("commit")
                .is_none()
        );
        let start = Instant::now();
        commit_panel::check(&mut manager, &resource, &node);
        for action in ["commit", "commit-push"] {
            assert!(
                manager.live["git"].views["staging"]
                    .active_node(action)
                    .is_some(),
                "{action} waited beyond the checkbox receipt"
            );
        }
        timings.push(start.elapsed().as_millis());
        assert_eq!(manager.live["git"].process_count(), 0);
        assert!(manager.take_editor_requests().is_empty());
        assert_eq!(git(root, &["show", ":tracked.txt"]), "base\n");
        idle(&mut manager);
        commit_panel::check(&mut manager, &resource, &node);
        idle(&mut manager);
    }
    eprintln!("checkbox-to-enabled milliseconds (3 samples): {timings:?}");
    // The isolated child normally hides successful stdout; optionally retain comparable benchmark
    // evidence at a caller-selected absolute artifact path without changing production telemetry.
    if let Some(path) = std::env::var_os("NANOBUG_GIT_LATENCY_REPORT") {
        let path = std::path::PathBuf::from(path);
        assert!(path.is_absolute());
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&json!({
                "plugin_version": candidate().manifest.version,
                "checkbox_to_enabled_ms": timings,
            }))
            .unwrap(),
        )
        .unwrap();
    }
    manager.disable("git").unwrap();
}

/// A local bare remote proves ordering; selected whole files commit disk bytes and excluded index stays.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE"]
fn git_package_commit_and_push_preserves_excluded_index_and_local_commit_on_rejection() {
    if !run_isolated_test(
        "index_edges::commit_submit::git_package_commit_and_push_preserves_excluded_index_and_local_commit_on_rejection",
    ) {
        return;
    }
    for reject in [false, true] {
        let repository = fixture(false);
        let remote = tempfile::tempdir().unwrap();
        let root = repository.path();
        git(remote.path(), &["init", "--bare"]);
        git(
            root,
            &["remote", "add", "origin", remote.path().to_str().unwrap()],
        );
        git(root, &["push", "-u", "origin", "main"]);
        let before = git(root, &["rev-parse", "HEAD"]);
        if reject {
            // A server-side policy failure is deterministic and never invokes user hooks or credentials.
            git(
                remote.path(),
                &["config", "receive.denyCurrentBranch", "refuse"],
            );
            git(remote.path(), &["config", "core.bare", "false"]);
            git(remote.path(), &["symbolic-ref", "HEAD", "refs/heads/main"]);
        }
        std::fs::write(root.join("tracked.txt"), "staged\n").unwrap();
        git(root, &["add", "--", "tracked.txt"]);
        std::fs::write(root.join("tracked.txt"), "unstaged\n").unwrap();
        std::fs::write(root.join("excluded.txt"), "excluded staged bytes\n").unwrap();
        git(root, &["add", "--", "excluded.txt"]);
        let (_profile, mut manager) = activate(&candidate(), root);
        draft(&mut manager, "提交并推送 fixture");
        let resource = commit_panel::load_tree(&mut manager);
        let node = resource
            .snapshot()
            .nodes
            .values()
            .find(|node| node.label == "tracked.txt")
            .unwrap()
            .id
            .clone();
        commit_panel::check(&mut manager, &resource, &node);
        assert!(
            manager.live["git"].views["staging"]
                .active_node("commit-push")
                .is_some()
        );
        start_submit(&mut manager, "commit-push");
        // Accepted work keeps its own message/range. Later draft/range gestures target a future
        // submission, and a duplicate submit click cannot broaden or repeat the current commit.
        draft(&mut manager, "next draft stays here");
        let excluded = resource
            .snapshot()
            .nodes
            .values()
            .find(|node| node.label == "excluded.txt")
            .unwrap()
            .id
            .clone();
        commit_panel::check(&mut manager, &resource, &excluded);
        commit_panel::check(&mut manager, &resource, &node);
        let revision = manager.live["git"].views["staging"].revision;
        assert!(
            manager
                .event(
                    "git",
                    Some("staging".into()),
                    api::Notification::Ui(ui::UiEvent {
                        revision,
                        node: "commit-push".into(),
                        action: ui::Action::Click,
                    })
                )
                .is_err(),
            "an in-flight submit action must be rejected by native admission"
        );
        idle(&mut manager);
        let after = git(root, &["rev-parse", "HEAD"]);
        assert_ne!(after, before);
        assert_eq!(git(root, &["rev-parse", "HEAD^"]), before);
        assert_eq!(
            git(root, &["show", "-s", "--format=%B", "HEAD"]).trim(),
            "提交并推送 fixture"
        );
        assert_eq!(git(root, &["show", "HEAD:tracked.txt"]), "unstaged\n");
        assert_eq!(
            std::fs::read_to_string(root.join("tracked.txt")).unwrap(),
            "unstaged\n"
        );
        assert_eq!(
            git(root, &["diff", "--cached", "--name-only"]),
            "excluded.txt\n"
        );
        assert_eq!(
            git(root, &["show", ":excluded.txt"]),
            "excluded staged bytes\n"
        );
        assert_eq!(
            git(remote.path(), &["rev-parse", "refs/heads/main"]),
            if reject { before } else { after }
        );
        let dialog = &manager.live["git"].views["staging"].dialog;
        if reject {
            let text =
                serde_json::to_string(dialog.as_ref().expect("push rejection is visible")).unwrap();
            assert!(
                text.contains("Commit succeeded, push failed")
                    && text.contains("local commit retained")
            );
            click(&mut manager, "staging", "commit-submit-error-close");
            assert!(manager.live["git"].views["staging"].dialog.is_none());
        } else {
            assert!(dialog.is_none());
        }
        assert!(
            matches!(&manager.live["git"].views["staging"].active_node("commit-message").unwrap().kind, ui::Kind::Textarea(input) if input.value == "next draft stays here")
        );
        manager.disable("git").unwrap();
        assert_eq!(manager.resource_count(), 0);
    }
}

/// Failure to acquire the real index lock must not create a commit or push a preexisting local tip.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE"]
fn git_package_failed_commit_never_pushes() {
    if !run_isolated_test("index_edges::commit_submit::git_package_failed_commit_never_pushes") {
        return;
    }
    let repository = fixture(false);
    let remote = tempfile::tempdir().unwrap();
    let root = repository.path();
    git(remote.path(), &["init", "--bare"]);
    git(
        root,
        &["remote", "add", "origin", remote.path().to_str().unwrap()],
    );
    git(root, &["push", "-u", "origin", "main"]);
    let remote_before = git(remote.path(), &["rev-parse", "refs/heads/main"]);
    git(root, &["commit", "--allow-empty", "-m", "unpublished"]);
    let local_before = git(root, &["rev-parse", "HEAD"]);
    std::fs::write(root.join("tracked.txt"), "staged\n").unwrap();
    git(root, &["add", "--", "tracked.txt"]);
    let (_profile, mut manager) = activate(&candidate(), root);
    draft(&mut manager, "will fail");
    let resource = commit_panel::load_tree(&mut manager);
    let node = resource
        .snapshot()
        .nodes
        .values()
        .find(|node| node.label == "tracked.txt")
        .unwrap()
        .id
        .clone();
    commit_panel::check(&mut manager, &resource, &node);
    std::fs::write(root.join(".git/index.lock"), b"fixture-owned lock").unwrap();
    click(&mut manager, "staging", "commit-push");
    assert_eq!(git(root, &["rev-parse", "HEAD"]), local_before);
    assert_eq!(
        git(remote.path(), &["rev-parse", "refs/heads/main"]),
        remote_before
    );
    assert!(manager.live["git"].views["staging"].dialog.is_some());
    assert_eq!(
        std::fs::read(root.join(".git/index.lock")).unwrap(),
        b"fixture-owned lock"
    );
    manager.disable("git").unwrap();
}
