//! Actual Git package declarations and the native Terminal exercise whole-region replacement on D3D.
use super::*;
use gpui_kit::Point;

/// A distinct value reuses the existing exclusive diagnostic without accepting the staging-only receipt.
pub(in super::super) fn requested() -> bool {
    std::env::var("NANOBUG_GIT_NATIVE_EXCLUSIVE").is_ok_and(|mode| mode == "replacement")
}

/// Both palettes and a fractional native scale observe full masks, replacement, cancellation and restoration.
#[test]
#[ignore = "requires Windows/D3D11, Git candidate/output and NANOBUG_GIT_NATIVE_EXCLUSIVE=replacement"]
fn git_native_exclusive_full_replacement() {
    assert!(requested());
    for case in [Case::SMOKE, Case::ALL[7]] {
        matrix::run(case).unwrap();
    }
}

/// Resolve the real outer slot surrounding a painted native control, including its title.
fn around(window: &Window, anchor: &ElementSnapshot) -> anyhow::Result<Bounds<Pixels>> {
    snapshots(window)
        .into_iter()
        .filter(|node| {
            node.visible()
                && matches!(node.path().last(), Some(ElementId::Name(id)) if id.as_ref() == "tab-group")
                && anchor.path().starts_with(node.path())
        })
        .max_by_key(|node| node.path().len())
        .map(|node| node.bounds())
        .context("painted dock slot missing")
}

/// Host controls retain stable identities even when their owning panel moves between native regions.
fn host(window: &Window, id: &str) -> anyhow::Result<Bounds<Pixels>> {
    around(
        window,
        named(&snapshots(window), id).context("host title not painted")?,
    )
}

/// Compare actual D3D logical allocations with a small rounding allowance at 150% scale.
fn same(actual: Bounds<Pixels>, expected: Bounds<Pixels>) -> bool {
    (actual.left() - expected.left()).abs() < px(2.)
        && (actual.top() - expected.top()).abs() < px(2.)
        && (actual.right() - expected.right()).abs() < px(2.)
        && (actual.bottom() - expected.bottom()).abs() < px(2.)
}

/// Gaps between ordinary side peers belong to the same whole-side preview.
fn union(a: Bounds<Pixels>, b: Bounds<Pixels>) -> Bounds<Pixels> {
    let origin = point(a.left().min(b.left()), a.top().min(b.top()));
    Bounds::new(
        origin,
        size(
            a.right().max(b.right()) - origin.x,
            a.bottom().max(b.bottom()) - origin.y,
        ),
    )
}

/// Dispatch the real hover and draw in one turn, then compare the complete blue mask with the target.
fn preview(
    h: &Harness,
    pointer: Point<Pixels>,
    expected: Bounds<Pixels>,
    image: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    edge_drop::sample_drag(h, pointer, image, cx)?;
    h.main.update(cx, |_, window, _| {
        let nodes = snapshots(window);
        let mask = named(&nodes, "local-dock-drop-indicator").context("replacement mask missing")?.bounds();
        ensure!(same(mask, expected), "whole-side mask differs: {mask:?} != {expected:?}");
        h.record(image, serde_json::json!({"mask": format!("{mask:?}"), "whole_target": format!("{expected:?}")}));
        Ok::<_, anyhow::Error>(())
    })??;
    h.capture(h.main, image, cx)
}

/// Only release commits the placement; it uses the same production listener as an ordinary pointer drop.
fn release(h: &Harness, pointer: Point<Pixels>, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.main.update(cx, |_, window, cx| {
        ensure!(
            cx.has_active_drag(),
            "replacement gesture ended before release"
        );
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position: pointer,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
            }),
            cx,
        );
        Ok::<_, anyhow::Error>(())
    })??;
    Ok(())
}

