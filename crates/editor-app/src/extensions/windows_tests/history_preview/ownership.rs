//! Repeated real package selections retain valid readonly source ownership after preview replacement.
use super::*;

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
