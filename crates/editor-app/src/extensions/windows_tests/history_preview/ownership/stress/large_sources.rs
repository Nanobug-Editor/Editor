//! Large legal native sources must never be copied into invisible secondary plugin editors.
use super::*;

/// Match Git's supported per-blob 256 KiB boundary and stay below the host's 1 MiB source limit.
/// Larger inputs exercise Git's unsupported-file branch instead of native source serialization.
const SOURCE_BYTES: usize = 256 * 1024;

/// Ordinary quote/backslash-containing source lines exercise the real JSON snapshot encoder.
fn large_content(file: &str, version: &str) -> String {
    let mut text = format!("// {version}: {file}\n");
    let line = "let config = { \"path\": \"C:\\\\Workspace\\\\src\\\\module.rs\", \"message\": \"retained native comparison source\" };\n";
    while text.len() + line.len() < SOURCE_BYTES {
        text.push_str(line);
    }
    text.extend(std::iter::repeat_n(' ', SOURCE_BYTES - text.len() - 1));
    text.push('\n');
    assert_eq!(text.len(), SOURCE_BYTES);
    assert!(text.lines().count() < 100_000);
    text
}

/// An inactive secondary window has no ownership reason to publish the native comparison's bytes.
fn assert_hidden_sources_absent(manager: &Manager) {
    for panel in ["commit-details", "conflicts"] {
        let Some(document) = manager.live["git"].views.get(panel) else {
            continue;
        };
        document.root.visit(&mut |node| {
            if let ui::Kind::Editor(editor) = &node.kind
                && let ui::EditorSource::Snapshot { text } = &editor.source
            {
                assert!(
                    text.is_empty(),
                    "hidden {panel} published {} source bytes while native comparison owns the source",
                    text.len()
                );
            }
        });
    }
}

/// Three real tree selections cover cold acquisition, replacement and reopening without user-data writes.
#[gpui::test]
#[ignore = "actual independent Git ZIP and native comparison; set NANOBUG_GIT_CANDIDATE"]
fn git_large_history_sources_do_not_serialize_hidden_panels(cx: &mut TestAppContext) {
    if !run_isolated_test(
        "extensions::windows_tests::history_preview::ownership::stress::large_sources::git_large_history_sources_do_not_serialize_hidden_panels",
    ) {
        return;
    }
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "core.autocrlf", "false"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["config", "user.name", "Large history source"],
            vec!["config", "user.email", "large-source@example.invalid"],
        ] {
            assert!(git(root, &args));
        }
        for version in ["base", "changed"] {
            for file in ["first-large.txt", "second-large.txt"] {
                std::fs::write(root.join(file), large_content(file, version)).unwrap();
            }
            assert!(git(
                root,
                &["add", "--", "first-large.txt", "second-large.txt"]
            ));
            assert!(git(root, &["commit", "-m", version]));
        }
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
        let incarnation = manager.instance_id("git").unwrap().to_owned();
        let observations = manager.runtime_observations();
        let mut peaks = Peaks::default();
        observations.start_capture();
        for file in ["first-large.txt", "second-large.txt", "first-large.txt"] {
            eprintln!("large history native click: {file}; source_bytes={SOURCE_BYTES}");
            click_file(&manager, visual, file);
            let expected = large_content(file, "changed");
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                flush_checked(&mut manager, &app, visual, |manager, _| {
                    peaks.inspect(manager, &incarnation);
                    assert_hidden_sources_absent(manager);
                });
                let visible = visual.update(|_, cx| {
                    let state = app.read(cx);
                    state.comparison_views.len() == 1
                        && state.editor.read(cx).value().as_str() == expected
                });
                if visible {
                    break;
                }
                if Instant::now() >= deadline {
                    let (comparisons, actual_bytes, host, worker, pending) =
                        visual.update(|_, cx| {
                            let state = app.read(cx);
                            let worker = state.extensions.read(cx).worker.state.lock().unwrap();
                            (
                                state.comparison_views.len(),
                                state.editor.read(cx).value().len(),
                                state.status.clone(),
                                worker.status.as_ref().map(|status| status.message.clone()),
                                worker.editor_requests.len(),
                            )
                        });
                    // Report bounded state, never the full large editor source, so unsupported
                    // content, a runtime fault and a delayed native receipt remain distinguishable.
                    let history = manager.live.get("git").and_then(|instance| {
                        instance.views.get("history").map(|document| {
                            [
                                "history-error",
                                "history-status",
                                "history-loading",
                                "history-oid",
                            ]
                            .into_iter()
                            .filter_map(|id| {
                                document.active_node(id).map(|node| {
                                    let description = match &node.kind {
                                        ui::Kind::Text { text } => text.chars().take(512).collect(),
                                        _ => "non-text node present".to_owned(),
                                    };
                                    (id, description)
                                })
                            })
                            .collect::<Vec<_>>()
                        })
                    });
                    let logs = manager
                        .runtime_logs()
                        .records("git")
                        .into_iter()
                        .rev()
                        .take(8)
                        .map(|record| {
                            (
                                record.level,
                                record.source,
                                record.message.chars().take(1024).collect::<String>(),
                            )
                        })
                        .collect::<Vec<_>>();
                    panic!(
                        "supported large source did not appear: {file}; expected_bytes={SOURCE_BYTES}; actual_bytes={actual_bytes}; comparisons={comparisons}; host={host}; worker={worker:?}; pending={pending}; history={history:?}; diagnostics={:?}; logs={logs:?}; {peaks:?}",
                        manager.diagnostics("git"),
                    );
                }
            }
            assert!(visual.debug_bounds("document-diff-left").is_some());
            assert!(visual.debug_bounds("document-diff-right").is_some());
            peaks.capture(&manager);
            observations.start_capture();
        }
        observations.stop_capture();
        eprintln!("Git legal large-source native selection: {peaks:?}");
        manager.disable("git").unwrap();
        flush(&mut manager, &app, visual);
    });
}
