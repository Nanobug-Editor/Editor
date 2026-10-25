//! Exclusive drop previews and committed display are exercised through real pointer events.
use super::*;

/// A target moved to another actual side invalidates the old gesture without replaying its snapshot.
#[gpui::test]
fn exclusive_drop_cancels_when_target_moves_side(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    let [explorer, _outline, editor] = panels(&app, visual);
    let bottom = visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            let id = PanelId::from(app.problems.panel.entity_id());
            app.dock_presentation.set_exclusive(id, true);
            app.dock_presentation.activate(id);
            app.sync_plugin_panels(window, cx);
            id
        })
    });
    let target = outer(&app, bottom, visual);
    begin(&app, explorer, visual);
    hover(target.center(), visual);
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.dock_area.update(cx, |area, cx| {
                let node = area
                    .layout(DockPlacement::Center)
                    .unwrap()
                    .find_panel_node(editor)
                    .unwrap();
                area.move_panel(
                    bottom,
                    gpui_base::dock::InsertTarget::Split {
                        node,
                        placement: gpui_base::Placement::Right,
                        size: Some(px(240.)),
                    },
                    window,
                    cx,
                );
            });
        });
    });
    let moved = outer(&app, bottom, visual);
    hover(moved.center(), visual);
    assert!(
        visual.debug_bounds("local-dock-drop-indicator").is_none(),
        "a stale gesture must not preview the target's new side using the old transaction"
    );
    visual.simulate_mouse_up(moved.center(), MouseButton::Left, Default::default());
    visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        assert!(
            area.layout(DockPlacement::Center)
                .unwrap()
                .find_panel_node(bottom)
                .is_some()
        );
        assert!(
            area.layout(DockPlacement::Left)
                .unwrap()
                .find_panel_node(explorer)
                .is_some()
        );
    });
}

/// Moving an exclusive source away must reveal, not unregister, its retained ordinary source peer.
#[gpui::test]
fn exclusive_drop_keeps_hidden_source_region_peers(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    let [explorer, outline, _] = panels(&app, visual);
    let bottom = visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            let id = PanelId::from(app.problems.panel.entity_id());
            app.dock_area.update(cx, |area, cx| {
                let node = area
                    .layout(DockPlacement::Bottom)
                    .unwrap()
                    .find_panel_node(id)
                    .unwrap();
                area.move_panel(
                    outline,
                    gpui_base::dock::InsertTarget::Split {
                        node,
                        placement: gpui_base::Placement::Right,
                        size: None,
                    },
                    window,
                    cx,
                );
            });
            app.dock_presentation.set_exclusive(id, true);
            app.dock_presentation.activate(id);
            app.sync_plugin_panels(window, cx);
            id
        })
    });
    let destination = outer(&app, explorer, visual);
    begin(&app, bottom, visual);
    hover(destination.center(), visual);
    visual.simulate_mouse_up(destination.center(), MouseButton::Left, Default::default());
    visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        let retained = area
            .panel(outline)
            .expect("the old ordinary peer must remain registered after exclusive source removal");
        assert!(retained.visible(cx));
        assert!(
            area.layout(DockPlacement::Bottom)
                .unwrap()
                .find_panel_node(outline)
                .is_some()
        );
    });
    assert!(outer(&app, outline, visual).size.width > px(800.));
}

