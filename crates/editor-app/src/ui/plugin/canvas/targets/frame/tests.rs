//! Native input keeps the outer UI revision of an unchanged, actually published display proof.
use crate::ui::plugin::{Environment, PluginView, images::VectorRenderer};
use gpui_kit::{
    AppContext as _, KeyDownEvent, KeyUpEvent, Keystroke, MouseButton, MouseUpEvent, PlatformInput,
    TestAppContext, point, px, size,
};
use plugin_runtime::plugin_protocol::ui::{
    self, Action, Canvas, CanvasEvent, Document, Kind, Node,
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};

/// A new visibility press ticket cannot invent a newer UI presentation proof for the same scene.
#[gpui_kit::gpui::test]
fn retained_visibility_same_scene_input_keeps_published_ui_revision(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::typography::init(cx);
        editor_ui::theme::apply_theme(editor_ui::theme::builtin_theme(false), cx);
        editor_ui::init(cx);
    });
    let mut canvas = Canvas::default();
    canvas.scene = Some(
        serde_json::from_value(serde_json::json!({
            "identity": 71, "revision": 1, "roots": [],
            "targets": [{"id": "surface", "epoch": 1, "focusable": true,
                "focus_on_down": true, "capture_on_down": true,
                "geometry": {"kind": "rect", "rect": {"x": 0, "y": 0, "w": 300, "h": 250}}}]
        }))
        .unwrap(),
    );
    let document =
        Document::new(Node::new("drawing", Kind::Canvas(canvas)).height(300.)).revision(1);
    let next_document = document.clone().revision(2);
    let mut renderer = VectorRenderer::default();
    renderer.set_environment(Environment::default());
    let images = renderer.prepare_resources(
        &BTreeMap::from([("ordinary/panel".into(), Arc::new(document.clone()))]),
        &BTreeMap::new(),
    );
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            let mut view = PluginView::new(
                "ordinary".into(),
                document,
                Environment::default(),
                move |event, _| output.borrow_mut().push(event),
                window,
                cx,
            );
            view.update_images("ordinary/panel", &images, window, cx);
            view
        });
        capture.replace(Some(view.clone()));
        gpui_base::Root::new(view, window, cx)
    });
    let view = slot.borrow_mut().take().unwrap();
    visual.simulate_resize(size(px(400.), px(300.)));
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    visual.run_until_parked();
    visual.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.set_scene_current(false, cx);
            view.set_scene_current(true, cx);
        });
    });
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    visual.run_until_parked();
    let receipts = || {
        events
            .borrow()
            .iter()
            .filter(|event| {
                matches!(
                    event.action,
                    Action::Canvas(CanvasEvent::ScenePresented { error: None, .. })
                )
            })
            .map(|event| event.revision)
            .collect::<Vec<_>>()
    };
    let before = receipts();
    assert_eq!(
        before.last(),
        Some(&1),
        "the unchanged scene must have a real published proof"
    );
    events
        .borrow_mut()
        .retain(|event| !matches!(event.action, Action::Canvas(CanvasEvent::Input { .. })));
    // Establish real native capture/focus first, then release after the publication in one
    // UI turn. No draw can replace the previously registered key/window-pointer listeners.
    visual.simulate_mouse_down(
        point(px(50.), px(50.)),
        MouseButton::Left,
        Default::default(),
    );
    let keystroke = Keystroke::parse("right").unwrap();
    visual.simulate_event(KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.update_document(next_document, Environment::default(), window, cx)
        });
        window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke }), cx);
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position: point(px(350.), px(280.)),
                button: MouseButton::Left,
                click_count: 1,
                modifiers: Default::default(),
            }),
            cx,
        );
    });
    visual.run_until_parked();
    {
        let events = events.borrow();
        for phase in [ui::CanvasKeyPhase::Down, ui::CanvasKeyPhase::Up] {
            assert!(events.iter().any(|event| event.revision == 1 && matches!(&event.action,
                Action::Canvas(CanvasEvent::Input { input })
                    if matches!(&input.kind, ui::CanvasInputKind::Key { phase: actual, .. } if *actual == phase))),
                "native key {phase:?} must retain the original proof without drawing");
        }
        for phase in [ui::CanvasPointerPhase::Down, ui::CanvasPointerPhase::Up] {
            assert!(events.iter().any(|event| event.revision == 1 && matches!(&event.action,
                Action::Canvas(CanvasEvent::Input { input })
                    if matches!(&input.kind, ui::CanvasInputKind::Pointer(pointer) if pointer.phase == phase))),
                "captured native pointer {phase:?} must retain the original proof without drawing");
        }
    }
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    visual.run_until_parked();
    assert_eq!(
        receipts(),
        before,
        "an identical display receipt must remain deduplicated"
    );
    visual.simulate_click(point(px(50.), px(50.)), Default::default());
    visual.run_until_parked();
    let events = events.borrow();
    let input = events
        .iter()
        .find(|event| {
            matches!(
                &event.action, Action::Canvas(CanvasEvent::Input { input })
                    if matches!(&input.kind, ui::CanvasInputKind::Pointer(pointer)
                        if pointer.phase == ui::CanvasPointerPhase::Down)
            )
        })
        .expect("a real native hit must emit typed pointer input");
    assert_eq!(
        input.revision, 1,
        "input must reference the outer UI revision that actually published its proof"
    );
}
