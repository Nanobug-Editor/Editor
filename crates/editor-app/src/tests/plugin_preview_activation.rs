//! Cold file opening crosses the real package, actor and native preview publication boundaries.
use super::*;
use crate::*;
use gpui_kit::{ClipboardItem, TestAppContext, VisualTestContext, gpui};
use plugin_runtime::plugin_protocol as protocol;
use std::cell::RefCell;

/// Isolated package storage and the real actor share the production native document window.
struct ColdPreview {
    directory: tempfile::TempDir,
    app: Entity<EditorApp>,
    worker: Arc<Worker>,
}

/// Install the exact reviewed ZIP and reopen cold; no test code executes a view demand.
fn mount(cx: &mut TestAppContext) -> (ColdPreview, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("notes.md");
    std::fs::write(&path, "# Cold preview\n").unwrap();
    let root = directory.path().join("runtime");
    let environment = protocol::Environment {
        workspace: directory.path().display().to_string(),
        ..Default::default()
    };
    let package = crate::test_packages::package("markdown-0.17.2");
    let mut manager = plugin_runtime::Manager::open(root.clone(), environment.clone()).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    manager.shutdown();
    drop(manager);
    let worker = Arc::new(Worker::start_background(root, environment, true));
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while !worker.state.lock().unwrap().ready {
        assert!(
            std::time::Instant::now() < deadline,
            "actor startup timed out"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(worker.state.lock().unwrap().instance_ids.is_empty());
    let workspace = Workspace::open(directory.path()).unwrap();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, ui) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    ui.update(|window, cx| {
        window.activate_window();
        app.read(cx).extensions.clone().update(cx, |panel, cx| {
            panel.worker = worker.clone();
            panel.poll(cx);
        });
        app.update(cx, |app, cx| {
            app.sync_plugin_panels(window, cx);
        });
    });
    (
        ColdPreview {
            directory,
            app,
            worker,
        },
        ui,
    )
}

impl ColdPreview {
    /// Open through native navigation so both source notifications originate in production code.
    fn open(&self, ui: &mut VisualTestContext) {
        let path = self.directory.path().join("notes.md");
        ui.update(|window, cx| {
            self.app
                .update(cx, |app, cx| app.open_file(path, window, cx))
        });
    }

    /// Draw actual fallback/preview bodies and poll real publications while the background actor prepares.
    fn frame(&self, ui: &mut VisualTestContext) {
        ui.executor().advance_clock(Duration::from_millis(160));
        ui.run_until_parked();
        ui.update(|window, cx| {
            let owner = self.app.read(cx).extensions.clone();
            owner.update(cx, |panel, cx| panel.poll(cx));
            let panels = self
                .app
                .read(cx)
                .plugin_panels
                .values()
                .cloned()
                .collect::<Vec<_>>();
            for panel in panels {
                panel.update(cx, |panel, cx| panel.poll(cx));
            }
            self.app.update(cx, |app, cx| {
                app.sync_plugin_panels(window, cx);
                app.sync_editor_previews(cx);
            });
            window.draw(cx).clear(cx);
        });
        ui.run_until_parked();
    }

    /// Checked retirement is an actor barrier for all native notifications already in its queue.
    fn shutdown(&self) {
        let (reply, receipt) = futures::channel::oneshot::channel();
        self.worker.tx.send(Work::Shutdown(Some(reply))).unwrap();
        futures::executor::block_on(receipt).unwrap().unwrap();
    }
}

/// A globally enabled cold provider is not a disabled plugin while its first file is being selected.
#[gpui::test]
#[ignore = "opt-in frozen Markdown Package/WASM; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn cold_markdown_open_does_not_report_disabled_file_and_preview(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx);
    fixture.open(ui);
    // Retirement is an actor barrier for the actual notifications queued by native file opening.
    // Do not paint or manually demand the view to hide an early source callback behind a warm guest.
    fixture.shutdown();
    let (enabled, errors) = {
        let state = fixture.worker.state.lock().unwrap();
        (
            state
                .entries
                .iter()
                .any(|entry| entry.manifest.id == "markdown" && entry.enabled),
            state
                .logs
                .records("markdown")
                .iter()
                .filter(|record| record.level == plugin_runtime::logs::LogLevel::Error)
                .map(|record| record.message.clone())
                .collect::<Vec<_>>(),
        )
    };
    assert!(enabled, "cold selection must preserve global enablement");
    assert!(
        errors.is_empty(),
        "enabled cold preview reported plugin errors: {errors:?}"
    );
}

