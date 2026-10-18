//! Regional split policy reads Base's live layout; it stores neither a second tree nor a remembered dock direction.
use super::*;
use gpui_base::dock::{PaneNode, PaneRef};
use gpui_kit::{Axis, Point};

/// The project uses Base's documented middle merge zone while keeping every panel in an independent leaf.
/// Live pointer geometry also covers a release before another frame has refreshed Base's indicator.
pub(super) fn edge_at(bounds: Bounds<Pixels>, pointer: Point<Pixels>) -> Option<Placement> {
    if !bounds.contains(&pointer) {
        return None;
    }
    if pointer.x < bounds.left() + bounds.size.width * 0.35 {
        Some(Placement::Left)
    } else if pointer.x > bounds.left() + bounds.size.width * 0.65 {
        Some(Placement::Right)
    } else if pointer.y < bounds.top() + bounds.size.height * 0.35 {
        Some(Placement::Top)
    } else if pointer.y > bounds.top() + bounds.size.height * 0.65 {
        Some(Placement::Bottom)
    } else {
        None
    }
}

/// Side bands stack up/down; top/bottom bands stack left/right. The editor itself accepts all four edges.
pub(super) fn resolve(
    area: &DockArea,
    node: NodeId,
    requested: Placement,
    bounds: Bounds<Pixels>,
    pointer: Point<Pixels>,
    cx: &App,
) -> Placement {
    match stack_axis(area, node, cx) {
        Some(Axis::Vertical) => match requested {
            Placement::Top | Placement::Bottom => requested,
            _ if pointer.y <= bounds.center().y => Placement::Top,
            _ => Placement::Bottom,
        },
        Some(Axis::Horizontal) => match requested {
            Placement::Left | Placement::Right => requested,
            _ if pointer.x <= bounds.center().x => Placement::Left,
            _ => Placement::Right,
        },
        None => requested,
    }
}

/// Derive a center-tree band's orientation from its separation from the host editor anchor.
/// Base has no Top DockPlacement: a vertical split above Editor is the native, persisted top band.
pub(crate) fn stack_axis(area: &DockArea, node: NodeId, cx: &App) -> Option<Axis> {
    for region in [
        DockPlacement::Left,
        DockPlacement::Right,
        DockPlacement::Bottom,
    ] {
        if area
            .layout(region)
            .is_some_and(|tree| tree.find_node(node).is_some())
        {
            return Some(if region == DockPlacement::Bottom {
                Axis::Horizontal
            } else {
                Axis::Vertical
            });
        }
    }
    let tree = area.layout(DockPlacement::Center)?;
    let anchor = tree.panels().find(|id| {
        area.panel(*id)
            .is_some_and(|panel| panel.panel_name(cx) == "Editor")
    })?;
    band_axis(tree.root(), node, tree.find_panel_node(anchor)?)
}

/// Locate the entire existing band immediately left of Editor, independent of plugin IDs.
/// Hidden host panels still anchor their band; a moved Explorer never drags a new left panel rightward.
pub(super) fn left_sidebar_target(area: &DockArea, cx: &App) -> Option<(NodeId, Option<Pixels>)> {
    let tree = area.layout(DockPlacement::Center)?;
    let editor = tree.panels().find(|id| {
        area.panel(*id)
            .is_some_and(|panel| panel.panel_name(cx) == "Editor")
    })?;
    let band = left_band(tree.root(), tree.find_panel_node(editor)?)?;
    // Only inspect persisted nodes here. Visibility getters may read the owner that
    // is currently registering this panel; Base reconciles visibility after the edit.
    let height = match band.kind() {
        PaneRef::Split {
            axis: Axis::Vertical,
            sizes,
            ..
        } => sizes
            .iter()
            .try_fold(gpui_kit::px(0.), |total, size| {
                size.map(|size| total + size)
            })
            .filter(|height| *height > gpui_kit::px(0.) && height.as_f32().is_finite()),
        _ => None,
    };
    Some((band.id(), height))
}

/// Reordering direct peers uses the painted allocation as a preference, subject to Base constraints.
/// Saved resize preferences can exceed the clipped slot; they must not become a fixed pixel size.
pub(super) fn reorder_size(
    area: &DockArea,
    source: NodeId,
    target: NodeId,
    axis: Axis,
    frames: &HashMap<NodeId, Bounds<Pixels>>,
    cx: &App,
) -> Option<Pixels> {
    if stack_axis(area, target, cx) != Some(axis) {
        return None;
    }
    let peers = [
        DockPlacement::Center,
        DockPlacement::Left,
        DockPlacement::Right,
        DockPlacement::Bottom,
    ]
    .into_iter()
    .any(|region| {
        area.layout(region)
            .is_some_and(|tree| same_split(tree.root(), source, target, axis))
    });
    if !peers {
        return None;
    }
    let bounds = frames.get(&source)?;
    let size = if axis == Axis::Vertical {
        bounds.size.height
    } else {
        bounds.size.width
    };
    (size > gpui_kit::px(0.) && size.as_f32().is_finite()).then_some(size)
}

/// Only same-parent peers in a shared stacking band are reordered; other drops create a fresh split.
fn same_split(node: &PaneNode, source: NodeId, target: NodeId, axis: Axis) -> bool {
    let PaneRef::Split {
        axis: split_axis,
        children,
        ..
    } = node.kind()
    else {
        return false;
    };
    (split_axis == axis
        && children.iter().any(|child| child.id() == source)
        && children.iter().any(|child| child.id() == target))
        || children
            .iter()
            .any(|child| same_split(child, source, target, axis))
}

/// The closest horizontal ancestor supplies the actual left band, without storing a second layout.
fn left_band(root: &PaneNode, editor: NodeId) -> Option<&PaneNode> {
    let PaneRef::Split { axis, children, .. } = root.kind() else {
        return None;
    };
    let index = children.iter().position(|child| contains(child, editor))?;
    left_band(&children[index], editor)
        .or_else(|| (axis == Axis::Horizontal && index > 0).then(|| &children[index - 1]))
}

/// The nearest split separating target and Editor determines the band's natural stacking axis.
fn band_axis(root: &PaneNode, target: NodeId, anchor: NodeId) -> Option<Axis> {
    let PaneRef::Split { axis, children, .. } = root.kind() else {
        return None;
    };
    let target_child = children.iter().position(|child| contains(child, target))?;
    let anchor_child = children.iter().position(|child| contains(child, anchor))?;
    if target_child != anchor_child {
        Some(match axis {
            Axis::Horizontal => Axis::Vertical,
            Axis::Vertical => Axis::Horizontal,
        })
    } else {
        band_axis(&children[target_child], target, anchor)
    }
}

/// Inspect public immutable nodes; mutations always stay behind Base's normalized edit methods.
fn contains(root: &PaneNode, target: NodeId) -> bool {
    let mut found = false;
    root.walk(&mut |node| found |= node.id() == target);
    found
}
