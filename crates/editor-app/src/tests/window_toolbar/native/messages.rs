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
    timestamped_stderr(handle, app, output, label, cx).await?;
    Ok(())
}

/// Replay timestamped stderr on `handle`, capture one card under `output`/`label`, then dismiss by input.
/// `app` keeps the original runtime history while `cx` drives the actual Windows message loop.
async fn timestamped_stderr(
    handle: AnyWindowHandle,
    app: &Entity<EditorApp>,
    output: &Path,
    label: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    // Earlier palette checks leave warning/error cards; close them through their actual native controls.
    while handle.update(cx, |_, _, cx| {
        app.read(cx).message_delivery.notifications(cx).count() > 0
    })? {
        let close = button_bounds(handle, "dismiss-local-notification", cx)?.center();
        dispatch(handle, down(close), cx)?;
        dispatch(handle, up(close), cx)?;
        frame(handle, cx).await?;
    }
    let logs = handle.update(cx, |_, _, cx| {
        app.read(cx).extensions.read(cx).runtime_logs()
    })?;
    for index in 0..3 {
        logs.append(
            "language-plugin",
            plugin_runtime::LogLevel::Warning,
            "lsp/analysis/stderr",
            format!(
                "2026-10-30T21:43:0{index}.4372163+08:00  WARN notify error: Input watch path is neither a file nor a directory."
            ),
        );
        handle.update(cx, |_, _, cx| {
            app.read(cx)
                .extensions
                .clone()
                .update(cx, |_, cx| cx.notify());
        })?;
        frame(handle, cx).await?;
    }
    ensure!(
        handle.update(cx, |_, _, cx| app
            .read(cx)
            .message_delivery
            .notifications(cx)
            .count())?
            == 1,
        "timestamped stderr created duplicate native cards"
    );
    ensure!(
        logs.records("language-plugin").len() == 3,
        "stderr history was lost"
    );
    let close = button_bounds(handle, "dismiss-local-notification", cx)?.center();
    capture(
        handle,
        &output.join(format!("{label}-stderr-coalesced.png")),
        cx,
    )?;
    dispatch(handle, down(close), cx)?;
    dispatch(handle, up(close), cx)?;
    frame(handle, cx).await?;
    ensure!(
        handle.update(cx, |_, _, cx| app
            .read(cx)
            .message_delivery
            .notifications(cx)
            .count())?
            == 0,
        "one native dismissal must close the repeated warning"
    );
    Ok(())
}
