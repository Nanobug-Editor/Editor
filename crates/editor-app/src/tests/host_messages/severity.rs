//! Painted reminder colors follow the highest unacknowledged host receipt in both toolbar locations.

use super::*;
use crate::app::messages::MessageLevel;

/// Publish through the production boundary without acknowledging or changing the dock.
fn receipt(form: &mut VisualTestContext, app: &Entity<EditorApp>, level: MessageLevel) {
    form.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.record_host_message(level, "Host receipt", cx)
        });
    });
    draw(form);
}

/// Match the actual clipped quad at `selector`, not just the presence of a decoration element.
fn assert_dot(form: &mut VisualTestContext, selector: &'static str, color: Option<u32>) {
    let Some(color) = color else {
        assert!(
            form.debug_bounds(selector).is_none(),
            "unexpected {selector}"
        );
        return;
    };
    let bounds = form
        .debug_bounds(selector)
        .expect("reminder must be painted");
    form.update(|window, _| {
        let scale = window.scale_factor();
        assert!(
            window.painted_quads().iter().any(|quad| {
                (quad.bounds.left().0 / scale - f32::from(bounds.left())).abs() < 0.5
                    && (quad.bounds.top().0 / scale - f32::from(bounds.top())).abs() < 0.5
                    && (quad.bounds.size.width.0 / scale - f32::from(bounds.size.width)).abs() < 0.5
                    && (quad.bounds.size.height.0 / scale - f32::from(bounds.size.height)).abs()
                        < 0.5
                    && quad.background == gpui_kit::Hsla::from(gpui_kit::rgb(color)).into()
                    && quad.content_mask.bounds.contains(&quad.bounds.center())
            }),
            "{selector} did not paint the expected severity color"
        );
    });
}

/// Resizing projects the same unread severity onto the overflow control without acknowledging it.
fn assert_both_locations(form: &mut VisualTestContext, color: u32) {
    assert_dot(form, "host-messages-dot", Some(color));
    form.simulate_resize(size(px(300.), px(800.)));
    draw(form);
    assert!(form.debug_bounds("host-messages-toggle").is_none());
    assert_dot(form, "host-messages-overflow-dot", Some(color));
    form.simulate_resize(size(px(1100.), px(800.)));
    draw(form);
    assert_dot(form, "host-messages-dot", Some(color));
}

/// New information, warnings and errors escalate; lower receipts, eviction and painting never downgrade.
#[gpui::test]
fn host_messages_severity_colors_preserve_priority_and_acknowledgement(cx: &mut TestAppContext) {
    with_messages(cx, |form, app, _| {
        for (dark, scale, blue, orange, red) in [
            (false, 1., 0x2563eb, 0xf59e0b, 0xdc2626),
            (true, 1.5, 0x60a5fa, 0xfbbf24, 0xf87171),
        ] {
            form.update(|window, cx| {
                apply_theme(builtin_theme(dark), cx);
                window.refresh();
            });
            form.simulate_scale_factor_change(scale);
            draw(form);
            click(form, "host-messages-hide");
            assert_dot(form, "host-messages-dot", None);

            receipt(form, &app, MessageLevel::Info);
            assert_both_locations(form, blue);
            assert!(form.debug_bounds("host-messages-panel").is_none());
            receipt(form, &app, MessageLevel::Warning);
            assert_both_locations(form, orange);
            receipt(form, &app, MessageLevel::Info);
            assert_dot(form, "host-messages-dot", Some(orange));
            receipt(form, &app, MessageLevel::Error);
            assert_both_locations(form, red);
            receipt(form, &app, MessageLevel::Warning);
            form.update(|_, cx| {
                app.update(cx, |app, cx| {
                    for _ in 0..500 {
                        app.record_host_message(MessageLevel::Info, "Newer information", cx);
                    }
                });
            });
            draw(form);
            assert_both_locations(form, red);

            click(form, "host-messages-toggle");
            assert_dot(form, "host-messages-dot", None);
            assert!(form.debug_bounds("host-message-info").is_some());
            assert!(form.debug_bounds("host-message-error").is_none());
            let clear = form.debug_bounds("host-messages-clear").unwrap().center();
            form.simulate_click(clear, Default::default());
            // A result delivered before the clear action's deferred repaint starts a new reminder.
            receipt(form, &app, MessageLevel::Info);
            assert_dot(form, "host-messages-dot", Some(blue));
            click(form, "host-messages-clear");
            assert_dot(form, "host-messages-dot", None);
            assert!(form.debug_bounds("host-messages-empty").is_some());
        }
    });
}
