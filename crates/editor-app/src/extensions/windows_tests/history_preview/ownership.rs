//! Repeated real package selections retain valid readonly source ownership after preview replacement.
use super::*;
mod stress;

/// A stream diagnostic belongs to host messages; unrelated loading/status text remains valid.
fn assert_no_inline_stream_failure(manager: &Manager) {
    for id in ["history-error", "history-status"] {
        let text = manager.live["git"].views["history"]
            .active_node(id)
            .and_then(|node| match &node.kind {
                ui::Kind::Text { text } => Some(text.as_str()),
                _ => None,
            });
        assert!(
            !text.is_some_and(|text| text.contains("Document stream overflow")),
            "document stream failure must be handled by host messages, not {id}: {text:?}"
        );
    }
}

/// A real guest publication must end its request after host retention, independently of its alert.
fn assert_stream_warning_published(
    manager: &Manager,
    app: &Entity<EditorApp>,
    visual: &mut VisualTestContext,
) {
    let expected = "Git\nLimitExceeded: Document stream overflow; enumerate and resubscribe";
    let notices = manager
        .runtime_logs()
        .records("git")
        .into_iter()
        .filter(|record| record.source == "host.message" && record.message == expected)
        .collect::<Vec<_>>();
    assert_eq!(
        notices.len(),
        1,
        "stream failure must have one exact host receipt: {notices:?}"
    );
    assert_eq!(notices[0].level, plugin_runtime::LogLevel::Warning);
    assert_eq!(
        manager.live["git"].resource_counts().requests,
        0,
        "message retention must complete its public request before alert dismissal"
    );
    visual.update(|_, cx| {
        let state = app.read(cx);
        assert_eq!(
            state.messages.read(cx).pending_level(),
            Some(crate::app::messages::MessageLevel::Warning)
        );
        assert_eq!(state.message_delivery.notifications(cx).count(), 1);
        assert!(
            state.plugin_interactions.is_empty(),
            "message publication must not create a pending Notify interaction"
        );
        assert!(
            state
                .extensions
                .read(cx)
                .worker
                .state
                .lock()
                .unwrap()
                .editor_requests
                .is_empty()
        );
    });
    assert!(
        visual.debug_bounds("host-message-toast").is_some(),
        "the host warning must have a visible native alert"
    );
}

