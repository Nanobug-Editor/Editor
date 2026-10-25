//! Docked Problems preserves Base controls, native layout gestures and the editor's focus lease.

use super::shortcuts::{click, draw, second_editor_window};
use gpui_base::dock::{DockPlacement, PanelId};
use gpui_kit::{Focusable as _, MouseButton, TestAppContext, gpui, point, px, size};

/// A fresh workspace gives Left the full height, Bottom the remaining width, and Problems the right column above it.
#[gpui::test]
fn native_problems_initial_right_column_and_region_priority(cx: &mut TestAppContext) {
    super::with_shortcut_editor(cx, false, Vec::new(), |visual, app, _| {
        visual.simulate_resize(size(px(1280.), px(820.)));
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.show_problems(window, cx);
                app.toggle_terminal(window, cx);
            });
        });
        draw(visual);
        let (explorer, editor, problems, terminal) = visual.update(|_, cx| {
            let app = app.read(cx);
            let area = app.dock_area.read(cx);
            let node = |placement, id| {
                area.layout(placement)
                    .unwrap()
                    .find_panel_node(PanelId::from(id))
                    .unwrap()
            };
            (
                node(DockPlacement::Left, app.explorer_panel.entity_id()),
                node(DockPlacement::Center, app.editor_panel.entity_id()),
                node(DockPlacement::Center, app.problems.panel.entity_id()),
                node(DockPlacement::Bottom, app.terminal.entity_id()),
            )
        });
        let mut bounds = |node: gpui_base::dock::NodeId| {
            let selector =
                Box::leak(format!("local-dock-content-{}", node.as_u64()).into_boxed_str());
            visual.debug_bounds(selector).unwrap()
        };
        let (left, center, right, bottom) = (
            bounds(explorer),
            bounds(editor),
            bounds(problems),
            bounds(terminal),
        );
        assert!((right.size.width - px(360.)).abs() < px(8.));
        assert!((left.right() - bottom.left()).abs() < px(8.));
        assert!((center.right() - right.left()).abs() < px(8.));
        assert!((bottom.right() - right.right()).abs() < px(8.));
        assert!(left.bottom() > bottom.bottom() - px(36.));
        assert!(right.bottom() <= bottom.top() + px(36.));
    });
}

/// Even an empty projection exposes named controls and returns keyboard focus on Escape.
#[gpui::test]
fn native_problems_heading_controls_and_escape_restore_focus(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    for dark in [false, true] {
        super::with_shortcut_editor_profile(
            cx,
            dark,
            Vec::new(),
            &profile.path().join("shortcuts.json"),
            |visual, app, _| {
                visual.simulate_keystrokes("ctrl-shift-m");
                visual.run_until_parked();
                visual.update(|window, cx| {
                    window.draw(cx).clear(cx);
                    let nodes = gpui_base::test_support::snapshots(window);
                    assert!(
                        !nodes
                            .iter()
                            .any(|node| node.role() == Some(gpui_kit::Role::Dialog))
                    );
                    let heading = nodes
                        .iter()
                        .find(|node| {
                            node.role() == Some(gpui_kit::Role::Heading)
                                && node.label()
                                    == Some(rust_i18n::t!("problems.title", count = 0).as_ref())
                        })
                        .expect("Problems must publish its visible title as a native heading");
                    assert_eq!(
                        heading.label(),
                        Some(rust_i18n::t!("problems.title", count = 0).as_ref())
                    );
                    let label = rust_i18n::t!("problems.close");
                    assert!(nodes.iter().any(|node| {
                        node.role() == Some(gpui_kit::Role::Button)
                            && node.label() == Some(label.as_ref())
                            && node.visible()
                    }));
                });
                assert!(visual.debug_bounds("native-problems-panel").is_some());
                assert!(visual.debug_bounds("syntax-error-indicator").is_none());
                assert!(visual.debug_bounds("problem-reveal").is_none());
                assert!(visual.debug_bounds("native-problems-actions").is_none());
                // The pane remains visible while real platform input edits the native document.
                visual.update(|window, cx| app.read(cx).editor.focus_handle(cx).focus(window, cx));
                visual.simulate_input("中文输入");
                draw(visual);
                assert!(visual.debug_bounds("native-problems-panel").is_some());
                assert!(visual.update(|_, cx| {
                    app.read(cx)
                        .editor
                        .read(cx)
                        .text()
                        .to_string()
                        .contains("中文输入")
                }));
                visual.simulate_keystrokes("ctrl-shift-m");
                draw(visual);
                visual.simulate_keystrokes("escape");
                visual.run_until_parked();
                visual.update(|window, cx| {
                    window.draw(cx).clear(cx);
                    assert!(app.read(cx).editor.focus_handle(cx).is_focused(window));
                    assert!(
                        !gpui_base::test_support::snapshots(window)
                            .iter()
                            .any(|node| node.role() == Some(gpui_kit::Role::Dialog))
                    );
                });
            },
        );
    }
}

