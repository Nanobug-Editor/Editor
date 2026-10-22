//! Real visible timer refreshes yield to current history input and resume without weakening stale admission.
use super::*;

/// Capture one ordinary native key at the current public scene, without polling or settling afterward.
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

/// Observe actual canvas text rather than a guest counter or an internal list of commits.
fn painted(manager: &Manager, title: &str) -> bool {
    let ui::Kind::Canvas(canvas) = &manager.live["git"].views["history"]
        .active_node("log")
        .expect("enabled history canvas")
        .kind
    else {
        panic!("history must expose its native canvas");
    };
    canvas.paint.iter().any(|paint| {
        matches!(paint, plugin_runtime::plugin_protocol::Paint::Text { text, .. } if text == title)
    })
}

/// Finish only normal native queries during automatic refresh; true means their receipts need another poll.
fn poll_native_receipts(manager: &mut Manager) -> bool {
    manager.poll();
    let requests = manager.take_editor_requests();
    let had_requests = !requests.is_empty();
    for (_, request) in requests {
        match request.operation().clone() {
            api::EditorOperation::ListDocuments => {
                request.finish(Ok(api::EditorValue::Documents {
                    documents: vec![],
                    active: None,
                }));
            }
            api::EditorOperation::SetPanelVisibility { panel, visible } => {
                request.finish(Ok(api::EditorValue::PanelVisibility { panel, visible }));
            }
            operation => panic!("unexpected quiet-history native request: {operation:?}"),
        }
    }
    had_requests
}

/// Ongoing admitted keys keep the visible frame usable across real timer ticks; once input stops,
/// automatic refresh finds a real new commit, revokes old input, and explicit focus still refreshes promptly.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real visible timer and quiet interval"]
fn git_package_history_activity_defers_automatic_refresh_until_quiet() {
    if !run_isolated_test(
        "workbench::scene_activity::git_package_history_activity_defers_automatic_refresh_until_quiet",
    ) {
        return;
    }
    let repository = repository();
    let root = repository.path();
    commit_file(root, "tracked.txt", "first\n", "first");
    commit_file(root, "tracked.txt", "second\n", "second");
    commit_file(root, "tracked.txt", "third\n", "third");
    let (_profile, mut manager) = activate(&candidate(), root);
    manager
        .event(
            "git",
            Some("history".into()),
            api::Notification::PanelVisibility {
                panel: "history".into(),
                visible: true,
            },
        )
        .unwrap();
    idle(&mut manager);
    // A mounted native canvas supplies its viewport before keyboard review. All four
    // fixture commits fit here, so the inserted top row stays visible throughout the test.
    send(
        &mut manager,
        "history",
        "log",
        ui::Action::Canvas(ui::CanvasEvent::Resize {
            width: 900.,
            height: 240.,
            grid: None,
        }),
    );
    assert!(painted(&manager, "third"));
    assert_eq!(manager.live["git"].process_count(), 0);
    assert!(manager.take_editor_requests().is_empty());

    // Direct key/poll pairs cross two thirty-second timer deadlines without idle gaps.
    // Repeat a boundary Home key: it expresses review activity without selecting another
    // commit whose legitimate preview query would be confused with background Git work.
    // The ordinary send helper would settle the queue and could mask input interruption.
    let active = Instant::now();
    let mut inputs = 0;
    while active.elapsed() < Duration::from_secs(65) {
        let name = "home";
        let event = key(&manager, name);
        manager.event("git", Some("history".into()), event).unwrap();
        let revision = manager.live["git"].views["history"].revision;
        assert_eq!(manager.live["git"].process_count(), 0);
        manager.poll();
        assert_eq!(
            manager.live["git"].views["history"].revision,
            revision,
            "automatic refresh replaced an active history frame after {:?}; key={name}, processes={}",
            active.elapsed(),
            manager.live["git"].process_count(),
        );
        assert_eq!(
            manager.live["git"].process_count(),
            0,
            "automatic refresh must yield while current history keys continue"
        );
        assert!(manager.take_editor_requests().is_empty());
        inputs += 1;
        std::thread::sleep(Duration::from_millis(50));
    }
    eprintln!(
        "visible history remained current for {}ms and {inputs} independent keys",
        active.elapsed().as_millis()
    );

    // Preserve the old envelope before an independent repository change. No UI input is
    // sent while waiting, so the real timer must eventually resume and publish the new data.
    let old_key = key(&manager, "home");
    let fresh_title = "external commit after quiet history";
    assert!(!painted(&manager, fresh_title));
    commit_file(root, "tracked.txt", "external\n", fresh_title);
    let quiet = Instant::now();
    let deadline = quiet + Duration::from_secs(75);
    loop {
        let had_requests = poll_native_receipts(&mut manager);
        if !had_requests
            && manager.live["git"].process_count() == 0
            && painted(&manager, fresh_title)
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "visible automatic refresh did not paint the external commit within 75 seconds"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let error = manager
        .event("git", Some("history".into()), old_key)
        .expect_err("a real automatic data update must revoke the old scene");
    assert_eq!(
        error
            .downcast_ref::<api::Failure>()
            .expect("typed stale rejection")
            .code,
        api::ErrorCode::StaleRevision
    );
    eprintln!(
        "automatic refresh painted the external commit after {}ms and rejected the old key",
        quiet.elapsed().as_millis()
    );

    // Home may legitimately select the newly inserted commit first. Finish that business
    // preview before a second boundary key establishes a quiet interval without Git work.
    let select = key(&manager, "home");
    manager
        .event("git", Some("history".into()), select)
        .unwrap();
    idle(&mut manager);
    let recent = key(&manager, "home");
    manager
        .event("git", Some("history".into()), recent)
        .unwrap();
    assert_eq!(manager.live["git"].process_count(), 0);
    manager
        .event("git", None, api::Notification::Focus(true))
        .unwrap();
    assert!(
        manager.live["git"].process_count() > 0,
        "explicit focus must start refresh immediately despite recent history input"
    );
    idle(&mut manager);
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}
