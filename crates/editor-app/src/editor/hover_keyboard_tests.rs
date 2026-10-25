//! Details close on edits and navigation; explicit keyboard requests can reopen a dismissed symbol.

use super::*;
use gpui_kit::VisualTestContext;

/// Hold the actual provider task until the test releases a reply after native key dispatch.
/// Unlike a timer, this proves Escape happened while the response was still unavailable.
pub(super) struct HeldHover(pub(super) Rc<RefCell<Vec<futures::channel::oneshot::Sender<Hover>>>>);

impl gpui_base::input::HoverProvider for HeldHover {
    fn hover(
        &self,
        _: &gpui_base::input::Rope,
        _: usize,
        _: &mut Window,
        cx: &mut App,
    ) -> gpui_kit::Task<anyhow::Result<Option<Hover>>> {
        let (reply, response) = futures::channel::oneshot::channel();
        self.0.borrow_mut().push(reply);
        cx.spawn(async move |_| Ok(Some(response.await?)))
    }
}

/// Leave the suppressed symbol and hover it again through the real pointer path.
fn show_details(cx: &mut VisualTestContext, view: &Entity<EditorApp>, position: Point<Pixels>) {
    let blank = cx.update(|_, cx| {
        let bounds = view.read(cx).editor.read(cx).input_bounds();
        point(bounds.right() - px(8.), position.y)
    });
    cx.simulate_mouse_move(blank, None::<MouseButton>, Default::default());
    cx.simulate_mouse_move(position, None::<MouseButton>, Default::default());
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("editor-definition-card").is_some());
}

/// Exercise key routing and edit events while the pointer stays on one symbol.
#[gpui::test]
fn hover_closes_on_keyboard_navigation_and_edits(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        theme::apply_theme(theme::builtin_theme(false), cx);
        cx.set_reduce_motion(true);
        // Production installs this shell shortcut during application startup.
        cx.bind_keys([KeyBinding::new(
            "ctrl-i",
            ShowDefinitionDetails,
            Some("EditorShell && !PluginSurface"),
        )]);
    });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("keyboard.rs");
    std::fs::write(&path, "alpha beta\nsecond_word\n").unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| EditorApp::new(workspace, Some(path), window, cx));
        *capture.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = slot.borrow_mut().take().unwrap();
    cx.update(|window, _| window.activate_window());
    cx.simulate_resize(size(px(1000.), px(600.)));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let requests = Rc::new(Cell::new(0));
    let position = cx.update(|window, cx| {
        let editor = view.read(cx).editor.clone();
        editor.read(cx).focus_handle(cx).focus(window, cx);
        editor.update(cx, |state, cx| {
            state.set_cursor_position(lsp_types::Position::new(0, 0), window, cx);
            state.lsp_mut().hover_provider = Some(Rc::new(ReadyHover(requests.clone())));
        });
        editor.read(cx).range_to_bounds(&(0..5)).unwrap().center()
    });
    // Pointer details must remain hidden until the hover delay expires.
    cx.simulate_mouse_move(position, None::<MouseButton>, Default::default());
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(999));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("editor-definition-card").is_none());
    // The shortcut bypasses that pending pointer timer without advancing time.
    cx.simulate_keystrokes("ctrl-i");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("editor-definition-card").is_some());
    let before = cx.update(|_, cx| view.read(cx).editor.read(cx).cursor());
    cx.simulate_keystrokes("right");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_ne!(
        cx.update(|_, cx| view.read(cx).editor.read(cx).cursor()),
        before
    );
    assert!(
        cx.debug_bounds("editor-definition-card").is_none(),
        "moving the caret must dismiss details"
    );
    let before_requests = requests.get();
    cx.simulate_mouse_move(
        position + point(px(1.), px(0.)),
        None::<MouseButton>,
        Default::default(),
    );
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        cx.debug_bounds("editor-definition-card").is_none(),
        "cached details must not reopen on the same symbol"
    );
    assert_eq!(requests.get(), before_requests);

    show_details(cx, &view, position);
    // Copy does not move the caret or edit the document, so it retains details.
    cx.simulate_keystrokes("ctrl-c");
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("editor-definition-card").is_some());
    cx.simulate_input("x");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        cx.debug_bounds("editor-definition-card").is_none(),
        "typing must dismiss details"
    );
    cx.update(|_, cx| {
        let app = view.read(cx);
        assert!(app.editor.read(cx).text().to_string().starts_with("axlpha"));
        assert!(app.pointer_hover_cached.is_none());
        assert!(!app.pointer_hover_pending);
    });
    // Forward deletion edits text without moving the caret. It must be
    // covered by Change events, independently of the keyboard cursor check.
    show_details(cx, &view, position);
    let cursor = cx.update(|_, cx| view.read(cx).editor.read(cx).cursor());
    cx.simulate_keystrokes("delete");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(
        cx.update(|_, cx| view.read(cx).editor.read(cx).cursor()),
        cursor
    );
    assert!(cx.debug_bounds("editor-definition-card").is_none());
    cx.update(|_, cx| {
        assert!(
            view.read(cx)
                .editor
                .read(cx)
                .text()
                .to_string()
                .starts_with("axpha")
        );
    });
}

