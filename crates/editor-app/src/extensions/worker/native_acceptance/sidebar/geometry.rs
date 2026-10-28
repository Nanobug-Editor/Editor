//! Read current Base frame/clip geometry rather than inferring usable controls from serialized Dock leaves.
use super::*;
use gpui_base::dock::{DockPlacement, PanelId};

#[derive(Clone, Copy)]
/// The outer peer allocation and the separately clipped native content rectangle.
pub(super) struct Frame {
    pub outer: Bounds<Pixels>,
    pub content: Bounds<Pixels>,
}

/// Current independent peer allocations in the shared left band.
pub(super) struct Geometry {
    pub staging: Frame,
    pub explorer: Frame,
    pub outline: Frame,
}

/// Match the frame itself, excluding children whose path only includes that ancestor.
fn named(node: &ElementSnapshot, id: &str) -> bool {
    matches!(node.path().last(), Some(ElementId::Name(name)) if name.as_ref() == id)
}

/// Explorer's title stays painted when its focus-sensitive action buttons fade away.
pub(super) fn explorer_title(window: &Window) -> anyhow::Result<ElementSnapshot> {
    snapshots(window)
        .into_iter()
        .find(|node| node.visible() && named(node, "explorer-title"))
        .context("Explorer title missing")
}

/// Base observes each group and content frame independently around the project's local renderer.
pub(super) fn frame(window: &Window, anchor: &ElementSnapshot) -> anyhow::Result<Frame> {
    let nodes = snapshots(window);
    let outer = nodes
        .iter()
        .filter(|node| {
            node.visible() && named(node, "tab-group") && anchor.path().starts_with(node.path())
        })
        .max_by_key(|node| node.path().len())
        .context("native Base panel frame missing")?;
    let content = nodes
        .iter()
        .filter(|node| {
            node.visible()
                && named(node, "local-dock-content-frame")
                && node.path().starts_with(outer.path())
        })
        .min_by_key(|node| node.path().len())
        .context("native Base panel clip missing")?;
    ensure!(
        outer.bounds().size.width > px(0.) && content.bounds().size.height > px(0.),
        "panel is not visibly sized"
    );
    Ok(Frame {
        outer: outer.bounds(),
        content: content.bounds(),
    })
}

/// A painted field/button can still lie beyond an ancestor clip; both corners must fit.
fn inside(outer: Bounds<Pixels>, inner: Bounds<Pixels>) -> bool {
    inner.size.width > px(0.)
        && inner.size.height > px(0.)
        && inner.left() >= outer.left() - px(1.)
        && inner.right() <= outer.right() + px(1.)
        && inner.top() >= outer.top() - px(1.)
        && inner.bottom() <= outer.bottom() + px(1.)
}

/// One complete tree row plus the full commit form/actions must remain inside the real staging clip.
pub(super) fn staging_controls(
    h: &Harness,
    window: &Window,
    cx: &App,
) -> anyhow::Result<Bounds<Pixels>> {
    let field = input::field(window, "commit-message")?;
    let group = frame(window, &field)?;
    let tree = input::tree(window, "staging")?;
    ensure!(
        tree.bounds().size.height >= px(28.) && inside(group.content, tree.bounds()),
        "staging tree has no complete interactive row inside its clip: tree={:?} clip={:?}",
        tree.bounds(),
        group.content
    );
    ensure!(
        inside(group.content, field.bounds()) && field.disabled() != Some(true),
        "commit textarea is clipped or disabled: field={:?} clip={:?}",
        field.bounds(),
        group.content
    );
    for id in ["stage", "commit"] {
        let button = input::button(window, id)?;
        ensure!(
            inside(group.content, button.bounds())
                && button.disabled() != Some(true)
                && h.node_current(h.main, id, cx),
            "{id} is clipped, disabled or stale: button={:?} clip={:?}",
            button.bounds(),
            group.content
        );
    }
    Ok(group.content)
}

