//! SDK project discovery routes visible unsaved sources to separate real native context transports.
use super::*;
use serde_json::{Value, json};

/// Two live project backends must stop using a discarded shared draft, then accept a new native
/// incarnation of the same path. Hover observes real stdio backend input, not transport counters.
#[gpui::test]
#[ignore = "set NANOBUG_PROJECT_CONTEXT_PACKAGE to a current SDK package and build native lsp_fixture in this profile"]
fn closing_shared_sources_removes_every_context_overlay_and_preserves_reopened_sources(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    for root in ["first", "second", "common"] {
        std::fs::create_dir(directory.path().join(root)).unwrap();
    }
    for root in ["first", "second"] {
        std::fs::write(
            directory.path().join(root).join("source.novel"),
            format!("{root} = 1\n"),
        )
        .unwrap();
    }
    let shared = directory.path().join("common/shared.ts");
    let saved = "shared = saved\n";
    std::fs::write(&shared, saved).unwrap();
    let shared = shared.canonicalize().unwrap();
    // Backend tracing is outside the watched workspace, so evidence writes cannot invalidate
    // the very related-source snapshots this scenario is trying to observe.
    let wire_directory = tempfile::tempdir().unwrap();
    let logs =
        ["first-wire.jsonl", "second-wire.jsonl"].map(|name| wire_directory.path().join(name));
    let package = shared_overlay_package(&shared, &logs);
    let paths = ["first/source.novel", "second/source.novel"]
        .map(|name| directory.path().join(name).canonicalize().unwrap());
    let (app, visual, mut manager) = shell(cx, directory.path(), vec![package]);
    open(&app, &shared, visual);
    visual.simulate_keystrokes("ctrl-a");
    visual.simulate_input("shared = discarded draft\n");
    for path in &paths {
        open(&app, path, visual);
    }
    await_context_hover_providers(&app, &mut manager, &paths, visual);
    assert_shared_overlay_hover(&app, &paths, "shared = discarded draft\n", visual);

    // The shared source has no primary provider. Both project backends learned its draft only
    // through the declared related-source graph, so close must reach both auxiliary consumers.
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.discard_tab(shared.clone(), window, cx);
            assert!(app.tabs.iter().all(|tab| tab.path() != shared));
        })
    });
    assert_shared_overlay_hover(&app, &paths, saved, visual);
    let shared_uri = language_navigation::file_uri(&shared).unwrap();
    for log in &logs {
        let frames = context_frames(log);
        assert_eq!(
            frames
                .iter()
                .filter(|frame| frame["method"] == "textDocument/didClose"
                    && frame["params"]["textDocument"]["uri"] == shared_uri.as_str())
                .count(),
            1,
            "each live project must release its old shared overlay"
        );
        assert_eq!(
            frames
                .iter()
                .filter(|frame| frame["method"] == "initialize")
                .count(),
            1,
            "closing an auxiliary source must not restart its still-open project"
        );
    }

    open(&app, &shared, visual);
    visual.simulate_keystrokes("ctrl-a");
    visual.simulate_input("shared = first reopened draft\n");
    assert_shared_overlay_hover(&app, &paths, "shared = first reopened draft\n", visual);
    // Close and reopen in one UI turn, before deferred close work is guaranteed to reach the pipe.
    // The old close may run after a new bind; it must compare the original lifetime before closing.
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.discard_tab(shared.clone(), window, cx);
            app.open_file(shared.clone(), window, cx);
        });
        window.draw(cx).clear(cx);
    });
    visual.simulate_keystrokes("ctrl-a");
    visual.simulate_input("shared = latest reopened draft\n");
    assert_shared_overlay_hover(&app, &paths, "shared = latest reopened draft\n", visual);
    pump(visual, 100);
    // Invalidate the target's positive hover cache after deferred closes have drained. A second
    // native analysis, rather than a cached reply, must still observe the latest shared incarnation.
    for path in &paths {
        open(&app, path, visual);
        visual.simulate_input("changed_");
    }
    assert_shared_overlay_hover(&app, &paths, "shared = latest reopened draft\n", visual);
    for log in &logs {
        let frames = context_frames(log);
        let lifecycle = frames
            .iter()
            .filter(|frame| frame["params"]["textDocument"]["uri"] == shared_uri.as_str())
            .filter_map(|frame| frame["method"].as_str())
            .filter(|method| matches!(*method, "textDocument/didOpen" | "textDocument/didClose"))
            .collect::<Vec<_>>();
        assert_eq!(
            lifecycle,
            [
                "textDocument/didOpen",
                "textDocument/didClose",
                "textDocument/didOpen",
                "textDocument/didClose",
                "textDocument/didOpen",
            ],
            "an old deferred close must not remove and reopen the latest source incarnation"
        );
    }
    assert_eq!(std::fs::read_to_string(&shared).unwrap(), saved);
    manager.shutdown();
}

