//! Notification previews navigate through the production card, dock and bounded message list.

use super::*;
use crate::app::messages::MessageLevel;

/// A revealed short receipt must be inside the scroll viewport, not merely present in the element tree.
fn assert_visible(form: &mut VisualTestContext, id: u64) {
    // The headless platform has no native frame loop; deliver the production after-layout callback.
    form.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    draw(form);
    let viewport = form.debug_bounds("host-message-list").unwrap();
    let target = form
        .debug_bounds(row(id))
        .expect("target receipt is disclosed");
    assert!(
        target.top() >= viewport.top() - px(1.),
        "{id}: {target:?} outside {viewport:?}"
    );
    assert!(
        target.bottom() <= viewport.bottom() + px(1.),
        "{id}: {target:?} outside {viewport:?}"
    );
}

/// Explicit newlines and wrapped Chinese descriptions both stay within two actual rendered lines.
#[gpui::test]
fn host_messages_toast_description_is_two_lines(cx: &mut TestAppContext) {
    with_messages(cx, |form, app, _| {
        for (dark, scale) in [(false, 1.), (true, 1.5), (false, 2.)] {
            form.update(|window, cx| {
                apply_theme(builtin_theme(dark), cx);
                app.update(cx, |app, cx| {
                    app.record_host_message(MessageLevel::Error, "Single line", cx)
                });
                window.refresh();
            });
            form.simulate_scale_factor_change(scale);
            draw(form);
            let line_height = form
                .debug_bounds("notification-message-body")
                .unwrap()
                .size
                .height;
            click(form, "host-message-toast-close");
            for message in [
                "第一行\n第二行\n第三行\n第四行".to_owned(),
                "中文错误详情和 English diagnostic ".repeat(60),
            ] {
                form.update(|_, cx| {
                    app.update(cx, |app, cx| {
                        app.record_host_message(MessageLevel::Error, message, cx)
                    });
                });
                draw(form);
                let preview = form.debug_bounds("notification-message-body").unwrap();
                assert!(preview.size.height > line_height);
                assert!(preview.size.height <= line_height * 2. + px(1.));
                click(form, "host-message-toast-close");
            }
        }
    });
}

/// Clicking an old card expands its hidden batch, opens the dock and scrolls to the exact retained row.
#[gpui::test]
fn host_messages_toast_reveals_undisclosed_receipt(cx: &mut TestAppContext) {
    with_messages(cx, |form, app, _| {
        click(form, "host-messages-hide");
        form.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.record_host_message(MessageLevel::Error, "Target receipt", cx);
                for index in 0..45 {
                    app.record_host_message(
                        MessageLevel::Info,
                        format!("Later receipt {index}"),
                        cx,
                    );
                }
            });
        });
        draw(form);
        assert!(form.debug_bounds(row(1)).is_none());
        click(form, "host-message-toast");
        draw(form);
        assert_visible(form, 1);
        assert!(form.debug_bounds("host-messages-dot").is_none());
        // Clicking again while already open must reveal the same row instead of toggling the panel shut.
        click(form, "host-message-toast");
        draw(form);
        assert_visible(form, 1);
    });
}

/// Coalescing follows the latest identical receipt; closing and expired identities never recreate history.
#[gpui::test]
fn host_messages_toast_coalescing_and_stale_targets(cx: &mut TestAppContext) {
    with_messages(cx, |form, app, _| {
        click(form, "host-messages-hide");
        form.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.record_host_message(MessageLevel::Error, "Repeated error", cx);
                for _ in 0..30 {
                    app.record_host_message(MessageLevel::Info, "Intermediate receipt", cx);
                }
                app.record_host_message(MessageLevel::Error, "Repeated error", cx);
                for _ in 0..25 {
                    app.record_host_message(MessageLevel::Info, "Later receipt", cx);
                }
            });
        });
        draw(form);
        click(form, "host-message-toast");
        draw(form);
        assert_visible(form, 32);
        assert!(
            form.debug_bounds(row(1)).is_none(),
            "the coalesced card must target the newer duplicate"
        );
        click(form, "host-messages-clear");
        click(form, "host-messages-hide");
        click(form, "host-message-toast");
        assert!(form.debug_bounds("host-messages-empty").is_some());
        assert!(form.debug_bounds(row(32)).is_none());
        click(form, "host-messages-hide");
        click(form, "host-message-toast-close");
        assert!(
            form.debug_bounds("host-messages-panel").is_none(),
            "closing must not activate the card"
        );
        form.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.record_host_message(MessageLevel::Warning, "Evicted target", cx);
                for _ in 0..500 {
                    app.record_host_message(MessageLevel::Info, "Retained receipt", cx);
                }
            });
        });
        draw(form);
        click(form, "host-message-toast");
        assert!(form.debug_bounds("host-messages-panel").is_some());
        assert!(
            form.debug_bounds(row(58)).is_none(),
            "capacity eviction must remain effective"
        );
    });
}
