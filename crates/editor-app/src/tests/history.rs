//! Actual host history save/recovery behavior in an isolated user profile.

use crate::*;
use gpui_kit::EntityInputHandler as _;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};

#[path = "history/capture.rs"]
mod capture;
#[path = "history/late_results.rs"]
mod late_results;
#[path = "history/observations.rs"]
mod observations;
#[path = "history/queueing.rs"]
mod queueing;
#[path = "history/recovery.rs"]
mod recovery;

/// Inspect completed native glyph projections without exposing history internals in production.
pub(crate) fn comparison_painted_ranges(
    app: &EditorApp,
    ranges: [std::ops::Range<usize>; 2],
    cx: &App,
) -> Option<[Bounds<Pixels>; 2]> {
    use editor_ui::aligned_editor::AlignmentStatus;
    let alignment = &app.history.panel.as_ref()?.comparison.as_ref()?.alignment;
    if alignment.status(cx) != AlignmentStatus::Ready {
        return None;
    }
    Some([
        alignment.project_range(0, ranges[0].clone(), cx)?,
        alignment.project_range(1, ranges[1].clone(), cx)?,
    ])
}

/// Isolate real history and session paths rather than swapping in a mock history implementation.
pub(crate) fn with_history_editor(
    cx: &mut TestAppContext,
    unavailable: bool,
    scenario: impl FnOnce(&mut VisualTestContext, Entity<EditorApp>, &Path),
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
        app::bind_editor_shell_keys(cx);
    });
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project");
    let profile = directory.path().join("profile");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(&profile).unwrap();
    if unavailable {
        std::fs::write(profile.join("history"), "not a directory").unwrap();
    }
    let path = project.join("note.txt");
    std::fs::write(&path, "original on disk").unwrap();
    let workspace = Workspace::open(&project).unwrap();
    let session = SessionState::load_in_profile(workspace.root(), &profile);
    let document = path.clone();
    let mut application = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let app = cx.new(|cx| {
            EditorApp::new_with_session(workspace, Some(document), Some(session), window, cx)
        });
        application = Some(app.clone());
        Root::new(app, window, cx)
    });
    visual.simulate_resize(size(px(1100.), px(850.)));
    let app = application.unwrap();
    visual.update(|window, cx| {
        app.read(cx)
            .editor
            .read(cx)
            .focus_handle(cx)
            .focus(window, cx);
        window.draw(cx).clear(cx);
    });
    scenario(visual, app, &path);
}

/// Native threads may finish between deterministic GPUI turns; poll only the observable predicate.
#[track_caller]
fn wait_until(
    visual: &mut VisualTestContext,
    mut ready: impl FnMut(&mut VisualTestContext) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        visual.run_until_parked();
        visual.update(|window, cx| window.draw(cx).clear(cx));
        if ready(visual) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "history host operation did not finish"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Wait for actual metadata/preview acknowledgements instead of assuming storage finishes in a UI turn.
fn open_first_preview(visual: &mut VisualTestContext, app: &Entity<EditorApp>, path: &Path) {
    visual.run_until_parked();
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.open_file_history(path.to_owned(), window, cx)
        })
    });
    wait_until(visual, |visual| {
        visual.update(|_, cx| {
            app.read(cx)
                .history
                .panel
                .as_ref()
                .is_some_and(|panel| !panel.busy && !panel.rows.is_empty())
        })
    });
    select_preview(visual, app, 0);
}

/// Bind the native selection to its committed version before requesting an action.
fn select_preview(visual: &mut VisualTestContext, app: &Entity<EditorApp>, index: usize) {
    visual
        .update(|window, cx| app.update(cx, |app, cx| app.select_history_item(index, window, cx)));
    wait_until(visual, |visual| {
        visual.update(|_, cx| {
            let panel = app.read(cx).history.panel.as_ref().unwrap();
            assert!(panel.error.is_none(), "preview failed: {:?}", panel.error);
            panel.preview.is_some()
        })
    });
}

/// Share the real profile connection only after native work has reached an observable checkpoint.
fn history_store(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
) -> editor_history::History {
    let root = visual.update(|_, cx| app.read(cx).history.paths.as_ref().unwrap().root.clone());
    editor_history::History::open(root, editor_history::Policy::default()).unwrap()
}
