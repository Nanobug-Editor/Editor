//! Real WASM callbacks must revoke old approvals without saving or replacing native document text.
use super::*;

/// Keep a native buffer observation independent from the bytes Git reads from the fixture disk.
fn document(dirty: bool) -> api::DocumentInfo {
    api::DocumentInfo {
        document: api::DocumentVersion {
            id: "native-workbench-buffer".into(),
            path: "tracked.txt".into(),
            revision: if dirty { 9 } else { 10 },
        },
        resource: api::ResourceIdentity::Local {
            path: "tracked.txt".into(),
        },
        title: "tracked.txt".into(),
        access: api::DocumentAccess {
            read: true,
            edit: true,
            save: true,
        },
        dirty,
        language: "text".into(),
        encoding: api::DocumentEncoding::Utf8,
        eol: api::DocumentEol::Lf,
        byte_len: 12,
        selection: api::TextRange { start: 0, end: 0 },
        visible_rows: None,
    }
}

fn observe(request: &plugin_runtime::EditorRequest, dirty: bool) {
    let document = document(dirty);
    request.finish(Ok(api::EditorValue::Documents {
        active: Some(document.document.clone()),
        documents: vec![document],
    }));
}

/// Leave one actual native call outstanding; any attempt to save/edit is a visible failure.
fn next_check(manager: &mut Manager) -> plugin_runtime::EditorRequest {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        manager.poll();
        for (_, request) in manager.take_editor_requests() {
            match request.operation() {
                api::EditorOperation::ListDocuments => return request,
                api::EditorOperation::SetPanelVisibility { panel, visible } => {
                    request.finish(Ok(api::EditorValue::PanelVisibility {
                        panel: panel.clone(),
                        visible: *visible,
                    }));
                }
                other => panic!("a buffer guard cannot save or edit native text: {other:?}"),
            }
        }
        assert!(
            Instant::now() < deadline,
            "native buffer check was not requested"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Unlike `send`, this returns before a ListDocuments receipt has been delivered.
fn press(manager: &mut Manager, node: &str) {
    let revision = manager.live["git"].views["history"].revision;
    manager
        .event(
            "git",
            Some("history".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: node.into(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
}

fn preview(manager: &Manager) -> String {
    let mut text = String::new();
    manager.live["git"].views["history"]
        .dialog
        .as_ref()
        .expect("confirmation required")
        .content
        .visit(&mut |node| {
            if let ui::Kind::Text { text: content } = &node.kind
                && node.id.starts_with("preview-")
            {
                text.push_str(content);
            }
        });
    text
}

fn reset_review(manager: &mut Manager, target: &str) {
    branch_menu::context_action(manager, "reset-hard", Some(target));
    assert!(
        manager.live["git"].views["history"]
            .active_node("confirm-command")
            .is_some()
    );
}

/// The target has a tracked obstruction which exists only after a manual native save.
fn fixture() -> (tempfile::TempDir, String, String) {
    let repository = repository();
    let root = repository.path();
    std::fs::write(root.join("tracked.txt"), "target tracked\n").unwrap();
    std::fs::write(root.join("after-save.txt"), "target obstruction\n").unwrap();
    git(root, &["add", "--", "."]);
    git(root, &["commit", "-m", "target"]);
    let target = git(root, &["rev-parse", "HEAD"]).trim().to_owned();
    git(root, &["rm", "--", "after-save.txt"]);
    std::fs::write(root.join("tracked.txt"), "current tracked\n").unwrap();
    git(root, &["add", "--", "."]);
    git(root, &["commit", "-m", "current"]);
    let original = git(root, &["rev-parse", "HEAD"]).trim().to_owned();
    (repository, target, original)
}

#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real native editor receipts"]
fn git_package_saving_after_dirty_guard_requires_second_impact_review_and_confirmation() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::native_guards::git_package_saving_after_dirty_guard_requires_second_impact_review_and_confirmation",
    ) {
        return;
    }
    let (repository, target, original) = fixture();
    let root = repository.path();
    let (_profile, mut manager) = activate(&candidate(), root);
    reset_review(&mut manager, &target);
    assert!(!preview(&manager).contains("\"after-save.txt\""));
    press(&mut manager, "confirm-command");
    observe(&next_check(&mut manager), true);
    idle(&mut manager);
    assert_eq!(git(root, &["rev-parse", "HEAD"]).trim(), original);
    std::fs::write(root.join("after-save.txt"), "manually saved native text\n").unwrap();
    std::fs::write(root.join("tracked.txt"), "manually saved tracked text\n").unwrap();
    click(&mut manager, "history", "confirm-command");
    assert_eq!(
        git(root, &["rev-parse", "HEAD"]).trim(),
        original,
        "rebuilding an impact review cannot execute the old approval"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("after-save.txt")).unwrap(),
        "manually saved native text\n"
    );
    assert!(preview(&manager).contains("\"after-save.txt\""));
    press(&mut manager, "confirm-command");
    observe(&next_check(&mut manager), false);
    idle(&mut manager);
    assert_eq!(git(root, &["rev-parse", "HEAD"]).trim(), target);
    assert_eq!(
        std::fs::read_to_string(root.join("after-save.txt")).unwrap(),
        "target obstruction\n"
    );
    manager.disable("git").unwrap();
}

#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; delayed native editor receipts"]
fn git_package_cancelled_buffer_receipt_cannot_approve_a_new_write_intent() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "workbench::native_guards::git_package_cancelled_buffer_receipt_cannot_approve_a_new_write_intent",
    ) {
        return;
    }
    let (repository, target, original) = fixture();
    let root = repository.path();
    let (_profile, mut manager) = activate(&candidate(), root);
    for cancelled in [false, true] {
        reset_review(&mut manager, &target);
        press(&mut manager, "confirm-command");
        let old = next_check(&mut manager);
        click(&mut manager, "history", "cancel-process");
        reset_review(&mut manager, &target);
        press(&mut manager, "confirm-command");
        let current = next_check(&mut manager);
        assert_ne!(old.handle(), current.handle());
        let resources = manager.live["git"].resource_count();
        if cancelled {
            old.cancel_from_host(api::CancelMode::StopWaiting);
        } else {
            observe(&old, false);
        }
        idle(&mut manager);
        assert_eq!(
            manager.live["git"].resource_count(),
            resources - 1,
            "both old terminal states close their owned request resource"
        );
        assert_eq!(
            git(root, &["rev-parse", "HEAD"]).trim(),
            original,
            "a cancelled clean receipt cannot release the newer, still-unobserved write"
        );
        assert!(!current.status().is_terminal());
        observe(&current, true);
        idle(&mut manager);
        assert_eq!(git(root, &["rev-parse", "HEAD"]).trim(), original);
        assert_eq!(
            std::fs::read_to_string(root.join("tracked.txt")).unwrap(),
            "current tracked\n"
        );
        click(&mut manager, "history", "cancel-command");
    }
    manager.disable("git").unwrap();
    assert!(!manager.live.contains_key("git"));
}
