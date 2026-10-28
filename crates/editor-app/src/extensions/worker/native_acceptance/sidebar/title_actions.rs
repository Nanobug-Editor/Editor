//! Native focus/hover reveal on the actual Explorer and installed Git panel chrome.

use super::*;
use gpui_kit::MouseMoveEvent;

/// A dedicated receipt cannot substitute for the full sidebar workflow's staging/drag assertions.
pub(in super::super) fn requested() -> bool {
    std::env::var_os("NANOBUG_GIT_NATIVE_TITLE_ACTIONS").is_some()
}

/// Exercise only title controls in two real renderer/theme/DPI cases, with bounded child cleanup.
#[test]
#[ignore = "requires Windows/D3D11, Git candidate/output and NANOBUG_GIT_NATIVE_TITLE_ACTIONS"]
fn native_panel_title_actions_follow_focus_and_hover() {
    assert!(requested());
    assert!(std::env::var_os("NANOBUG_GIT_NATIVE_SHARED_SIDEBAR").is_none());
    let _cursor = CursorRestore::capture().unwrap();
    let scaled = Case::ALL
        .into_iter()
        .find(|case| case.name == "light-en-150")
        .unwrap();
    for case in [Case::SMOKE, scaled] {
        matrix::run(case).unwrap();
        eprintln!("native title actions case passed: {}", case.name);
    }
}

/// Find the real painted button with `id`, optionally scoped to an ancestor's `path`.
fn button(window: &Window, id: &str, path: Option<&[ElementId]>) -> Option<ElementSnapshot> {
    snapshots(window).into_iter().find(|node| {
        node.visible()
            && node.role() == Some(Role::Button)
            && input::named(node, id)
            && path.is_none_or(|path| node.path().starts_with(path))
    })
}

/// Dispatch a plain pointer move through the real native window, without a click or focus change.
fn hover(window: &mut Window, position: Point<Pixels>, cx: &mut App) {
    window.dispatch_event(
        PlatformInput::MouseMove(MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers: Default::default(),
        }),
        cx,
    );
}

/// Native pixels and input prove that both title groups share pointer-or-focus behavior.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let (explorer, plugin, path, outside) = h.main.update(cx, |_, window, cx| {
        let title = geometry::explorer_title(window)?;
        let explorer = geometry::frame(window, &title)?.content.center();
        let field = input::field(window, "commit-message")?;
        let plugin = geometry::frame(window, &field)?.content.center();
        let group = snapshots(window).into_iter().filter(|node| {
            field.path().starts_with(node.path())
                && matches!(node.path().last(), Some(ElementId::Name(name)) if name.as_ref() == "tab-group")
        }).max_by_key(|node| node.path().len()).context("plugin group missing")?;
        let outside = point(window.viewport_size().width / 2., px(80.));
        h.app.read(cx).editor.focus_handle(cx).focus(window, cx);
        hover(window, outside, cx);
        Ok::<_, anyhow::Error>((explorer, plugin, group.path().to_vec(), outside))
    })??;
    h.until(
        h.main,
        "unfocused title actions hidden",
        |window, _| {
            Ok(
                button(window, "explorer-collapse-all-button", None).is_none()
                    && button(window, "plugin-hide", Some(&path)).is_none(),
            )
        },
        cx,
    )
    .await?;
    for (name, position, id, scope) in [
        ("explorer", explorer, "explorer-collapse-all-button", None),
        ("plugin", plugin, "plugin-hide", Some(path.as_slice())),
    ] {
        h.main
            .update(cx, |_, window, cx| hover(window, position, cx))?;
        h.until(
            h.main,
            "hover reveals title actions",
            |window, cx| {
                ensure!(
                    h.app.read(cx).editor.focus_handle(cx).is_focused(window),
                    "hover stole editor focus"
                );
                Ok(button(window, id, scope).is_some())
            },
            cx,
        )
        .await?;
        // Capture fully revealed pixels instead of the first nearly transparent animation frame.
        cx.background_executor()
            .timer(Duration::from_millis(160))
            .await;
        h.capture(h.main, &format!("title-actions-{name}-hover.png"), cx)?;
        h.main.update(cx, |_, window, cx| {
            let app = h.app.read(cx);
            let focus = if name == "explorer" {
                app.explorer_panel.read(cx).focus_handle(cx)
            } else {
                app.plugin_panels["git/staging"].read(cx).focus_handle(cx)
            };
            focus.focus(window, cx);
            hover(window, outside, cx);
        })?;
        cx.background_executor()
            .timer(Duration::from_millis(160))
            .await;
        h.until(
            h.main,
            "focus keeps title actions after pointer leaves",
            |window, _| Ok(button(window, id, scope).is_some()),
            cx,
        )
        .await?;
        h.main.update(cx, |_, window, cx| {
            h.app.read(cx).editor.focus_handle(cx).focus(window, cx);
        })?;
        h.until(
            h.main,
            "leaving pointer and focus hides title actions",
            |window, _| Ok(button(window, id, scope).is_none()),
            cx,
        )
        .await?;
    }
    h.record(
        "title_actions_hover_and_focus",
        serde_json::json!({
            "explorer": true, "actual_plugin": "git/staging", "hover_preserves_editor_focus": true,
            "focus_retains_after_pointer_exit": true, "hidden_after_both_leave": true,
        }),
    );
    Ok(())
}