/// Ordinary new-user Explorer=true/Outline=false must leave a usable staging form in the same band.
pub(super) fn record_default(h: &Harness, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let facts = h.main.update(cx, |_, window, cx| {
        let app = h.app.read(cx);
        ensure!(app.explorer_visible && !app.session_state.outline_visible, "sidebar must start from the ordinary host visibility default");
        let area = app.dock_area.read(cx);
        let tree = area.layout(DockPlacement::Left).context("left layout missing")?;
        let explorer_node = tree.find_panel_node(PanelId::from(app.explorer_panel.entity_id())).context("Explorer is outside Left")?;
        let staging_node = tree.find_panel_node(PanelId::from(app.plugin_panels["git/staging"].entity_id())).context("staging is outside Left")?;
        ensure!(explorer_node != staging_node, "default peers merged into tabs");
        let staging = frame(window, &input::field(window, "commit-message")?)?;
        let explorer = frame(window, &explorer_title(window)?)?;
        ensure!((staging.outer.left() - explorer.outer.left()).abs() <= px(2.) && (staging.outer.size.width - explorer.outer.size.width).abs() <= px(2.), "default peers occupy multiple columns");
        ensure!(staging.outer.bottom() <= explorer.outer.top() + px(2.) || explorer.outer.bottom() <= staging.outer.top() + px(2.), "default peer frames overlap");
        let clip = staging_controls(h, window, cx)?;
        Ok::<_, anyhow::Error>(serde_json::json!({"explorer_visible": true, "outline_visible": false, "staging": format!("{:?}", staging.outer), "explorer": format!("{:?}", explorer.outer), "staging_clip": format!("{clip:?}"), "tree_and_commit_controls_inside_clip": true}))
    })??;
    h.checkpoint("sidebar_ordinary_default", facts)
}

/// Three different current Base leaves must occupy one vertical band, with the native Editor frame to its right.
pub(super) fn sample(h: &Harness, window: &Window, cx: &App) -> anyhow::Result<Geometry> {
    let app = h.app.read(cx);
    let area = app.dock_area.read(cx);
    let tree = area
        .layout(DockPlacement::Left)
        .context("left layout missing")?;
    let ids = [
        PanelId::from(app.explorer_panel.entity_id()),
        PanelId::from(app.outline_panel.entity_id()),
        PanelId::from(app.plugin_panels["git/staging"].entity_id()),
    ];
    let nodes = ids.map(|id| {
        tree.find_panel_node(id)
            .context("shared peer is outside Left")
    });
    let [explorer_node, outline_node, staging_node] = nodes;
    let (explorer_node, outline_node, staging_node) =
        (explorer_node?, outline_node?, staging_node?);
    ensure!(
        explorer_node != outline_node
            && explorer_node != staging_node
            && outline_node != staging_node,
        "sidebar peers merged into tabs"
    );
    ensure!(
        area.layout(DockPlacement::Center).is_some_and(|tree| tree
            .find_panel_node(PanelId::from(app.editor_panel.entity_id()))
            .is_some()),
        "editor left Center"
    );
    let staging = frame(window, &input::field(window, "commit-message")?)?;
    let explorer = frame(window, &explorer_title(window)?)?;
    let outline = frame(window, &toolbar_button(window, "outline-hide")?)?;
    let mut peers = [staging.outer, explorer.outer, outline.outer];
    for peer in peers {
        ensure!(
            (peer.left() - staging.outer.left()).abs() <= px(2.)
                && (peer.size.width - staging.outer.size.width).abs() <= px(2.),
            "sidebar occupies multiple columns: staging={:?} other={peer:?}",
            staging.outer
        );
    }
    peers.sort_by(|a, b| a.top().partial_cmp(&b.top()).unwrap());
    ensure!(
        peers
            .windows(2)
            .all(|pair| pair[0].bottom() <= pair[1].top() + px(2.)),
        "shared peer frames overlap"
    );
    let editor_on_right = snapshots(window).into_iter().any(|node| {
        let bounds = node.bounds();
        node.visible()
            && named(&node, "tab-group")
            && bounds.left() >= staging.outer.right() - px(2.)
            && bounds.size.width > px(32.)
            && bounds.top() <= peers[0].top() + px(2.)
            && bounds.bottom() >= peers[2].bottom() - px(2.)
    });
    ensure!(
        editor_on_right,
        "no native editor frame to the right of the shared sidebar"
    );
    staging_controls(h, window, cx)?;
    Ok(Geometry {
        staging,
        explorer,
        outline,
    })
}

/// Save actual frame/clip measurements before any next interaction; failures retain their real pixels separately.
pub(super) fn record(h: &Harness, label: &str, cx: &mut AsyncApp) -> anyhow::Result<Geometry> {
    let geometry = h.main.update(cx, |_, window, cx| sample(h, window, cx))??;
    let measured = |frame: Frame| serde_json::json!({"outer": format!("{:?}", frame.outer), "content_clip": format!("{:?}", frame.content)});
    h.checkpoint(label, serde_json::json!({"staging": measured(geometry.staging), "explorer": measured(geometry.explorer), "outline": measured(geometry.outline), "tree_and_commit_controls_inside_clip": true}))?;
    Ok(geometry)
}
