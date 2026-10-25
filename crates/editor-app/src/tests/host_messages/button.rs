//! The message toggle must paint the same persistent selection as other window controls.

use super::*;
use gpui_base::dock::{DockLayout, DockPlacement, InsertTarget, PanelId};
use gpui_base::test_support::snapshots;

/// Inspect the actual button fill after moving away, so hover cannot masquerade as selection.
fn assert_selection(form: &mut VisualTestContext, visible: bool) {
    form.update(|window, cx| window.simulate_mouse_move(point(px(700.), px(400.)), cx));
    draw(form);
    assert_eq!(form.debug_bounds("host-messages-panel").is_some(), visible);
    let bounds = form.debug_bounds("host-messages-toggle").unwrap();
    form.update(|window, cx| {
        let button = snapshots(window)
            .into_iter()
            .find(|node| {
                node.visible()
                    && node.path().last()
                        == Some(&gpui_kit::ElementId::Name("toggle-host-messages".into()))
            })
            .expect("the real message button is painted");
        assert_eq!(button.expanded(), Some(visible));
        let selected = component_styles(cx, ThemeComponent::PanelToggle).selected;
        let background = selected.background.unwrap_or(cx.theme().list_active);
        let scale = window.scale_factor();
        let painted_selection = window.painted_quads().iter().any(|quad| {
            (quad.bounds.left().0 / scale - f32::from(bounds.left())).abs() < 0.5
                && (quad.bounds.top().0 / scale - f32::from(bounds.top())).abs() < 0.5
                && (quad.bounds.size.width.0 / scale - f32::from(bounds.size.width)).abs() < 0.5
                && (quad.bounds.size.height.0 / scale - f32::from(bounds.size.height)).abs() < 0.5
                && quad.background == background.into()
                && quad.content_mask.bounds.contains(&quad.bounds.center())
        });
        assert_eq!(
            painted_selection, visible,
            "message button selection must follow the visible panel after pointer exit"
        );
    });
}

/// Opening, both close routes and a closed dock retain correct paint in either theme and scale.
#[gpui::test]
fn host_messages_button_selection_tracks_panel_visibility(cx: &mut TestAppContext) {
    with_messages(cx, |form, app, _| {
        for (dark, scale) in [(false, 1.), (true, 1.5)] {
            form.update(|window, cx| {
                apply_theme(builtin_theme(dark), cx);
                window.refresh();
            });
            form.simulate_scale_factor_change(scale);
            assert_selection(form, true);
            click(form, "host-messages-toggle");
            assert_selection(form, false);
            click(form, "host-messages-toggle");
            assert_selection(form, true);
            click(form, "host-messages-hide");
            assert_selection(form, false);
            click(form, "host-messages-toggle");
            assert_selection(form, true);
            form.update(|window, cx| {
                let (dock, message) = {
                    let app = app.read(cx);
                    (
                        app.dock_area.clone(),
                        PanelId::from(app.messages.entity_id()),
                    )
                };
                dock.update(cx, |area, cx| {
                    // A saved outer-right placement still supports closing and reopening its dock.
                    if !area.has_dock(DockPlacement::Right) {
                        area.set_dock(DockPlacement::Right, DockLayout::tabs(), window, cx);
                        let node = area.layout(DockPlacement::Right).unwrap().root().id();
                        area.move_panel(
                            message,
                            InsertTarget::Split {
                                node,
                                placement: gpui_base::Placement::Right,
                                size: Some(px(360.)),
                            },
                            window,
                            cx,
                        );
                    }
                    area.toggle_dock(DockPlacement::Right, window, cx);
                });
            });
            assert_selection(form, false);
            click(form, "host-messages-toggle");
            assert_selection(form, true);
        }
    });
}
