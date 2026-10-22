//! SDK project discovery routes visible unsaved sources to separate real native context transports.
use super::*;
use serde_json::{Value, json};

/// Two unfamiliar subprojects retain distinct processes, roots and unsaved input through native tabs.
#[gpui::test]
#[ignore = "build project-context with host --plugin-package --debug and native lsp_fixture first"]
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
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../plugin-runtime/tests/fixtures/project-context/project-context-fixture-0.1.0.zip");
    let mut files = Package::read(&fixture)
        .expect("build the real SDK fixture first")
        .files;
    let executable = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/examples/lsp_fixture.exe")
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
