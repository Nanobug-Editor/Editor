//! Shared real-package GPUI setup. Tests retain their own scenarios and observable assertions.
use super::*;
use crate::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};
use std::cell::RefCell;

/// Shared receipt interaction is confined to these real-package worker scenarios.
#[cfg(windows)]
pub(super) mod receipts;

/// Keep the profile alive until the production actor and its managed tools have shut down.
pub(super) struct Fixture {
    pub app: Entity<EditorApp>,
    pub actor: Arc<Worker>,
    pub workspace: PathBuf,
    /// Host-owned test store permits public registry/private-data inspection without a guest handle.
    pub store: PathBuf,
    _profile: tempfile::TempDir,
}

/// Require explicit archive inputs; these tests never substitute source folders for installed code.
pub(super) fn input(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).expect(name))
}

/// Install granted real packages and start the same background actor used by the application.
/// The returned visual context belongs to the test window; `Fixture` owns the isolated profile.
pub(super) fn setup<'a>(
    cx: &'a mut TestAppContext,
    archives: &[&str],
) -> (Fixture, &'a mut VisualTestContext) {
    let workspace = input("NANOBUG_VUE_FIXTURES").canonicalize().unwrap();
    setup_for_workspace(cx, archives, workspace)
}

/// Use a caller-owned disposable project for package scenarios unrelated to Vue fixtures.
/// Its lifetime must cover the returned fixture, including orderly worker shutdown.
pub(super) fn setup_for_workspace<'a>(
    cx: &'a mut TestAppContext,
    archives: &[&str],
    workspace: PathBuf,
) -> (Fixture, &'a mut VisualTestContext) {
    let workspace = workspace.canonicalize().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let root = profile.path().join("runtime-plugins");
    let environment = Environment {
        workspace: workspace.display().to_string(),
        ..Default::default()
    };
    {
        let mut manager = Manager::open(root.clone(), environment.clone()).unwrap();
        for name in archives {
            let package = Package::read(&input(name)).unwrap();
            manager
                .install(&package, package.manifest.permissions.clone())
                .unwrap();
        }
    }
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::controls::init(cx);
        crate::bind_editor_shell_keys(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let session = SessionState::load_in_profile(&workspace, profile.path());
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let editor_workspace = Workspace::open(&workspace).unwrap();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| {
            EditorApp::new_with_session(editor_workspace, None, Some(session), window, cx)
        });
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    let actor = Arc::new(Worker::start_background(root.clone(), environment, true));
    visual.update(|_, cx| {
        app.read(cx)
            .extensions
            .clone()
            .update(cx, |panel, _| panel.worker = actor.clone())
    });
    visual.simulate_resize(size(px(1200.), px(800.)));
    (
        Fixture {
            app,
            actor,
            workspace,
            store: root,
            _profile: profile,
        },
        visual,
    )
}

