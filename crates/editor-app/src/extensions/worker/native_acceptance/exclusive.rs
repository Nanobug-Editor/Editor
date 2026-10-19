//! Actual Git packages exercise the generic exclusive property on Windows/D3D with isolated profiles.
use super::*;
use gpui_base::dock::PanelId;
use gpui_base::test_support::{ElementSnapshot, snapshots};
use gpui_kit::{ElementId, Role};

mod edge_drop;

/// This workflow requires a test candidate declaring staging exclusive; user installations stay untouched.
pub(super) fn requested() -> bool {
    std::env::var_os("NANOBUG_GIT_NATIVE_EXCLUSIVE").is_some()
}

/// Two actual native processes cover dark Chinese at 100% and light English at 150%.
#[test]
#[ignore = "requires Windows/D3D11, exclusive Git candidate/output and NANOBUG_GIT_NATIVE_EXCLUSIVE"]
fn git_native_exclusive_dock_replaces_and_restores_peers() {
    assert!(requested());
    for case in [
        Case::SMOKE,
        Case::ALL
            .into_iter()
            .find(|case| case.name == "light-en-150")
            .unwrap(),
    ] {
        matrix::run(case).unwrap();
    }
}

/// Find an actually painted host control by its exact native path name.
fn named<'a>(nodes: &'a [ElementSnapshot], name: &str) -> Option<&'a ElementSnapshot> {
    nodes.iter().find(|node| {
        node.visible()
            && node
                .path()
                .iter()
                .any(|part| matches!(part, ElementId::Name(id) if id.as_ref() == name))
    })
}

/// Check display, unchanged host intent and real staging clip geometry for the requested mode.
fn check(h: &Harness, exclusive: bool, window: &Window, cx: &App) -> anyhow::Result<bool> {
    let app = h.app.read(cx);
    let area = app.dock_area.read(cx);
    ensure!(
        app.explorer_visible && app.session_state.outline_visible,
        "replacement overwrote host visibility intent"
    );
    for id in [
        PanelId::from(app.explorer_panel.entity_id()),
        PanelId::from(app.outline_panel.entity_id()),
    ] {
        if area.panel(id).context("host leaf removed")?.visible(cx) == exclusive {
            return Ok(false);
        }
    }
    let nodes = snapshots(window);
    for id in ["explorer-hide", "outline-hide"] {
        if named(&nodes, id).is_some() == exclusive {
            return Ok(false);
        }
    }
    let stage = input::field(window, "commit-message");
    if !exclusive {
        return Ok(stage.is_err());
    }
    let Ok(stage) = stage else { return Ok(false) };
    let Some(frame) = nodes.iter().filter(|node| node.visible()
        && matches!(node.path().last(), Some(ElementId::Name(id)) if id.as_ref() == "local-dock-content-frame")
        && stage.path().starts_with(node.path())).max_by_key(|node| node.path().len()) else { return Ok(false) };
    let clip = frame.bounds();
    ensure!(
        clip.size.height > px(350.),
        "exclusive panel did not fill the sidebar: {clip:?}"
    );
    let tree = input::tree(window, "staging")?;
    ensure!(
        tree.role() == Some(Role::Tree) && tree.bounds().size.height >= px(28.),
        "staging tree clipped"
    );
    for bounds in [
        stage.bounds(),
        tree.bounds(),
        input::button(window, "commit")?.bounds(),
    ] {
        ensure!(
            bounds.left() >= clip.left() - px(2.)
                && bounds.right() <= clip.right() + px(2.)
                && bounds.top() >= clip.top() - px(2.)
                && bounds.bottom() <= clip.bottom() + px(2.),
            "exclusive control outside native clip: {bounds:?}, clip={clip:?}"
        );
    }
    Ok(true)
}

