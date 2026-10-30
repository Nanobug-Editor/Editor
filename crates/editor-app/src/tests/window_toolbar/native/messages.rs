//! Real Windows/D3D reminder painting uses the same host publication and original button input.

use super::*;
use crate::app::messages::MessageLevel;

/// Check the three reminder colors on the owned native window, then restore an empty closed panel.
pub(super) async fn exercise(
    handle: AnyWindowHandle,
    app: &Entity<EditorApp>,
    output: &Path,
    label: &str,
    dark: bool,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    for (level, name, color) in [
        (
            MessageLevel::Info,
            "info",
            if dark { 0x60a5fa } else { 0x2563eb },
        ),
        (
            MessageLevel::Warning,
            "warning",
            if dark { 0xfbbf24 } else { 0xf59e0b },
        ),
        (
            MessageLevel::Error,
            "error",
            if dark { 0xf87171 } else { 0xdc2626 },
        ),
    ] {
        handle.update(cx, |_, _, cx| {
            app.update(cx, |app, cx| {
                app.record_host_message(level, format!("Host {name}"), cx)
            });
        })?;
        frame(handle, cx).await?;
        let button = button_bounds(handle, "host-messages-toggle", cx)?;
        ensure!(
            handle.update(cx, |_, window, _| {
                let scale = window.scale_factor();
                window.painted_quads().iter().any(|quad| {
                    let center = point(
                        px(quad.bounds.center().x.0 / scale),
                        px(quad.bounds.center().y.0 / scale),
                    );
                    button.contains(&center)
                        && (quad.bounds.size.width.0 / scale - 6.).abs() < 0.5
                        && (quad.bounds.size.height.0 / scale - 6.).abs() < 0.5
                        && quad.background == gpui_kit::Hsla::from(gpui_kit::rgb(color)).into()
                        && quad.content_mask.bounds.contains(&quad.bounds.center())
                })
            })?,
            "native {name} reminder did not paint the expected color"
        );
        capture(
            handle,
            &output.join(format!("{label}-messages-{name}.png")),
            cx,
        )?;
    }
    let messages = button_bounds(handle, "host-messages-toggle", cx)?.center();
    dispatch(handle, down(messages), cx)?;
    dispatch(handle, up(messages), cx)?;
    frame(handle, cx).await?;
    ensure!(
        handle.update(cx, |_, _, cx| app
            .read(cx)
            .messages
            .read(cx)
            .pending_level()
            .is_none())?,
        "opening did not acknowledge the colored reminder"
    );
    let clear = button_bounds(handle, "clear-host-messages", cx)?.center();
    dispatch(handle, down(clear), cx)?;
    dispatch(handle, up(clear), cx)?;
    frame(handle, cx).await?;
    dispatch(handle, down(messages), cx)?;
    dispatch(handle, up(messages), cx)?;
    frame(handle, cx).await?;
    ensure!(
        !messages_visible(handle, app, cx)?,
        "reminder scenario left messages open"
    );
    navigation(handle, app, output, label, cx).await?;
    Ok(())
}

/// Real font/D3D readback verifies the compact preview and the older target revealed by native card input.
async fn navigation(
    handle: AnyWindowHandle,
    app: &Entity<EditorApp>,
    output: &Path,
    label: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    // Reminder checks leave automatic cards alive; remove only this owned window's cards first.
    while let Ok(close) = button_bounds(handle, "dismiss-local-notification", cx) {
        dispatch(handle, down(close.center()), cx)?;
        dispatch(handle, up(close.center()), cx)?;
        frame(handle, cx).await?;
    }
    handle.update(cx, |_, _, cx| {
        app.update(cx, |app, cx| {
            app.record_host_message(
                MessageLevel::Error,
                "目标消息 / Target receipt\n第二行预览 / Second preview line\n完整详情第三行 / Full third line\n完整详情第四行 / Full fourth line",
                cx,
            );
            for index in 0..45 {
                app.record_host_message(MessageLevel::Info, format!("Later receipt {index}"), cx);
            }
        });
    })?;
    frame(handle, cx).await?;
    capture(
        handle,
        &output.join(format!("{label}-message-preview.png")),
        cx,
    )?;
    let close = button_bounds(handle, "dismiss-local-notification", cx)?;
    let body = point(close.left() - px(100.), close.center().y + px(18.));
    dispatch(handle, down(body), cx)?;
    dispatch(handle, up(body), cx)?;
    frame(handle, cx).await?;
    frame(handle, cx).await?;
    ensure!(
        messages_visible(handle, app, cx)?,
        "native card click did not open history"
    );
    dispatch(handle, down(close.center()), cx)?;
    dispatch(handle, up(close.center()), cx)?;
    frame(handle, cx).await?;
    ensure!(
        messages_visible(handle, app, cx)?,
        "closing the card toggled history"
    );
    capture(
        handle,
        &output.join(format!("{label}-message-revealed.png")),
        cx,
    )?;
    let clear = button_bounds(handle, "clear-host-messages", cx)?.center();
    dispatch(handle, down(clear), cx)?;
    dispatch(handle, up(clear), cx)?;
    frame(handle, cx).await?;
    let toggle = button_bounds(handle, "host-messages-toggle", cx)?.center();
    dispatch(handle, down(toggle), cx)?;
    dispatch(handle, up(toggle), cx)?;
    frame(handle, cx).await?;
    Ok(())
}