/// Reuse the real SDK discovery guest; only its public service plan adds explicit related-source
/// membership and a backend fixture argument. No host branch knows these language or file names.
fn shared_overlay_package(shared: &Path, logs: &[PathBuf; 2]) -> Package {
    let fixture = PathBuf::from(
        std::env::var_os("NANOBUG_PROJECT_CONTEXT_PACKAGE")
            .expect("set the independently built project-context package"),
    );
    let mut files = Package::read(&fixture).unwrap().files;
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/lsp_fixture.exe")
        .canonicalize()
        .expect("build the matching native lsp_fixture first");
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["api"]["required"]["language.control"] = json!("^1");
    manifest["permissions"]
        .as_array_mut()
        .unwrap()
        .extend([json!("editor.read"), json!("workspace.read")]);
    manifest["language_servers"][0]["related_documents"] = json!("fixture/context/all-documents");
    manifest["language_servers"][0]["control_methods"] = json!({
        "fixture/context/all-documents": protocol::language_context::related_documents_method()
    });
    for (root, log) in ["first", "second"].into_iter().zip(logs) {
        manifest["services"][root] =
            json!({"program":executable,"args":[log,"--related-source",shared]});
    }
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    files.get_mut("plugin.toml").unwrap().extend_from_slice(b"\n[[language_definitions]]\nid='unfamiliar-project-language'\nname='Unfamiliar projects'\nextensions=['novel']\n");
    Package::from_files(files).unwrap()
}

/// Await actual source discovery and native Base bindings before observing the dependency backend.
fn await_context_hover_providers(
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    paths: &[PathBuf; 2],
    visual: &mut VisualTestContext,
) {
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    loop {
        crate::extensions::lsp_tests::publish(app, manager, visual);
        pump(visual, 250);
        let ready = visual.update(|_, cx| {
            paths.iter().all(|path| {
                app.read(cx)
                    .tabs
                    .iter()
                    .find(|tab| tab.path() == path)
                    .and_then(|tab| tab.text.as_ref())
                    .is_some_and(|tab| tab.editor.read(cx).lsp().hover_provider.is_some())
            })
        });
        if ready {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "project hover providers did not attach"
        );
    }
}