/// Read the actual sidebar allocation around a painted control, rather than a saved size preference.
fn left_frame(window: &Window, exclusive: bool) -> anyhow::Result<Bounds<Pixels>> {
    let nodes = snapshots(window);
    let anchor = if exclusive {
        input::field(window, "commit-message")?
    } else {
        named(&nodes, "explorer-hide")
            .context("Explorer title missing")?
            .clone()
    };
    nodes.iter().filter(|node| node.visible()
        && matches!(node.path().last(), Some(ElementId::Name(id)) if id.as_ref() == "tab-group")
        && anchor.path().starts_with(node.path()))
        .max_by_key(|node| node.path().len())
        .map(|node| node.bounds())
        .context("native sidebar frame missing")
}

/// Production footer clicks open/close the right dock without changing shared or exclusive left pixels.
async fn check_right_width(h: &Harness, exclusive: bool, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let before = h.main.update(cx, |_, window, cx| {
        ensure!(
            !h.app.read(cx).messages_visible(cx),
            "right width baseline must be closed"
        );
        left_frame(window, exclusive)
    })??;
    let mode = if exclusive { "exclusive" } else { "shared" };
    for open in [true, false] {
        sidebar::toggle(h, "host-messages-toggle", cx).await?;
        h.settle(cx).await?;
        h.until(
            h.main,
            "right visibility preserves left pixels",
            |window, cx| {
                if h.app.read(cx).messages_visible(cx) != open {
                    return Ok(false);
                }
                let after = left_frame(window, exclusive)?;
                ensure!(
                    (after.left() - before.left()).abs() < px(2.)
                        && (after.size.width - before.size.width).abs() < px(2.)
                        && (after.size.height - before.size.height).abs() < px(2.),
                    "right open={open} changed left: {before:?} -> {after:?}"
                );
                Ok(true)
            },
            cx,
        )
        .await?;
        h.capture(
            h.main,
            &format!(
                "right-width-{mode}-{}.png",
                if open { "open" } else { "closed" }
            ),
            cx,
        )?;
    }
    h.record(
        &format!("right_width_{mode}"),
        serde_json::json!({"left_frame":format!("{before:?}"),"open_close_preserves_left":true}),
    );
    Ok(())
}

/// Open, switch to the ordinary combination, reopen and close through production client pointer dispatch.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.activate(cx).await?;
    h.main.update(cx, |_, _, cx| {
        let app = h.app.read(cx);
        ensure!(
            app.extensions
                .read(cx)
                .entries
                .iter()
                .any(|entry| entry.manifest.id == "git"
                    && entry
                        .manifest
                        .panels
                        .iter()
                        .any(|panel| panel.id == "staging" && panel.exclusive)),
            "candidate must declare exclusive staging through the public manifest"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    h.until(
        h.main,
        "exclusive staging replaces both host peers",
        |window, cx| check(h, true, window, cx),
        cx,
    )
    .await?;
    h.capture(h.main, "exclusive-open.png", cx)?;
    check_right_width(h, true, cx).await?;
    sidebar::toggle(h, "explorer-panel-toggle", cx).await?;
    h.until(
        h.main,
        "ordinary Explorer and Outline combination restored",
        |window, cx| check(h, false, window, cx),
        cx,
    )
    .await?;
    h.capture(h.main, "exclusive-shared.png", cx)?;
    check_right_width(h, false, cx).await?;
    edge_drop::exercise(h, cx).await?;
    sidebar::toggle(h, "plugin-window-git/staging", cx).await?;
    h.settle(cx).await?;
    h.until(
        h.main,
        "exclusive staging reopened",
        |window, cx| check(h, true, window, cx),
        cx,
    )
    .await?;
    h.capture(h.main, "exclusive-reopened.png", cx)?;
    sidebar::toggle(h, "plugin-window-git/staging", cx).await?;
    h.until(
        h.main,
        "closing exclusive restores both host peers",
        |window, cx| check(h, false, window, cx),
        cx,
    )
    .await?;
    edge_drop::nested::exercise(h, cx).await?;
    edge_drop::grouped::exercise(h, cx).await?;
    h.record("exclusive_complete", true.into());
    Ok(())
}