/// Escape suppresses automatic pointer reopening, but a later explicit request must remain visible
/// through real same-symbol mouse movement. The test never changes the app's suppression state.
#[gpui::test]
fn explicit_keyboard_hover_reopens_a_suppressed_symbol_and_survives_pointer_motion(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        theme::apply_theme(theme::builtin_theme(false), cx);
        cx.set_reduce_motion(true);
        // Use the production shell binding, so Ctrl+I reaches EditorApp's ordinary action handler.
        crate::bind_editor_shell_keys(cx);
    });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("reopen.rs");
    let source = "alpha beta\n";
    std::fs::write(&path, source).unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| EditorApp::new(workspace, Some(path), window, cx));
        *capture.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = slot.borrow_mut().take().unwrap();
    cx.update(|window, _| window.activate_window());
    cx.simulate_resize(size(px(1000.), px(600.)));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let requests = Rc::new(Cell::new(0));
    let position = cx.update(|window, cx| {
        let editor = view.read(cx).editor.clone();
        editor.update(cx, |state, cx| {
            state.lsp_mut().hover_provider = Some(Rc::new(ReadyHover(requests.clone())));
            state.set_selected_range(2..2, cx);
            state.focus(window, cx);
        });
        editor.read(cx).range_to_bounds(&(0..5)).unwrap().center()
    });
    // This first card must actually be visible before Escape; warming only a provider task does
    // not establish the native dismissal range that caused the desktop regression.
    show_details(cx, &view, position);
    assert!(requests.get() > 0);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("editor-definition-card").is_none());
    let dismissed_requests = requests.get();
    cx.simulate_mouse_move(
        position + point(px(1.), px(0.)),
        None::<MouseButton>,
        Default::default(),
    );
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        cx.debug_bounds("editor-definition-card").is_none(),
        "Escape must still suppress automatic same-symbol pointer reopening"
    );
    assert_eq!(
        requests.get(),
        dismissed_requests,
        "a suppressed pointer must not request details again without explicit input"
    );
    // Keep the same caret and focus after Escape. No direct handler invocation or refocus can
    // conceal a broken native keyboard route here.
    cx.simulate_keystrokes("ctrl-i");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        requests.get() > dismissed_requests,
        "the explicit shell action must reach the hover provider"
    );
    assert!(
        cx.debug_bounds("editor-definition-card").is_some(),
        "explicit Ctrl+I must reopen the same dismissed symbol"
    );
    cx.simulate_mouse_move(
        position + point(px(2.), px(0.)),
        None::<MouseButton>,
        Default::default(),
    );
    // Assert before another async response or pointer delay can hide a one-frame dismissal.
    cx.update(|_, cx| {
        assert!(
            view.read(cx).editor.read(cx).hover_popover().is_some(),
            "same-symbol pointer motion must retain explicitly requested details"
        );
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        cx.debug_bounds("editor-definition-card").is_some(),
        "explicit details must remain painted after same-symbol mouse movement"
    );
    cx.update(|_, cx| {
        let app = view.read(cx);
        assert_eq!(app.editor.read(cx).cursor(), 2);
        assert_eq!(app.editor.read(cx).text().to_string(), source);
    });
}

/// Escape also cancels an explicit request whose card has not appeared yet; a late reply must
/// not revive it, while a subsequent ordinary Ctrl+I remains usable for the same caret.
#[gpui::test]
fn escape_cancels_pending_explicit_hover_without_reviving_its_late_reply(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        theme::apply_theme(theme::builtin_theme(false), cx);
        cx.set_reduce_motion(true);
        crate::bind_editor_shell_keys(cx);
    });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("pending.rs");
    std::fs::write(&path, "alpha beta\n").unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| EditorApp::new(workspace, Some(path), window, cx));
        *capture.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = slot.borrow_mut().take().unwrap();
    cx.update(|window, _| window.activate_window());
    cx.simulate_resize(size(px(1000.), px(600.)));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let replies = Rc::new(RefCell::new(Vec::new()));
    cx.update(|window, cx| {
        let editor = view.read(cx).editor.clone();
        editor.update(cx, |state, cx| {
            state.lsp_mut().hover_provider = Some(Rc::new(HeldHover(replies.clone())));
            state.set_selected_range(2..2, cx);
            state.focus(window, cx);
        });
    });
    cx.simulate_keystrokes("ctrl-i");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(
        replies.borrow().len(),
        1,
        "the actual shell action must have reached a still-pending provider"
    );
    assert!(cx.debug_bounds("editor-definition-card").is_none());
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    let old_reply = replies.borrow_mut().pop().unwrap();
    // Dropping a canceled waiter is also legitimate. If it still accepts a reply, presentation
    // must reject that result by its original request lifetime rather than its unchanged text.
    let _ = old_reply.send(Hover {
        contents: HoverContents::Scalar(MarkedString::String("stale details".into())),
        range: None,
    });
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        cx.debug_bounds("editor-definition-card").is_none(),
        "Escape while hover is pending must reject its late response"
    );
    cx.simulate_keystrokes("ctrl-i");
    cx.run_until_parked();
    assert_eq!(
        replies.borrow().len(),
        1,
        "a fresh explicit request must remain usable after cancellation"
    );
    replies
        .borrow_mut()
        .pop()
        .unwrap()
        .send(Hover {
            contents: HoverContents::Scalar(MarkedString::String("fresh details".into())),
            range: None,
        })
        .expect("the fresh hover waiter is still live");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("editor-definition-card").is_some());
    cx.update(|_, cx| {
        let app = view.read(cx);
        let state = app.editor.read(cx);
        assert_eq!(state.cursor(), 2);
        assert_eq!(
            state.hover_popover().unwrap().hover.contents,
            HoverContents::Scalar(MarkedString::String("fresh details".into()))
        );
    });
}