/// Resize and move the actual Base leaf, then restore its saved position and visibility in another window.
#[gpui::test]
fn native_problems_drag_resize_and_saved_layout(cx: &mut TestAppContext) {
    super::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        visual.simulate_resize(size(px(1100.), px(850.)));
        draw(visual);
        visual.simulate_keystrokes("ctrl-shift-m");
        draw(visual);
        let (id, node, editor_node) = visual.update(|_, cx| {
            let app = app.read(cx);
            let id = PanelId::from(app.problems.panel.entity_id());
            let area = app.dock_area.read(cx);
            (
                id,
                area.layout(DockPlacement::Center)
                    .unwrap()
                    .find_panel_node(id)
                    .unwrap(),
                area.layout(DockPlacement::Center)
                    .unwrap()
                    .find_panel_node(PanelId::from(app.editor_panel.entity_id()))
                    .unwrap(),
            )
        });
        let title_selector =
            Box::leak(format!("local-dock-title-{}", node.as_u64()).into_boxed_str());
        let editor_selector =
            Box::leak(format!("local-dock-content-{}", editor_node.as_u64()).into_boxed_str());
        let title = visual.debug_bounds(title_selector).unwrap();
        let initial = visual.debug_bounds("native-problems-panel").unwrap();
        // The initial right column has a native split handle just inside its leading edge.
        let divider = point(title.left() + px(2.), initial.center().y);
        visual.simulate_mouse_down(divider, MouseButton::Left, Default::default());
        visual.simulate_mouse_move(
            divider - point(px(20.), px(0.)),
            MouseButton::Left,
            Default::default(),
        );
        visual.simulate_mouse_move(
            divider - point(px(80.), px(0.)),
            MouseButton::Left,
            Default::default(),
        );
        visual.simulate_mouse_up(
            divider - point(px(80.), px(0.)),
            MouseButton::Left,
            Default::default(),
        );
        draw(visual);
        assert!(
            visual
                .debug_bounds("native-problems-panel")
                .unwrap()
                .size
                .width
                > initial.size.width + px(50.),
            "Dragging the right divider must resize the native list"
        );

        let title = visual.debug_bounds(title_selector).unwrap();
        let editor = visual.debug_bounds(editor_selector).unwrap();
        let destination = point(editor.center().x, editor.bottom() - px(10.));
        visual.simulate_mouse_down(title.center(), MouseButton::Left, Default::default());
        visual.simulate_mouse_move(
            title.center() + point(px(12.), px(0.)),
            MouseButton::Left,
            Default::default(),
        );
        visual.simulate_mouse_move(destination, MouseButton::Left, Default::default());
        visual.update(|window, cx| window.draw(cx).clear(cx));
        assert!(visual.debug_bounds("local-dock-drop-indicator").is_some());
        assert!(visual.debug_bounds("local-dock-floating-preview").is_some());
        visual.simulate_mouse_up(destination, MouseButton::Left, Default::default());
        draw(visual);
        let moved = visual.debug_bounds("native-problems-panel").unwrap();
        visual.update(|_, cx| {
            app.update(cx, |app, cx| {
                assert!(
                    app.dock_area
                        .read(cx)
                        .layout(DockPlacement::Center)
                        .unwrap()
                        .find_panel_node(id)
                        .is_some()
                );
                app.capture_dock_layout(cx);
                app.persist_session();
            })
        });
        let (restored, mut restored_visual) = second_editor_window(visual, path.parent().unwrap());
        let recovered = restored_visual
            .debug_bounds("native-problems-panel")
            .unwrap();
        assert!((recovered.size.width - moved.size.width).abs() < px(2.));
        assert!((recovered.size.height - moved.size.height).abs() < px(2.));
        restored_visual.update(|_, cx| {
            let app = restored.read(cx);
            assert!(app.session_state.problems_visible);
            let tree = serde_json::to_string(&app.dock_area.read(cx).dump(cx)).unwrap();
            assert_eq!(
                tree.matches("\"Problems\"").count(),
                1,
                "Restoration must keep exactly one leaf"
            );
        });
        click(&mut restored_visual, "problem-close");
        assert!(
            restored_visual
                .debug_bounds("native-problems-panel")
                .is_none()
        );
    });
}
