//! Native pointer events open each public history menu route before the shared recovery scenario.

use super::*;

/// Application, explorer and tab menus must expose reachable history actions in both locales.
pub(super) async fn exercise(h: &Native, path: &Path, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.click("application-menu-trigger", cx).await?;
    select_history(h, cx).await?;
    h.click("history-close", cx).await?;
    let filename = path.file_name().unwrap().to_string_lossy();
    for role in [Role::TreeItem, Role::Tab] {
        h.frame(cx).await?;
        h.window.update(cx, |_, window, cx| {
            let node = snapshots(window)
                .into_iter()
                .find(|node| {
                    node.visible()
                        && node.role() == Some(role)
                        && node
                            .label()
                            .is_some_and(|label| label.contains(filename.as_ref()))
                })
                .with_context(|| format!("native context target {role:?}/{filename} missing"))?;
            let position = node.bounds().center();
            window.activate_window();
            window.dispatch_event(
                gpui_kit::PlatformInput::MouseDown(gpui_kit::MouseDownEvent {
                    position,
                    button: gpui_kit::MouseButton::Right,
                    modifiers: Default::default(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
            window.dispatch_event(
                gpui_kit::PlatformInput::MouseUp(gpui_kit::MouseUpEvent {
                    position,
                    button: gpui_kit::MouseButton::Right,
                    modifiers: Default::default(),
                    click_count: 1,
                }),
                cx,
            );
            Ok::<_, anyhow::Error>(())
        })??;
        select_history(h, cx).await?;
        h.click("history-close", cx).await?;
    }
    h.evidence.borrow_mut()["observations"]["native_history_entrypoints"] =
        serde_json::json!(["application-menu", "explorer-context", "tab-context"]);
    Ok(())
}

/// Select the actually painted localized row, not a direct call to the history controller.
async fn select_history(h: &Native, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.frame(cx).await?;
    h.window.update(cx, |_, window, cx| {
        let title = t!("history.title");
        let node = snapshots(window)
            .into_iter()
            .find(|node| node.visible() && node.label() == Some(title.as_ref()))
            .context("native history menu action missing")?;
        input::click(window, &node, cx)
    })??;
    h.until(
        "native history menu route",
        |app, _| app.history.panel.is_some(),
        cx,
    )
    .await?;
    Ok(())
}
