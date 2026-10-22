//! Shared real-package GPUI setup. Tests retain their own scenarios and observable assertions.
use super::*;
use crate::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};
use std::cell::RefCell;

/// Keep the profile alive until the production actor and its managed tools have shut down.
pub(super) struct Fixture {
    pub app: Entity<EditorApp>,
    pub actor: Arc<Worker>,
    pub workspace: PathBuf,
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
    let actor = Arc::new(Worker::start_background(root, environment, true));
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
    let done = Rc::new(Cell::new(false));
    let complete = done.clone();
    visual.update(|_, cx| {
        fixture.app.update(cx, |_, cx| {
            cx.spawn(async move |_, _| {
                receive.await.unwrap().unwrap();
                complete.set(true);
            })
            .detach()
        })
    });
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "managed shutdown",
        15,
        |_, _| done.get(),
    );
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