/// Native comparison admission can outlive a quiet scene; wait for the requested actual editor.
fn await_file_preview(
    manager: &mut Manager,
    app: &Entity<EditorApp>,
    visual: &mut VisualTestContext,
    file: &str,
) {
    let expected = format!("changed {file}\n");
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        flush(manager, app, visual);
        let (count, text, host, worker, worker_requests) = visual.update(|_, cx| {
            let state = app.read(cx);
            let worker = state.extensions.read(cx).worker.state.lock().unwrap();
            (
                state.comparison_views.len(),
                state.editor.read(cx).value().to_string(),
                state.status.clone(),
                worker.status.as_ref().map(|status| status.message.clone()),
                worker.editor_requests.len(),
            )
        });
        if count == 1 && text == expected {
            assert_no_inline_stream_failure(manager);
            return;
        }
        let guest = manager.live["git"].views["history"]
            .active_node("history-error")
            .map(|node| format!("{:?}", node.kind));
        let scene = ["history-status", "history-loading", "history-oid", "files"]
            .into_iter()
            .filter_map(|id| {
                manager.live["git"].views["history"]
                    .active_node(id)
                    .map(|node| format!("{id}: {node:?}"))
            })
            .collect::<Vec<_>>();
        assert!(
            guest.is_none() && Instant::now() < deadline,
            "preview must settle on {file}: views={count}, text={text:?}, host={host}, worker={worker:?}, worker_requests={worker_requests}, guest={guest:?}, scene={scene:?}, diagnostics={:?}, observations={:?}",
            manager.diagnostics("git"),
            manager.runtime_observations().snapshot(Some("git")),
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Each click uses the currently rendered tree and then observes the exact native right source.
#[gpui::test]
#[ignore = "actual independent Git ZIP; set NANOBUG_GIT_CANDIDATE"]
fn git_history_repeated_file_selection_keeps_preview_sources_owned(cx: &mut TestAppContext) {
    if !run_isolated_test(
        "extensions::windows_tests::history_preview::ownership::git_history_repeated_file_selection_keeps_preview_sources_owned",
    ) {
        return;
    }
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "core.autocrlf", "false"],
            vec!["config", "user.name", "Ownership fixture"],
            vec!["config", "user.email", "ownership@example.invalid"],
        ] {
            assert!(git(root, &args));
        }
        for file in ["first.txt", "second.txt", "third.txt"] {
            std::fs::write(root.join(file), format!("base {file}\n")).unwrap();
        }
        assert!(git(
            root,
            &["add", "--", "first.txt", "second.txt", "third.txt"]
        ));
        assert!(git(root, &["commit", "-m", "base"]));
        for file in ["first.txt", "second.txt", "third.txt"] {
            std::fs::write(root.join(file), format!("changed {file}\n")).unwrap();
        }
        assert!(git(root, &["commit", "-am", "changed"]));
        let package = Package::read(Path::new(
            &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate ZIP"),
        ))
        .unwrap();
        let profile = tempfile::tempdir().unwrap();
        let env = visual.update(|_, cx| environment(root, cx));
        let mut manager = Manager::open(profile.path().into(), env).unwrap();
        // Match the production actor's shared log publication before the guest can emit a message.
        visual.update(|_, cx| {
            app.read(cx)
                .extensions
                .read(cx)
                .worker
                .state
                .lock()
                .unwrap()
                .logs = manager.runtime_logs();
        });
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        package_ui_test_support::demand(
            &mut manager,
            "git",
            plugin_runtime::DemandReason::View("history".into()),
        )
        .unwrap();
        flush(&mut manager, &app, visual);
        select_row(&mut manager, 0);
        flush(&mut manager, &app, visual);
        for file in [
            "first.txt",
            "second.txt",
            "first.txt",
            "third.txt",
            "second.txt",
            "first.txt",
        ] {
            let node = manager.live["git"]
                .tree_resources()
                .into_iter()
                .flat_map(|tree| tree.snapshot().nodes.into_values())
                .find(|node| !node.branch && node.label == file)
                .expect("published changed file")
                .id;
            press(visual, format!("plugin-tree-row-{node}"), 1);
            await_file_preview(&mut manager, &app, visual, file);
            visual.update(|_, cx| {
                let app = app.read(cx);
                assert_eq!(
                    app.comparison_views.len(),
                    1,
                    "one preview after selecting {file}"
                );
                assert_eq!(
                    app.editor.read(cx).value().as_str(),
                    format!("changed {file}\n"),
                    "repeated selection must switch the actual preview to {file}"
                );
            });
        }
        // Queue genuine tree clicks without waiting for each native/Git preview to complete.
        // Older source receipts must never override the final requested file after settling.
        for file in ["second.txt", "first.txt", "second.txt", "third.txt"] {
            let node = manager.live["git"]
                .tree_resources()
                .into_iter()
                .flat_map(|tree| tree.snapshot().nodes.into_values())
                .find(|node| !node.branch && node.label == file)
                .expect("published file during rapid selection")
                .id;
            press(visual, format!("plugin-tree-row-{node}"), 1);
        }
        await_file_preview(&mut manager, &app, visual, "third.txt");
        visual.update(|_, cx| {
            assert_eq!(
                app.read(cx).editor.read(cx).value().as_str(),
                "changed third.txt\n",
                "rapid alternating clicks must leave the last requested file visible"
            );
        });
        // A native metadata queue can overflow while the UI keeps drawing and replacing previews.
        // Use the production terminal stream failure; no test bypasses ownership or invents handles.
        manager.document_stream_failed(api::Failure::new(
            api::ErrorCode::LimitExceeded,
            "Document stream overflow; enumerate and resubscribe",
        ));
        flush(&mut manager, &app, visual);
        // The former regression only inspected history-error. The screenshot's warning lived
        // in history-status and survived recovery, even while every native source assertion passed.
        assert_no_inline_stream_failure(&manager);
        assert_stream_warning_published(&manager, &app, visual);
        for file in ["second.txt", "first.txt", "third.txt", "second.txt"] {
            let node = manager.live["git"]
                .tree_resources()
                .into_iter()
                .flat_map(|tree| tree.snapshot().nodes.into_values())
                .find(|node| !node.branch && node.label == file)
                .expect("published changed file after metadata recovery")
                .id;
            press(visual, format!("plugin-tree-row-{node}"), 1);
            await_file_preview(&mut manager, &app, visual, file);
            let diagnostic = manager.live["git"].views["history"]
                .active_node("history-error")
                .map(|node| format!("{:?}", node.kind));
            visual.update(|_, cx| {
                assert_eq!(
                    app.read(cx).editor.read(cx).value().as_str(),
                    format!("changed {file}\n"),
                    "metadata stream loss must not leave a stale source blocking {file}: {diagnostic:?}"
                );
            });
        }
        manager.disable("git").unwrap();
        flush(&mut manager, &app, visual);
    });
}