/// Real actor clocks and deterministic UI clocks are distinct; drive publication and frames.
pub(super) fn wait(
    app: &Entity<EditorApp>,
    actor: &Worker,
    visual: &mut VisualTestContext,
    label: &str,
    seconds: u64,
    predicate: impl Fn(&EditorApp, &gpui::App) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        visual.executor().advance_clock(Duration::from_millis(20));
        visual.run_until_parked();
        let ready = visual.update(|window, cx| {
            app.read(cx)
                .extensions
                .clone()
                .update(cx, |panel, cx| panel.poll(cx));
            window.draw(cx).clear(cx);
            predicate(app.read(cx), cx)
        });
        if ready {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{label} failed: {}",
            status(app, actor, visual)
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Wait for managed teardown so a successful test cannot leave a worker running in the background.
pub(super) fn shutdown(fixture: &Fixture, visual: &mut VisualTestContext) {
    let (send, receive) = futures::channel::oneshot::channel();
    fixture.actor.tx.send(Work::Shutdown(Some(send))).unwrap();
    let receive = RefCell::new(receive);
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "managed shutdown",
        15,
        // Real actor completion must not directly wake a deterministic foreground task. Keep
        // checking the same authoritative acknowledgement and error within the original deadline.
        |_, _| match receive.borrow_mut().try_recv().unwrap() {
            Some(result) => {
                result.unwrap();
                true
            }
            None => false,
        },
    );
}

/// Exercise the native References action and inspect its real painted source-coordinate rows.
/// Expected UTF-16 positions come directly from the immutable original text, independently of
/// the host conversion helper. Foreign projects and generated file names cannot satisfy them.
pub(super) fn assert_native_references(
    app: &Entity<EditorApp>,
    actor: &Worker,
    workspace: &Path,
    path: &Path,
    symbol: &str,
    visual: &mut VisualTestContext,
) {
    let source = visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.open_file(path.to_owned(), window, cx);
            let source = app.editor.read(cx).text().to_string();
            let offset = source.rfind(symbol).expect("fixture symbol") + 1;
            app.editor.update(cx, |editor, cx| {
                editor.set_selected_range(offset..offset, cx);
                editor.focus(window, cx);
            });
            app.request_language_feature(
                plugin_runtime::plugin_protocol::direct::Feature::References,
                window,
                cx,
            );
            source
        })
    });
    let relative = path
        .strip_prefix(workspace)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let mut expected = source
        .match_indices(symbol)
        .map(|(offset, _)| {
            let prefix = &source[..offset];
            let row = prefix.bytes().filter(|byte| *byte == b'\n').count();
            let column = prefix.rsplit('\n').next().unwrap().encode_utf16().count();
            format!("{relative}:{}:{}", row + 1, column + 1)
        })
        .collect::<Vec<_>>();
    expected.sort();
    assert!(
        expected.len() >= 2,
        "the fixture must contain a declaration and a use"
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut observed = loop {
        visual.executor().advance_clock(Duration::from_millis(20));
        visual.run_until_parked();
        let rows = visual.update(|window, cx| {
            app.read(cx)
                .extensions
                .clone()
                .update(cx, |panel, cx| panel.poll(cx));
            window.draw(cx).clear(cx);
            gpui_base::test_support::snapshots(window)
                .iter()
                .filter(|node| node.visible() && node.role() == Some(gpui_kit::Role::Button))
                .filter_map(|node| node.label())
                .filter(|label| label.contains(".vue") && label.contains(':'))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        });
        if !rows.is_empty() {
            break rows;
        }
        assert!(
            Instant::now() < deadline,
            "native References produced no source rows for {}: {}",
            path.display(),
            status(app, actor, visual)
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    observed.sort();
    assert_eq!(
        observed, expected,
        "References must show each original source position once, without foreign or virtual files"
    );
    visual.simulate_keystrokes("escape");
}

/// Native diagnostics must retain one unresolved source expression on the correct project's tab.
/// This checks the composed visible model after syntax/LSP joining, including its Unicode range.
pub(super) fn assert_native_source_diagnostic(
    app: &Entity<EditorApp>,
    actor: &Worker,
    path: &Path,
    expected: &str,
    project_symbols: &[&str],
    visual: &mut VisualTestContext,
) {
    wait(
        app,
        actor,
        visual,
        "original-source semantic diagnostic",
        30,
        |app, cx| {
            app.tabs
                .iter()
                .find(|tab| tab.path() == path)
                .and_then(|tab| tab.text.as_ref())
                .and_then(|tab| tab.editor.read(cx).diagnostics())
                .is_some_and(|items| items.iter().any(|item| item.message.contains(expected)))
        },
    );
    visual.update(|_, cx| {
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
        let items = editor.diagnostics().unwrap();
        let matching = items
            .iter()
            .filter(|item| item.message.contains(expected))
            .collect::<Vec<_>>();
        assert_eq!(
            matching.len(),
            1,
            "semantic publishers must not duplicate this error"
        );
        assert_eq!(
            matching[0].severity,
            gpui_base::input::DiagnosticSeverity::Error
        );
        assert!(
            items.iter().all(|item| project_symbols
                .iter()
                .all(|other| { *other == expected || !item.message.contains(other) })),
            "another Vue project's diagnostic leaked into this source"
        );
        // Base's visible DiagnosticEntry.range is the final UTF-8 byte interval, whereas its
        // diagnostic.range retains line/column positions. Check painted source directly rather
        // than recomputing the implementation's UTF-16-to-native conversion in the expectation.
        let source = editor.text().to_string();
        let expected_start = source.find(expected).unwrap();
        assert_eq!(
            matching[0].range,
            expected_start..expected_start + expected.len()
        );
        let selected = source.get(matching[0].range.clone()).unwrap();
        assert_eq!(
            selected, expected,
            "diagnostic range must select original Unicode source text"
        );
    });
}

/// Failure evidence reports lifecycle and routing state, not plugin data or source contents.
fn status(app: &Entity<EditorApp>, actor: &Worker, visual: &mut VisualTestContext) -> String {
    let worker = {
        let state = actor.state.lock().unwrap();
        format!(
            "status={:?}; entries={:?}; startup={:?}; bindings={:?}",
            state.status.as_ref().map(|value| &value.message),
            state
                .entries
                .iter()
                .map(|item| (&item.manifest.id, item.enabled, &item.error))
                .collect::<Vec<_>>(),
            state.startup,
            state.language_document_bindings
        )
    };
    visual.update(|_, cx| {
        format!(
            "{worker}; native={:?}; status={}",
            app.read(cx).language_service_states,
            app.read(cx).status
        )
    })
}
