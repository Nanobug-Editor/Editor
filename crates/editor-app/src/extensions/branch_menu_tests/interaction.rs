//! Observe real row backgrounds and deliver counted native presses without bypassing tree handlers.
use super::*;

/// Return the last row-sized background matching the provider row in logical coordinates.
/// Popup/background quads and unpainted state fields cannot stand in for the row's visible color.
pub(super) fn background(visual: &mut VisualTestContext, selector: &'static str) -> gpui_kit::Hsla {
    let row = visual.debug_bounds(selector).expect("painted provider row");
    visual.update(|window, _| {
        let scale = window.scale_factor();
        window
            .painted_quads()
            .into_iter()
            .filter(|quad| {
                (quad.bounds.left().0 / scale - f32::from(row.left())).abs() < 1.
                    && (quad.bounds.top().0 / scale - f32::from(row.top())).abs() < 1.
                    && (quad.bounds.size.width.0 / scale - f32::from(row.size.width)).abs() < 1.
                    && (quad.bounds.size.height.0 / scale - f32::from(row.size.height)).abs() < 1.
            })
            .filter_map(|quad| quad.background.as_solid().map(|color| (quad.order, color)))
            .max_by_key(|(order, _)| *order)
            .expect("actual native row background")
            .1
    })
}

/// Supply the native second press after the test's ordinary first click; selection uses production code.
pub(super) fn second_press(visual: &mut VisualTestContext, selector: &'static str) {
    let position = visual.debug_bounds(selector).unwrap().center();
    visual.simulate_event(gpui_kit::MouseDownEvent {
        position,
        button: gpui_kit::MouseButton::Left,
        modifiers: Default::default(),
        click_count: 2,
        first_mouse: false,
    });
    visual.simulate_event(gpui_kit::MouseUpEvent {
        position,
        button: gpui_kit::MouseButton::Left,
        modifiers: Default::default(),
        click_count: 2,
    });
    visual.run_until_parked();
}
