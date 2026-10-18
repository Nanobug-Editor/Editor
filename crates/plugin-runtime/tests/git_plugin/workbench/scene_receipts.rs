//! Non-business receipts retain a painted frame's owned history input without relaxing admission.
use super::*;

/// Send one native key at the sampled scene; callers deliberately retain the original envelope.
fn key(manager: &Manager, name: &str) -> api::Notification {
    api::Notification::Ui(ui::UiEvent {
        revision: manager.live["git"].views["history"].revision,
        node: "log".into(),
        action: ui::Action::Canvas(ui::CanvasEvent::Key {
            key: name.into(),
            ctrl: false,
            alt: false,
            shift: false,
        }),
    })
}

/// Capture the guest's actual visibility call after the real Git history-file query completes.
fn visibility_request(manager: &mut Manager) -> plugin_runtime::EditorRequest {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        manager.poll();
        for (_, request) in manager.take_editor_requests() {
            match request.operation() {
                api::EditorOperation::SetPanelVisibility { panel, visible }
                    if panel == "commit-details" && *visible =>
                {
                    assert_eq!(manager.live["git"].process_count(), 0);
                    return request;
                }
                operation => {
                    panic!("unexpected native request while opening history: {operation:?}")
                }
            }
        }
        assert!(
            Instant::now() < deadline,
            "history never requested its actual native window"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Published detail identity is the exact Git OID, rather than an abbreviated title or row index.
fn identity(manager: &Manager) -> &str {
    let ui::Kind::Text { text } = &manager.live["git"].views["commit-details"]
        .active_node("commit-identity")
        .unwrap()
        .kind
    else {
        panic!("history identity is not visible text");
    };
    text
}

/// The success receipt only retires its request; a saved key from the same frame must remain
/// valid once, while a failed receipt must still publish its visible error normally.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real visibility ack and admitted key"]
fn git_package_visibility_ack_preserves_an_admitted_history_key_and_exact_oid() {
    if !run_isolated_test(
        "workbench::scene_receipts::git_package_visibility_ack_preserves_an_admitted_history_key_and_exact_oid",
    ) {
        return;
    }
    let repository = repository();
    let root = repository.path();
    let first = commit_file(root, "tracked.txt", "first\n", "first");
    let second = commit_file(root, "tracked.txt", "second\n", "second");
    commit_file(root, "tracked.txt", "third\n", "third");
    let (_profile, mut manager) = activate(&candidate(), root);
    for name in ["home", "down", "enter"] {
        let event = key(&manager, name);
        manager.event("git", Some("history".into()), event).unwrap();
    }
    let request = visibility_request(&mut manager);
    assert_eq!(identity(&manager), second);
    let saved_key = key(&manager, "down");
    let revision = manager.live["git"].views["history"].revision;
    let resources = manager.live["git"].resource_count();
    request.finish(Ok(api::EditorValue::PanelVisibility {
        panel: "commit-details".into(),
        visible: true,
    }));
    manager.poll();
    assert_eq!(
        manager.live["git"].resource_count() + 1,
        resources,
        "the actual visibility request must be consumed and closed before delivering the key"
    );
    let after_ack = manager.live["git"].views["history"].revision;
    let delivered = manager.event("git", Some("history".into()), saved_key);
    assert!(
        delivered.is_ok(),
        "a successful visibility ack replaced scene {revision} with {after_ack} and rejected its one saved key: {delivered:?}"
    );
    assert_eq!(
        after_ack, revision,
        "a visibility success owns no replacement scene"
    );
    let enter = key(&manager, "enter");
    manager.event("git", Some("history".into()), enter).unwrap();
    idle(&mut manager);
    assert_eq!(
        identity(&manager),
        first,
        "the old-frame down key must move to the exact preceding Git commit"
    );

    let enter = key(&manager, "enter");
    manager.event("git", Some("history".into()), enter).unwrap();
    let request = visibility_request(&mut manager);
    let revision = manager.live["git"].views["history"].revision;
    request.finish(Err(api::Failure::new(
        api::ErrorCode::OperationFailed,
        "fixture visibility denied",
    )));
    manager.poll();
    assert!(manager.live["git"].views["history"].revision > revision);
    assert!(matches!(
        &manager.live["git"].views["commit-details"].active_node("window-status").unwrap().kind,
        ui::Kind::Text { text } if text.contains("fixture visibility denied")
    ));
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}