/// No DockArea edits or manifest substitutions stand in for user gestures in this workflow.
pub(super) async fn exercise(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    h.activate(cx).await?;
    let [explorer, outline, terminal, history, staging] = h.main.update(cx, |_, _, cx| {
        let app = h.app.read(cx);
        let owner = app.extensions.read(cx);
        let manifest = &owner
            .entries
            .iter()
            .find(|entry| entry.manifest.id == "git")
            .context("Git missing")?
            .manifest;
        ensure!(
            manifest
                .panels
                .iter()
                .any(|panel| panel.id == "history" && panel.exclusive)
        );
        ensure!(
            manifest
                .panels
                .iter()
                .any(|panel| panel.id == "staging" && !panel.exclusive)
        );
        Ok::<_, anyhow::Error>([
            PanelId::from(app.explorer_panel.entity_id()),
            PanelId::from(app.outline_panel.entity_id()),
            PanelId::from(app.terminal.entity_id()),
            PanelId::from(app.plugin_panels["git/history"].entity_id()),
            PanelId::from(app.plugin_panels["git/staging"].entity_id()),
        ])
    })??;
    h.capture(h.main, "replacement-defaults.png", cx)?;
    let (source, target) = h.main.update(cx, |_, window, _| {
        Ok::<_, anyhow::Error>((
            host(window, "explorer-title")?,
            around(window, &input::tree(window, "branches")?)?,
        ))
    })??;
    edge_drop::begin_drag(h, source, cx).await?;
    preview(
        h,
        target.center(),
        target,
        "replacement-ordinary-preview.png",
        cx,
    )?;
    h.main.update(cx, |_, window, _| {
        ensure!(same(host(window, "explorer-title")?, source));
        ensure!(same(
            around(window, &input::tree(window, "branches")?)?,
            target
        ));
        Ok::<_, anyhow::Error>(())
    })??;
    release(h, target.center(), cx)?;
    h.until(
        h.main,
        "ordinary window fully replaces exclusive Git",
        |window, cx| {
            let app = h.app.read(cx);
            let area = app.dock_area.read(cx);
            if area.panel(history).context("Git unregistered")?.visible(cx) {
                return Ok(false);
            }
            ensure!(
                app.plugin_panels["git/history"].read(cx).visible.get(),
                "replacement closed Git"
            );
            ensure!(
                area.panel(explorer)
                    .context("Explorer missing")?
                    .visible(cx)
            );
            ensure!(same(host(window, "explorer-title")?, target));
            Ok(true)
        },
        cx,
    )
    .await?;
    h.capture(h.main, "replacement-ordinary-dropped.png", cx)?;

    sidebar::toggle(h, "terminal-toggle", cx).await?;
    h.until(
        h.main,
        "native Terminal takes the complete bottom side",
        |window, cx| {
            let app = h.app.read(cx);
            let area = app.dock_area.read(cx);
            if !area
                .panel(terminal)
                .context("Terminal missing")?
                .visible(cx)
            {
                return Ok(false);
            }
            ensure!(!area.panel(explorer).context("Explorer lost")?.visible(cx));
            ensure!(!area.panel(history).context("Git lost")?.visible(cx));
            ensure!(same(host(window, "native-terminal-hide")?, target));
            Ok(true)
        },
        cx,
    )
    .await?;
    let (source, target) = h.main.update(cx, |_, window, _| {
        let stage = around(window, &input::field(window, "commit-message")?)?;
        Ok::<_, anyhow::Error>((
            host(window, "native-terminal-hide")?,
            union(stage, host(window, "outline-hide")?),
        ))
    })??;
    let pointer = point(target.center().x, target.bottom() - px(8.));
    edge_drop::begin_drag(h, source, cx).await?;
    preview(
        h,
        pointer,
        target,
        "replacement-terminal-cancel-preview.png",
        cx,
    )?;
    h.main.update(cx, |_, window, cx| {
        editor_ui::testing::press_key(window, "escape", cx)
    })?;
    h.frame(h.main, cx).await?;
    h.main.update(cx, |_, window, cx| {
        ensure!(!cx.has_active_drag());
        ensure!(same(host(window, "native-terminal-hide")?, source));
        ensure!(same(
            union(
                around(window, &input::field(window, "commit-message")?)?,
                host(window, "outline-hide")?
            ),
            target
        ));
        Ok::<_, anyhow::Error>(())
    })??;
    edge_drop::begin_drag(h, source, cx).await?;
    preview(h, pointer, target, "replacement-terminal-preview.png", cx)?;
    release(h, pointer, cx)?;
    h.until(
        h.main,
        "exclusive Terminal replaces the whole ordinary group",
        |window, cx| {
            let app = h.app.read(cx);
            let area = app.dock_area.read(cx);
            if area
                .panel(outline)
                .context("Outline unregistered")?
                .visible(cx)
            {
                return Ok(false);
            }
            ensure!(
                !area
                    .panel(staging)
                    .context("commit panel unregistered")?
                    .visible(cx)
            );
            ensure!(
                area.panel(explorer)
                    .context("source peer unregistered")?
                    .visible(cx)
            );
            ensure!(
                app.session_state.outline_visible
                    && app.plugin_panels["git/staging"].read(cx).visible.get()
            );
            ensure!(same(host(window, "native-terminal-hide")?, target));
            Ok(true)
        },
        cx,
    )
    .await?;
    h.capture(h.main, "replacement-terminal-dropped.png", cx)?;
    sidebar::toggle(h, "outline-toggle", cx).await?;
    h.until(
        h.main,
        "ordinary left group restores retained peers",
        |window, cx| {
            let app = h.app.read(cx);
            let area = app.dock_area.read(cx);
            Ok(!area
                .panel(terminal)
                .context("Terminal unregistered")?
                .visible(cx)
                && area.panel(outline).context("Outline lost")?.visible(cx)
                && area
                    .panel(staging)
                    .context("commit panel lost")?
                    .visible(cx)
                && input::field(window, "commit-message").is_ok())
        },
        cx,
    )
    .await?;
    sidebar::toggle(h, "plugin-window-git/history", cx).await?;
    h.settle(cx).await?;
    h.capture(h.main, "replacement-restored.png", cx)?;
    h.record("exclusive_full_replacement_complete", true.into());
    Ok(())
}
