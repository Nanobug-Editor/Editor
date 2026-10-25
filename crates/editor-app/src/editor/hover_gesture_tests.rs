//! Real dock gestures must never admit editor details through the moving source viewport.

use super::*;
use gpui_base::input::{Diagnostic, Position};
use gpui_kit::VisualTestContext;

/// Open an isolated source and visible sidebar through the production shell and dock renderer.
fn setup(
    cx: &mut TestAppContext,
) -> (tempfile::TempDir, Entity<EditorApp>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        theme::apply_theme(theme::builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("hover.txt");
    std::fs::write(&path, "alpha beta\nsecond_word\n").unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let mut session =
        SessionState::load_in_profile(workspace.root(), &directory.path().join("profile"));
    session.explorer_visible = true;
    session.outline_visible = false;
    session.messages_visible = false;
    session.problems_visible = false;
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            EditorApp::new_with_session(workspace, Some(path), Some(session), window, cx)
        });
        *capture.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = slot.borrow_mut().take().unwrap();
    visual.update(|window, _| window.activate_window());
    visual.simulate_resize(size(px(1000.), px(600.)));
    draw(visual);
    (directory, view, visual)
}

/// Flush actual native layout before examining visible cards or the next drag position.
fn draw(visual: &mut VisualTestContext) {
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
}

/// A sidebar press followed by movement over a diagnostic must resize without showing details.
#[gpui::test]
fn hover_dock_resize_does_not_show_diagnostic(cx: &mut TestAppContext) {
    let (_directory, view, visual) = setup(cx);
    visual.update(|_, cx| {
        let editor = view.read(cx).editor.clone();
        editor.update(cx, |state, cx| {
            let text = state.text().clone();
            let diagnostics = state.diagnostics_mut().unwrap();
            diagnostics.reset(&text);
            diagnostics.push(
                Diagnostic::new(Position::new(0, 0)..Position::new(0, 5), "error on alpha")
                    .with_severity(gpui_base::input::DiagnosticSeverity::Error),
            );
            cx.notify();
        });
    });
    draw(visual);
    let source = visual.debug_bounds("editor-panel-content").unwrap();
    let symbol = visual.update(|_, cx| {
        view.read(cx)
            .editor
            .read(cx)
            .range_to_bounds(&(0..5))
            .unwrap()
            .center()
    });
    let handle = point(source.left() - px(1.), symbol.y);
    // An idle resize cursor is outside the text hit target, even on an erroneous row.
    visual.simulate_mouse_move(handle, None, Default::default());
    visual.executor().advance_clock(Duration::from_secs(2));
    draw(visual);
    assert!(visual.debug_bounds("editor-diagnostic-card").is_none());
    visual.simulate_mouse_down(handle, MouseButton::Left, Default::default());
    visual.simulate_mouse_move(
        handle + point(px(12.), px(0.)),
        MouseButton::Left,
        Default::default(),
    );
    visual.simulate_mouse_move(symbol, MouseButton::Left, Default::default());
    draw(visual);
    assert!(
        visual.debug_bounds("editor-panel-content").unwrap().left() > source.left() + px(5.),
        "the real sidebar must resize"
    );
    assert!(
        visual.debug_bounds("editor-diagnostic-card").is_none(),
        "dragging the sidebar over a diagnostic must not display an error card"
    );
    visual.executor().advance_clock(Duration::from_secs(2));
    draw(visual);
    assert!(visual.debug_bounds("editor-definition-card").is_none());
    assert!(visual.debug_bounds("editor-diagnostic-card").is_none());
    visual.simulate_mouse_up(symbol, MouseButton::Left, Default::default());
    draw(visual);
    let (blank, current) = visual.update(|_, cx| {
        let editor = view.read(cx).editor.read(cx);
        (
            editor.input_bounds().bottom_right() - point(px(20.), px(20.)),
            editor.range_to_bounds(&(0..5)).unwrap().center(),
        )
    });
    visual.simulate_mouse_move(blank, None, Default::default());
    visual.simulate_mouse_move(current, None, Default::default());
    draw(visual);
    assert!(
        visual.debug_bounds("editor-diagnostic-card").is_some(),
        "a fresh hover after releasing the splitter must still work"
    );
}

/// A pending response must not open after a press on the splitter, even before its first move.
#[gpui::test]
fn hover_splitter_press_retires_pending_details(cx: &mut TestAppContext) {
    let (_directory, view, visual) = setup(cx);
    let replies = Rc::new(RefCell::new(Vec::new()));
    let symbol = visual.update(|_, cx| {
        let editor = view.read(cx).editor.clone();
        editor.update(cx, |state, _| {
            state.lsp_mut().hover_provider =
                Some(Rc::new(super::keyboard::HeldHover(replies.clone())));
        });
        editor.read(cx).range_to_bounds(&(0..5)).unwrap().center()
    });
    visual.simulate_mouse_move(symbol, None, Default::default());
    visual.run_until_parked();
    assert!(
        !replies.borrow().is_empty(),
        "the real pointer path must request details"
    );
    let source = visual.debug_bounds("editor-panel-content").unwrap();
    let handle = point(source.left() - px(1.), symbol.y);
    visual.simulate_mouse_down(handle, MouseButton::Left, Default::default());
    for reply in replies.borrow_mut().drain(..) {
        let _ = reply.send(Hover {
            contents: HoverContents::Scalar(MarkedString::String("late details".into())),
            range: None,
        });
    }
    visual.executor().advance_clock(Duration::from_secs(2));
    draw(visual);
    assert!(
        visual.debug_bounds("editor-definition-card").is_none(),
        "a reply from before the splitter press must never become visible"
    );
    visual.simulate_mouse_up(handle, MouseButton::Left, Default::default());
}
