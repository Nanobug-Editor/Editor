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
        crate::ui::typography::init(cx);
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
        crate::ui::typography::init(cx);
        crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(false), cx);
    });
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let mut document = Document::new(Node::text("body", "Body"));
        document.menu = Some(ui::PopupMenu {
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