/// Cold source validation still rejects malformed text and undeclared surfaces before owner admission.
#[gpui::test]
#[ignore = "opt-in frozen Markdown Package/WASM; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn cold_preview_validation_errors_remain_visible(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx);
    fixture.open(ui);
    let epoch = fixture
        .worker
        .state
        .lock()
        .unwrap()
        .instance_epochs
        .get("markdown")
        .copied()
        .unwrap_or(0);
    for (panel, notification) in [
        (
            "preview",
            protocol::api::Notification::Preview {
                document: None,
                text: "unbound text".into(),
            },
        ),
        (
            "undeclared",
            protocol::api::Notification::FilePreview { file: None },
        ),
    ] {
        fixture
            .worker
            .tx
            .send(Work::Event(
                "markdown".into(),
                epoch,
                Some(panel.into()),
                notification,
            ))
            .unwrap();
    }
    fixture.shutdown();
    let errors = fixture
        .worker
        .state
        .lock()
        .unwrap()
        .logs
        .records("markdown")
        .into_iter()
        .filter(|record| record.level == plugin_runtime::logs::LogLevel::Error)
        .map(|record| record.message)
        .collect::<Vec<_>>();
    assert_eq!(
        errors.len(),
        2,
        "unexpected cold-source diagnostics: {errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("LimitExceeded: Invalid or oversized preview document"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("PermissionDenied: File preview requires"))
    );
}

/// First birth resubmits the latest unsaved snapshot, rather than losing the early callback or replaying old text.
#[gpui::test]
#[ignore = "opt-in frozen Markdown Package/WASM; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn cold_markdown_first_birth_renders_latest_unsaved_source(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx);
    fixture.open(ui);
    ui.simulate_resize(gpui_kit::size(px(1400.), px(900.)));
    fixture.frame(ui);
    ui.update(|window, cx| {
        fixture
            .app
            .read(cx)
            .editor
            .clone()
            .update(cx, |editor, cx| editor.focus(window, cx));
        cx.write_to_clipboard(ClipboardItem::new_string("# Latest 中文\n".into()));
    });
    ui.simulate_keystrokes("ctrl-a ctrl-v");
    ui.run_until_parked();
    let expected = ui.update(|_, cx| {
        let app = fixture.app.read(cx);
        assert_eq!(app.editor.read(cx).text().to_string(), "# Latest 中文\n");
        app.plugin_document_version(app.active_tab_index().unwrap())
            .unwrap()
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(90);
    loop {
        fixture.frame(ui);
        let available = ui.update(|_, cx| fixture.app.read(cx).active_editor_preview(cx).is_some());
        let bound = fixture
            .worker
            .state
            .lock()
            .unwrap()
            .views
            .get("markdown/preview")
            .is_some_and(|scene| {
                scene.source.as_ref() == Some(&expected)
                    && serde_json::to_string(scene.as_ref())
                        .unwrap()
                        .contains("Latest 中文")
            });
        if bound && ui.debug_bounds("plugin-ui-preview-root").is_some() {
            break;
        }
        if !available || std::time::Instant::now() >= deadline {
            let entry_errors = {
                let state = fixture.worker.state.lock().unwrap();
                state
                    .entries
                    .iter()
                    .map(|entry| entry.error.clone())
                    .collect::<Vec<_>>()
            };
            panic!(
                "latest source did not reach the born preview: available={available}, errors={entry_errors:?}"
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    fixture.shutdown();
    let errors = fixture
        .worker
        .state
        .lock()
        .unwrap()
        .logs
        .records("markdown")
        .into_iter()
        .filter(|record| record.level == plugin_runtime::logs::LogLevel::Error)
        .map(|record| record.message)
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "native cold-open errors: {errors:?}");
    assert_eq!(
        std::fs::read_to_string(fixture.directory.path().join("notes.md")).unwrap(),
        "# Cold preview\n"
    );
}
