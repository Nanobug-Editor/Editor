//! Settings acceptance dispatches the real native controls and reads only the saved private profile.
use super::*;

/// User choices persist, affect painted titles/colors and survive another provider joining the window.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    click_control(h, h.main, "open-settings", cx).await?;
    let dialog: AnyWindowHandle = cx
        .update(|cx| {
            h.app
                .read(cx)
                .dialog_window
                .context("settings dialog did not open")
        })?
        .into();
    h.dpi(dialog, cx).await?;
    scroll_to(h, dialog, "presentation-filename-colors", cx).await?;
    click_control(h, dialog, "presentation-filename-colors", cx).await?;
    h.until(
        h.main,
        "filename colors disabled",
        |_, cx| {
            Ok(h.app
                .read(cx)
                .explorer_file_decoration(
                    &h.fixture
                        .workspace
                        .canonicalize()?
                        .join(fixture::REVIEW_PATH),
                    false,
                    cx,
                )
                .is_none())
        },
        cx,
    )
    .await?;
    click_control(h, dialog, "presentation-filename-colors", cx).await?;
    scroll_to(h, dialog, "presentation-provider-review-queue", cx).await?;
    click_control(h, dialog, "presentation-provider-review-queue", cx).await?;
    h.until(
        h.main,
        "explicit review provider has no Git fallback",
        |_, cx| {
            Ok(h.app
                .read(cx)
                .explorer_file_decoration(
                    &h.fixture
                        .workspace
                        .canonicalize()?
                        .join(fixture::REVIEW_PATH),
                    false,
                    cx,
                )
                .is_none())
        },
        cx,
    )
    .await?;
    click_control(h, dialog, "presentation-provider-git", cx).await?;
    colors(h, dialog, cx).await?;
    scroll_to(h, dialog, "presentation-visible-git/branch", cx).await?;
    click_control(h, dialog, "presentation-visible-git/branch", cx).await?;
    h.until(
        h.main,
        "user hidden title",
        |window, _| Ok(control(window, "workspace-entry-git/branch").is_err()),
        cx,
    )
    .await?;
    click_control(h, dialog, "presentation-visible-git/branch", cx).await?;
    scroll_to(h, dialog, "presentation-up-review-queue/queue", cx).await?;
    click_control(h, dialog, "presentation-up-review-queue/queue", cx).await?;
    let saved = preferences(h)?;
    ensure!(
        saved["order"][0] == "review-queue/queue",
        "title reorder not persisted: {saved}"
    );
    ensure!(
        saved["providers"][h.fixture.workspace.canonicalize()?.to_str().unwrap()] == "git",
        "explicit provider not persisted"
    );
    h.record("native_settings", saved);
    h.capture(dialog, "presentation-settings.png", cx)?;
    // Close through the painted native chrome; the settings owner's close subscription must retire it.
    h.until(
        dialog,
        "settings close button",
        |window, cx| {
            let row = control(window, "app-dialog-close")?;
            input::click(window, &row, cx)?;
            Ok(true)
        },
        cx,
    )
    .await?;
    h.until(
        h.main,
        "settings dialog dismissed",
        |_, cx| Ok(h.app.read(cx).dialog_window.is_none()),
        cx,
    )
    .await
}

/// Native Unicode input reaches the existing Base hex input; no backing value is assigned by the test.
async fn colors(h: &Harness, dialog: AnyWindowHandle, cx: &mut AsyncApp) -> anyhow::Result<()> {
    scroll_to(h, dialog, "presentation-color-1", cx).await?;
    dialog.update(cx, |_, window, cx| {
        let field = snapshots(window)
            .into_iter()
            .find(|node| {
                node.visible()
                    && node.role() == Some(Role::TextInput)
                    && input::named(node, "presentation-color-1")
            })
            .context("hex color input")?;
        input::click(window, &field, cx)?;
        Ok::<_, anyhow::Error>(())
    })??;
    h.frame(dialog, cx).await?;
    dialog.update(cx, |_, window, cx| {
        editor_ui::testing::input_text(window, "#12AB34", cx)
    })?;
    h.until(
        h.main,
        "custom theme filename color",
        |_, cx| {
            Ok(h.app
                .read(cx)
                .explorer_file_decoration(
                    &h.fixture
                        .workspace
                        .canonicalize()?
                        .join(fixture::REVIEW_PATH),
                    false,
                    cx,
                )
                .is_some_and(|(color, _)| color == gpui_kit::rgb(0x12ab34).into()))
        },
        cx,
    )
    .await?;
    ensure!(
        preferences(h)?[if h.case.dark { "dark" } else { "light" }][1] == 0x12ab34,
        "theme color not saved"
    );
    Ok(())
}

/// Scroll the actual settings viewport until its current public native target is visible.
async fn scroll_to(
    h: &Harness,
    dialog: AnyWindowHandle,
    id: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    h.until(
        dialog,
        id,
        |window, cx| {
            if snapshots(window).iter().any(|node| {
                node.visible()
                    && input::named(node, id)
                    && Bounds::new(point(px(0.), px(0.)), window.viewport_size())
                        .contains(&node.bounds().center())
            }) {
                return Ok(true);
            }
            let position = point(
                window.viewport_size().width - px(80.),
                window.viewport_size().height / 2.,
            );
            input::wheel(window, position, -180., cx);
            Ok(false)
        },
        cx,
    )
    .await
}
/// Read back the isolated profile written by production settings, never the user's real profile.
fn preferences(h: &Harness) -> anyhow::Result<serde_json::Value> {
    Ok(serde_json::from_slice(&fs::read(
        h.fixture.profile.join("workspace-presentation.json"),
    )?)?)
}