/// Both Base hover requests traverse native graph synchronization and the independent framed
/// backend. `expected` is a literal fixture preimage; it is not derived from host transport state.
fn assert_shared_overlay_hover(
    app: &Entity<EditorApp>,
    paths: &[PathBuf; 2],
    expected: &str,
    visual: &mut VisualTestContext,
) {
    let tasks = visual.update(|window, cx| {
        let captures = paths
            .iter()
            .map(|path| {
                let app = app.read(cx);
                let editor = app
                    .tabs
                    .iter()
                    .find(|tab| tab.path() == path)
                    .unwrap()
                    .text
                    .as_ref()
                    .unwrap()
                    .editor
                    .read(cx);
                (
                    editor.lsp().hover_provider.clone().unwrap(),
                    editor.text().clone(),
                )
            })
            .collect::<Vec<_>>();
        captures
            .into_iter()
            .map(|(provider, source)| provider.hover(&source, 1, window, cx))
            .collect::<Vec<_>>()
    });
    let results = Rc::new(RefCell::new(Vec::new()));
    for task in tasks {
        let results = results.clone();
        visual.update(|_, cx| {
            app.update(cx, |_, cx| {
                cx.spawn(async move |_, _| {
                    let result = task.await;
                    results.borrow_mut().push(result);
                })
                .detach();
            })
        });
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while results.borrow().len() != paths.len() {
        pump(visual, 10);
        assert!(
            std::time::Instant::now() < deadline,
            "shared-source hover did not return"
        );
    }
    for result in results.borrow_mut().drain(..) {
        let hover = result
            .expect("source-bound hover must succeed")
            .expect("fixture hover");
        let lsp_types::HoverContents::Markup(contents) = hover.contents else {
            panic!("fixture must return plaintext markup");
        };
        assert_eq!(
            contents.value, expected,
            "project analysis retained the wrong shared overlay"
        );
    }
}

/// Completed hover responses establish that every preceding fixture log record is fully written.
fn context_frames(log: &Path) -> Vec<Value> {
    std::fs::read_to_string(log)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// Two unfamiliar subprojects retain distinct processes, roots and unsaved input through native tabs.
#[gpui::test]
#[ignore = "set NANOBUG_PROJECT_CONTEXT_PACKAGE to a current public SDK package and build native lsp_fixture in this profile"]
fn installed_document_contexts_route_native_tabs_without_cross_project_text(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    for root in ["first", "second"] {
        std::fs::create_dir(directory.path().join(root)).unwrap();
        std::fs::write(
            directory.path().join(root).join("source.novel"),
            format!("{root} = 1\n"),
        )
        .unwrap();
    }
    let fixture = PathBuf::from(
        std::env::var_os("NANOBUG_PROJECT_CONTEXT_PACKAGE")
            .expect("set the independently built project-context package"),
    );
    let mut files = Package::read(&fixture)
        .expect("build the real SDK fixture first")
        .files;
    // The running test binary fixes the source snapshot, Cargo target and profile of this fixture.
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/lsp_fixture.exe")
        .canonicalize()
        .expect("build lsp_fixture first");
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    let logs = [
        directory.path().join("first-wire.jsonl"),
        directory.path().join("second-wire.jsonl"),
    ];
    for (root, log) in ["first", "second"].into_iter().zip(&logs) {
        manifest["services"][root] = json!({"program":executable,"args":[log]});
    }
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    files.get_mut("plugin.toml").unwrap().extend_from_slice(b"\n[[language_definitions]]\nid='unfamiliar-project-language'\nname='Unfamiliar projects'\nextensions=['novel']\n");
    let package = Package::from_files(files).unwrap();
    let first = directory
        .path()
        .join("first/source.novel")
        .canonicalize()
        .unwrap();
    let second = directory
        .path()
        .join("second/source.novel")
        .canonicalize()
        .unwrap();
    let (app, visual, mut manager) = shell(cx, directory.path(), vec![package]);
    open(&app, &first, visual);
    visual.simulate_input("unsaved");
    let source = text(&app, visual);
    assert!(source.contains("unsaved"));
    open(&app, &second, visual);
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    loop {
        crate::extensions::lsp_tests::publish(&app, &mut manager, visual);
        // A publication intentionally invalidates syntax tasks. Let the production 180 ms source
        // debounce finish before publishing again, so this harness does not starve every didOpen.
        pump(visual, 250);
        let ready = visual.update(|_, cx| {
            app.read(cx)
                .tabs
                .iter()
                .filter(|tab| tab.path() == first || tab.path() == second)
                .all(|tab| {
                    tab.text
                        .as_ref()
                        .unwrap()
                        .editor
                        .read(cx)
                        .lsp()
                        .definition_provider
                        .is_some()
                })
        });
        if ready {
            break;
        }
        if std::time::Instant::now() >= deadline {
            let status = visual.update(|_, cx| {
                let app = app.read(cx);
                format!(
                    "status={} states={:?} languages={:?}",
                    app.status,
                    app.language_service_states,
                    app.tabs
                        .iter()
                        .map(|tab| (tab.path(), app.language_for_path(tab.path())))
                        .collect::<Vec<_>>()
                )
            });
            panic!(
                "both original contexts must attach: {status}; bindings={:?}; wires={:?}",
                manager.language_document_bindings(),
                logs.iter()
                    .map(|log| std::fs::read_to_string(log).unwrap_or_default())
                    .collect::<Vec<_>>()
            );
        }
    }
    // Exercise Base's public navigation provider exactly as a native definition gesture does.
    // Merely attaching providers may keep synchronization lazy until the source is consumed.
    let navigation = visual.update(|window, cx| {
        let captures = app
            .read(cx)
            .tabs
            .iter()
            .filter(|tab| tab.path() == first || tab.path() == second)
            .map(|tab| {
                let editor = tab.text.as_ref().unwrap().editor.read(cx);
                (
                    editor.lsp().definition_provider.clone().unwrap(),
                    editor.text().clone(),
                )
            })
            .collect::<Vec<_>>();
        captures
            .into_iter()
            .map(|(provider, text)| provider.definitions(&text, 0, window, cx))
            .collect::<Vec<_>>()
    });
    let completed = Rc::new(RefCell::new(0));
    for navigation in navigation {
        let completed = completed.clone();
        visual.update(|_, cx| {
            app.update(cx, |_, cx| {
                cx.spawn(async move |_, _| {
                    navigation.await.unwrap();
                    *completed.borrow_mut() += 1;
                })
                .detach()
            })
        });
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while *completed.borrow() != 2 {
        pump(visual, 10);
        assert!(
            std::time::Instant::now() < deadline,
            "both native definition requests must finish"
        );
    }
    for (index, root) in ["first", "second"].into_iter().enumerate() {
        let frames = std::fs::read_to_string(&logs[index])
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            frames
                .iter()
                .filter(|frame| frame["method"] == "initialize")
                .count(),
            1
        );
        let initialize = frames
            .iter()
            .find(|frame| frame["method"] == "initialize")
            .unwrap();
        assert!(
            initialize["params"]["rootUri"]
                .as_str()
                .unwrap()
                .ends_with(root)
        );
        let opens = frames
            .iter()
            .filter(|frame| frame["method"] == "textDocument/didOpen")
            .collect::<Vec<_>>();
        assert_eq!(
            opens.len(),
            1,
            "one root must never synchronize another project's open source"
        );
        assert_eq!(
            opens[0]["params"]["textDocument"]["languageId"],
            "unfamiliar-project-language"
        );
        assert_eq!(
            opens[0]["params"]["textDocument"]["text"],
            if index == 0 {
                source.clone()
            } else {
                "second = 1\n".into()
            }
        );
    }
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "first = 1\n");
    manager.disable("project-context-fixture").unwrap();
    crate::extensions::lsp_tests::publish(&app, &mut manager, visual);
}
