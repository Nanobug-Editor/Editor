//! Physical scrolling gestures must target the shared viewport and original native x offset.
use super::*;

/// A wrapped comparison has no horizontal overflow, so its native viewport must reach the pane edge.
#[gpui::test]
fn aligned_editor_wrapped_sources_do_not_reserve_an_empty_bottom_rail(cx: &mut TestAppContext) {
    let (view, ui) = mount(cx);
    for scale in [1., 1.5] {
        ui.simulate_scale_factor_change(scale);
        settle(ui);
        ui.update(|_, cx| {
            let group = view.read(cx).group.state.read(cx);
            for viewport in group.viewports.iter().flatten() {
                assert!(
                    (viewport.bottom() - px(180.)).abs() < px(0.5),
                    "wrapped sources leave an unused bottom strip: {viewport:?}"
                );
            }
        });
    }
    // A real horizontal overflow still reserves a draggable rail and releases it after wrapping.
    for wrapped in [false, true] {
        ui.update(|window, cx| {
            let editors = view.read(cx).editors.clone();
            for editor in editors {
                editor.update(cx, |editor, cx| editor.set_soft_wrap(wrapped, window, cx));
            }
        });
        settle(ui);
        ui.update(|_, cx| {
            let group = view.read(cx).group.state.read(cx);
            let bottom = px(180. - if wrapped { 0. } else { scroll::RAIL });
            for viewport in group.viewports.iter().flatten() {
                assert!((viewport.bottom() - bottom).abs() < px(0.5));
            }
        });
    }
}

/// Locate the actually painted local thumb, not a guessed rail coordinate. Both axes keep their
/// hit surface outside the editor's pointer projection, so the real Base drag must move content.
fn thumb(ui: &mut VisualTestContext, viewport: Bounds<Pixels>, horizontal: bool) -> Point<Pixels> {
    ui.update(|window, _| {
        let scale = window.scale_factor();
        let thumb = window
            .painted_quads()
            .into_iter()
            .find(|quad| {
                let left = px(quad.bounds.left().0 / scale);
                let right = px(quad.bounds.right().0 / scale);
                let top = px(quad.bounds.top().0 / scale);
                let bottom = px(quad.bounds.bottom().0 / scale);
                let in_rail = if horizontal {
                    top >= viewport.bottom()
                        && bottom <= viewport.bottom() + px(scroll::RAIL)
                        && left >= viewport.left()
                        && right <= viewport.right()
                        && right - left >= px(20.)
                        && bottom - top < px(scroll::RAIL)
                } else {
                    left >= viewport.right()
                        && right <= viewport.right() + px(scroll::RAIL)
                        && top >= viewport.top()
                        && bottom <= viewport.bottom()
                        && bottom - top >= px(20.)
                        && right - left < px(scroll::RAIL)
                };
                in_rail
                    && quad.background.as_solid().is_some_and(|color| color.a > 0.)
                    && quad.content_mask.bounds.contains(&quad.bounds.center())
            })
            .expect("the local scrollbar thumb must actually paint");
        point(
            px(thumb.bounds.center().x.0 / scale),
            px(thumb.bounds.center().y.0 / scale),
        )
    })
}

#[gpui::test]
fn aligned_editor_real_scrollbar_drags_and_diagonal_wheel_preserve_native_horizontal_axis(
    cx: &mut TestAppContext,
) {
    use gpui_kit::{MouseButton, ScrollDelta, ScrollEvent, TouchPhase};
    let (view, ui) = mount(cx);
    ui.update(|window, cx| {
        let editors = view.read(cx).editors.clone();
        for editor in editors {
            editor.update(cx, |editor, cx| editor.set_soft_wrap(false, window, cx));
        }
    });
    settle(ui);
    let before = ui.update(|_, cx| {
        let fixture = view.read(cx);
        (
            fixture.group.scroll_offset(cx),
            fixture.editors[0].read(cx).selected_range(),
        )
    });
    ui.simulate_event(ScrollEvent {
        // Wheel and trackpad fixtures retain pointer-device semantics.
        source: gpui_kit::ScrollSource::Pointer,
        position: point(px(120.), px(80.)),
        delta: ScrollDelta::Pixels(point(px(-50.), px(-55.))),
        touch_phase: TouchPhase::Moved,
        modifiers: Default::default(),
    });
    settle(ui);
    let (viewport, scrolled) = ui.update(|_, cx| {
        let fixture = view.read(cx);
        assert!(fixture.group.scroll_offset(cx) > before.0);
        assert!(
            fixture.editors[0].read(cx).scroll_offset().x < px(0.),
            "consuming shared y must preserve the same touchpad event's native x"
        );
        assert_eq!(fixture.editors[1].read(cx).scroll_offset().x, px(0.));
        (
            fixture.group.viewport(0, cx).unwrap(),
            fixture.group.scroll_offset(cx),
        )
    });
    ui.simulate_mouse_move(
        point(viewport.right() + px(6.), viewport.top() + px(25.)),
        None,
        Default::default(),
    );
    settle(ui);
    let start = thumb(ui, viewport, false);
    let end = start + point(px(0.), px(35.));
    ui.simulate_mouse_down(start, MouseButton::Left, Default::default());
    ui.simulate_mouse_move(end, MouseButton::Left, Default::default());
    ui.simulate_mouse_up(end, MouseButton::Left, Default::default());
    ui.run_until_parked();
    settle(ui);
    let native_x = ui.update(|_, cx| {
        let fixture = view.read(cx);
        assert!(
            fixture.group.scroll_offset(cx) > scrolled + px(50.),
            "the actually painted native thumb must drag the shared viewport"
        );
        assert_eq!(
            fixture.editors[0].read(cx).selected_range(),
            before.1,
            "a scrollbar drag cannot become an editor selection"
        );
        let group = fixture.group.state.read(cx);
        let mut y = Vec::new();
        for index in 0..2 {
            let text = content(index == 1);
            assert_eq!(fixture.editors[index].read(cx).value().as_ref(), text);
            let byte = byte_at(&text, 22);
            let bounds = fixture.editors[index]
                .read(cx)
                .range_to_bounds(&(byte..byte))
                .unwrap();
            y.push(group.project(index, bounds).unwrap().top());
        }
        assert!(
            (y[0] - y[1]).abs() < px(0.5),
            "peer panes must redraw at the dragged shared offset"
        );
        fixture.editors[0].read(cx).scroll_offset().x
    });
    ui.simulate_mouse_move(
        point(viewport.left() + px(50.), viewport.bottom() + px(6.)),
        None,
        Default::default(),
    );
    settle(ui);
    let start = thumb(ui, viewport, true);
    let end = start + point(px(40.), px(0.));
    ui.simulate_mouse_down(start, MouseButton::Left, Default::default());
    ui.simulate_mouse_move(end, MouseButton::Left, Default::default());
    ui.simulate_mouse_up(end, MouseButton::Left, Default::default());
    ui.run_until_parked();
    settle(ui);
    ui.update(|_, cx| {
        let fixture = view.read(cx);
        assert!(
            fixture.editors[0].read(cx).scroll_offset().x < native_x,
            "the local horizontal thumb must reach the original source ScrollHandle"
        );
        assert_eq!(fixture.editors[1].read(cx).scroll_offset().x, px(0.));
        assert_eq!(fixture.editors[0].read(cx).selected_range(), before.1);
    });
}
