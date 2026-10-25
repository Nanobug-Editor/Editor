//! Real host persistence must see only the completed visible-neighbor height gesture.

use super::*;

/// A retained hidden Outline between Explorer and another ordinary tool reproduces Commit's seam.
#[gpui::test]
fn native_split_resize_skips_hidden_panel_and_saves_on_release(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, false);
    let [explorer, outline, _] = panels(&app, visual);
    let tool = visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.messages
                .update(cx, |panel, cx| panel.set_visible(true, cx));
            let tool = PanelId::from(app.messages.entity_id());
            app.dock_area.update(cx, |area, cx| {
                let node = area
                    .layout(DockPlacement::Left)
                    .unwrap()
                    .find_panel_node(outline)
                    .unwrap();
                area.move_panel(
                    tool,
                    gpui_base::dock::InsertTarget::Split {
                        node,
                        placement: gpui_base::Placement::Bottom,
                        size: Some(px(300.)),
                    },
                    window,
                    cx,
                );
            });
            tool
        })
    });
    let before = outer(&app, tool, visual);
    let upper = outer(&app, explorer, visual);
    let saved = visual.update(|_, cx| app.read(cx).session_state.dock_layout.clone());
    let start = point(before.center().x, before.top());
    visual.simulate_mouse_down(start, MouseButton::Left, Default::default());
    for delta in [10., 35., 60., 60., 20.] {
        hover(start + point(px(0.), px(delta)), visual);
        let after = outer(&app, tool, visual);
        assert!(
            (after.top() - before.top() - px(delta)).abs() < px(2.),
            "tool boundary must follow each move"
        );
        let after_upper = outer(&app, explorer, visual);
        assert!((after_upper.size.height - upper.size.height - px(delta)).abs() < px(2.));
        visual.update(|_, cx| {
            assert!(app.read(cx).dock_ui.is_dragging());
            assert_eq!(
                app.read(cx).session_state.dock_layout,
                saved,
                "moves must not persist intermediate geometry"
            );
        });
    }
    visual.simulate_mouse_up(
        start + point(px(0.), px(20.)),
        MouseButton::Left,
        Default::default(),
    );
    outer(&app, tool, visual);
    visual.update(|_, cx| {
        let app = app.read(cx);
        assert!(!app.dock_ui.is_dragging());
        assert_ne!(
            app.session_state.dock_layout, saved,
            "release persists the final height"
        );
        assert!(app.dock_area.read(cx).panel(tool).is_some());
    });
}
