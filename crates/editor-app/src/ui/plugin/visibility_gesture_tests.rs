//! Retaining a hidden native view must not revive a button press from its previous display.
use super::*;
use gpui_base::Root;
use gpui_kit::gpui;
use gpui_kit::{MouseButton, TestAppContext};
use plugin_runtime::plugin_protocol::ui::Node;
use std::cell::RefCell;

/// Exercise ordinary controls without document-source authority, with and without a hidden frame.
#[gpui::test]
fn retained_view_visibility_revokes_ordinary_held_presses(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::typography::init(cx);
        crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(false), cx);
    });
    for (modal, draw_hidden_frame) in [(false, false), (false, true), (true, false), (true, true)] {
        let events = Rc::new(RefCell::new(Vec::new()));
        let output = events.clone();
        let slot = Rc::new(RefCell::new(None));
        let capture = slot.clone();
        let (_, visual) = cx.add_window_view(move |window, cx| {
            let view = cx.new(|cx| {
                PluginView::new(
                    "ordinary-controls".into(),
                    if modal {
                        Document::new(Node::text("root", "Body")).dialog(ui::Dialog::new(
                            "modal",
                            "Modal",
                            Node::text("body", "Content"),
                        ))
                    } else {
                        Document::new(Node::button("activate", "Activate"))
                    },
                    Environment::default(),
                    move |event, _| output.borrow_mut().push(event),
                    window,
                    cx,
                )
            });
            *capture.borrow_mut() = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = slot.borrow().clone().unwrap();
        visual.update(|window, cx| window.draw(cx).clear(cx));
        let selector = if modal {
            "plugin-dialog-close"
        } else {
            "plugin-button-activate"
        };
        events.borrow_mut().clear();
        let position = visual
            .debug_bounds(selector)
            .expect("ordinary native button")
            .center();
        visual.simulate_event(gpui_kit::MouseDownEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            first_mouse: false,
            modifiers: Default::default(),
        });
        visual.update(|_, cx| view.update(cx, |view, cx| view.set_scene_current(false, cx)));
        if draw_hidden_frame {
            visual.update(|window, cx| window.draw(cx).clear(cx));
        }
        visual.update(|_, cx| view.update(cx, |view, cx| view.set_scene_current(true, cx)));
        if draw_hidden_frame {
            visual.update(|window, cx| window.draw(cx).clear(cx));
        }
        visual.simulate_event(gpui_kit::MouseUpEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Default::default(),
        });
        visual.run_until_parked();
        assert!(
            !events
                .borrow()
                .iter()
                .any(|event| matches!(event.action, Action::Click | Action::Dismiss)),
            "a hidden/reopened view cannot adopt an earlier Down; frame={draw_hidden_frame}"
        );
        visual.update(|window, cx| window.draw(cx).clear(cx));
        let position = visual.debug_bounds(selector).unwrap().center();
        visual.simulate_click(position, Default::default());
        visual.run_until_parked();
        assert_eq!(
            events
                .borrow()
                .iter()
                .filter(|event| matches!(event.action, Action::Click | Action::Dismiss))
                .count(),
            1,
            "a fresh native press remains usable after reopening"
        );
    }
}

/// A reopened declared popup gets fresh native state and the same visibility admission as controls.
#[gpui::test]
fn retained_view_popup_remains_usable_after_visibility_withdrawal(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::typography::init(cx);
        crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(false), cx);
    });
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let mut document = Document::new(Node::text("body", "Body"));
        document.menu = Some(ui::PopupMenu {
            anchor: None,
            id: "popup".into(),
            x: 20.,
            y: 20.,
            items: vec![ui::MenuItem {
                id: "item".into(),
                label: "Action".into(),
                disabled: false,
                separator_before: false,
            }],
        });
        let view = cx.new(|cx| {
            PluginView::new(
                "ordinary-popup".into(),
                document,
                Environment::default(),
                move |event, _| output.borrow_mut().push(event),
                window,
                cx,
            )
        });
        *capture.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = slot.borrow().clone().unwrap();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    visual.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.set_scene_current(false, cx);
            view.set_scene_current(true, cx);
        })
    });
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    visual.run_until_parked();
    events.borrow_mut().clear();
    visual.simulate_keystrokes("enter");
    visual.run_until_parked();
    assert!(events.borrow().iter().any(|event| event.node == "popup"
        && matches!(&event.action, Action::Select(id) if id == "item")));
}

/// Snapshot rail presses bind their original literal UI without acquiring document authority.
#[gpui::test]
fn snapshot_editor_action_replacement_revokes_held_native_press_before_draw(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::typography::init(cx);
        crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(false), cx);
        editor_ui::init(cx);
    });
    let document = |revision, text: &str| {
        let mut editor = ui::Editor::snapshot(text);
        editor.line_actions = Some(ui::EditorActions {
            left: vec![ui::EditorAction {
                offset: 0,
                content: Box::new(Node::button("activate", "Activate")),
            }],
            right: vec![],
        });
        Document::new(Node::new("snapshot", Kind::Editor(editor)).height(180.)).revision(revision)
    };
    let first = document(1, "Before");
    let next = document(2, "After");
    first.validate().unwrap();
    next.validate().unwrap();
    assert!(first.editor_action_source("activate").is_none());
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            PluginView::new(
                "snapshot-controls".into(),
                first,
                Environment::default(),
                move |event, _| output.borrow_mut().push(event),
                window,
                cx,
            )
        });
        capture.replace(Some(view.clone()));
        Root::new(view, window, cx)
    });
    let view = slot.borrow_mut().take().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    let position = visual
        .debug_bounds("plugin-button-activate")
        .expect("actual snapshot rail button")
        .center();
    visual.simulate_mouse_down(position, MouseButton::Left, Default::default());
    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.update_document(next, Environment::default(), window, cx)
        });
        window.dispatch_event(
            gpui_kit::PlatformInput::MouseUp(gpui_kit::MouseUpEvent {
                position,
                button: MouseButton::Left,
                click_count: 1,
                modifiers: Default::default(),
            }),
            cx,
        );
    });
    visual.run_until_parked();
    assert!(
        !events
            .borrow()
            .iter()
            .any(|event| matches!(event.action, Action::Click)),
        "the replacement snapshot cannot adopt the old rail's pending Down"
    );
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    let position = visual
        .debug_bounds("plugin-button-activate")
        .unwrap()
        .center();
    visual.simulate_click(position, Default::default());
    visual.run_until_parked();
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|event| event.revision == 2 && matches!(event.action, Action::Click))
            .count(),
        1,
        "fresh native input remains usable with the replacement UI ticket"
    );
}