/// Dropping an ordinary window onto an exclusive region replaces its entire visible allocation.
#[gpui::test]
fn exclusive_drop_ordinary_window_replaces_the_whole_target(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    let [explorer, outline, _editor] = panels(&app, visual);
    let bottom = visual.update(|_, cx| PanelId::from(app.read(cx).problems.panel.entity_id()));
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            // Retain a hidden ordinary peer so replacement cannot accidentally reveal a combination.
            app.dock_area.update(cx, |area, cx| {
                let node = area
                    .layout(DockPlacement::Bottom)
                    .unwrap()
                    .find_panel_node(bottom)
                    .unwrap();
                area.move_panel(
                    outline,
                    gpui_base::dock::InsertTarget::Split {
                        node,
                        placement: gpui_base::Placement::Right,
                        size: None,
                    },
                    window,
                    cx,
                );
            });
            app.dock_presentation.set_exclusive(bottom, true);
            app.dock_presentation.activate(bottom);
            app.sync_plugin_panels(window, cx);
        });
    });
    let target = outer(&app, bottom, visual);
    let original = outer(&app, explorer, visual);
    begin(&app, explorer, visual);
    hover(target.center(), visual);
    assert_eq!(marked(visual, "local-dock-drop-indicator"), target);
    assert_eq!(
        outer(&app, explorer, visual),
        original,
        "hover must not edit the source layout"
    );
    visual.simulate_mouse_up(target.center(), MouseButton::Left, Default::default());
    let replacement = outer(&app, explorer, visual);
    // Explorer was the last leaf in the left dock, so releasing that source gives Bottom the
    // complete 1280px viewport. The preview must still cover the target's unchanged pre-drop slot.
    assert!(
        (replacement.size.width - px(1280.)).abs() < px(2.),
        "whole replacement: {replacement:?}, target before drop: {target:?}"
    );
    assert!(replacement.left().abs() < px(2.));
    assert!(
        (replacement.size.height - target.size.height).abs() < px(2.),
        "whole replacement: {replacement:?}, target before drop: {target:?}"
    );
    visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        assert!(area.panel(explorer).unwrap().visible(cx));
        assert!(
            !area.panel(bottom).unwrap().visible(cx),
            "the replaced window remains registered but hidden"
        );
        assert!(
            !area.panel(outline).unwrap().visible(cx),
            "ordinary peers must not leak into a full replacement"
        );
    });
    let saved = visual.update(|_, cx| app.read(cx).session_state.clone());
    let workspace = Workspace::open(_directory.path()).unwrap();
    let (restored, visual) = open(cx, workspace, saved);
    let [explorer, outline, _] = panels(&restored, visual);
    let bottom = visual.update(|_, cx| PanelId::from(restored.read(cx).problems.panel.entity_id()));
    let reopened = outer(&restored, explorer, visual);
    assert!((reopened.size.width - replacement.size.width).abs() < px(2.));
    visual.update(|_, cx| {
        let area = restored.read(cx).dock_area.read(cx);
        assert!(area.panel(explorer).unwrap().visible(cx));
        assert!(
            !area.panel(bottom).unwrap().visible(cx),
            "a fresh entity binding must retain the user's whole-region replacement"
        );
        assert!(!area.panel(outline).unwrap().visible(cx));
    });
    // A later explicit ordinary reveal restores the ordinary combination, even when that same
    // window previously owned the side through a temporary full-replacement selection.
    for id in ["terminal-toggle", "explorer-panel-toggle"] {
        visual.update(|window, cx| window.draw(cx).clear(cx));
        let button = visual.debug_bounds(id).unwrap();
        visual.simulate_mouse_down(button.center(), MouseButton::Left, Default::default());
        visual.simulate_mouse_up(button.center(), MouseButton::Left, Default::default());
    }
    visual.update(|_, cx| {
        let app = restored.read(cx);
        let area = app.dock_area.read(cx);
        assert!(area.panel(explorer).unwrap().visible(cx));
        assert!(
            area.panel(outline).unwrap().visible(cx),
            "an explicit ordinary reveal must clear the old single-window replacement selection"
        );
        assert!(app.session_state.dock_replacement_panels.is_empty());
    });
}

/// The native terminal defaults to the same exclusive selection as a declared exclusive plugin.
#[gpui::test]
fn terminal_default_exclusive_replaces_ordinary_bottom_peer(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    let (bottom, terminal) = visual.update(|_, cx| {
        let app = app.read(cx);
        (
            PanelId::from(app.problems.panel.entity_id()),
            PanelId::from(app.terminal.entity_id()),
        )
    });
    outer(&app, bottom, visual);
    let button = visual.debug_bounds("terminal-toggle").unwrap();
    visual.simulate_mouse_down(button.center(), MouseButton::Left, Default::default());
    visual.simulate_mouse_up(button.center(), MouseButton::Left, Default::default());
    let shown = outer(&app, terminal, visual);
    visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        assert!(area.panel(terminal).unwrap().visible(cx));
        assert!(
            !area.panel(bottom).unwrap().visible(cx),
            "the default terminal must replace rather than split its ordinary peer"
        );
    });
    assert!(shown.size.width > px(900.));
}

/// An exclusive source replaces the destination's entire ordinary combination, not just one cell.
#[gpui::test]
fn exclusive_drop_source_replaces_ordinary_side_and_escape_preserves_layout(
    cx: &mut TestAppContext,
) {
    let (_directory, app, visual) = setup(cx, true);
    let [explorer, outline, editor] = panels(&app, visual);
    let bottom = visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            let id = PanelId::from(app.problems.panel.entity_id());
            app.dock_presentation.set_exclusive(id, true);
            app.dock_presentation.activate(id);
            app.sync_plugin_panels(window, cx);
            id
        })
    });
    let first = outer(&app, explorer, visual);
    let second = outer(&app, outline, visual);
    let region = Bounds::new(
        first.origin,
        size(first.size.width, second.bottom() - first.top()),
    );
    let before = visual.update(|_, cx| app.read(cx).session_state.dock_layout.clone());
    begin(&app, bottom, visual);
    hover(first.center(), visual);
    assert_eq!(marked(visual, "local-dock-drop-indicator"), region);
    visual.simulate_keystrokes("escape");
    assert_eq!(outer(&app, explorer, visual), first);
    assert_eq!(outer(&app, outline, visual), second);
    assert_eq!(
        visual.update(|_, cx| app.read(cx).session_state.dock_layout.clone()),
        before
    );
    // Both the center and the opposite window edge use the same full-region replacement.
    begin(&app, bottom, visual);
    let pointer = edge(second, gpui_base::Placement::Bottom);
    hover(pointer, visual);
    assert_eq!(marked(visual, "local-dock-drop-indicator"), region);
    visual.simulate_mouse_up(pointer, MouseButton::Left, Default::default());
    let shown = outer(&app, bottom, visual);
    let editor = outer(&app, editor, visual);
    assert!((shown.size.width - region.size.width).abs() < px(2.));
    assert!((shown.top() - editor.top()).abs() < px(2.));
    assert!((shown.bottom() - editor.bottom()).abs() < px(2.));
    visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        assert!(area.panel(bottom).unwrap().visible(cx));
        for id in [explorer, outline] {
            assert!(!area.panel(id).unwrap().visible(cx));
        }
    });
}
