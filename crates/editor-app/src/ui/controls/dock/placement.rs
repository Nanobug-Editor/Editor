//! Regional split policy reads Base's live layout; it stores neither a second tree nor a remembered dock direction.
use super::*;
use gpui_base::dock::{PaneNode, PaneRef, PaneTree, PanelId};
use gpui_kit::{Axis, Point};

/// A borrowed regional row/column distinguishes its direct members from nested cell contents.
/// This is derived from Base's tree for each query and is never persisted as another layout.
pub(super) struct Band<'a> {
    /// The first-level regional container, or its only window before it has been split.
    root: &'a PaneNode,
    /// First-level side regions stack vertically; top/bottom regions stack horizontally.
    axis: Axis,
}

impl<'a> Band<'a> {
    /// Only a first-level member may carry its regional combination; nested children stay local.
    pub(super) fn drag_group(&self, node: NodeId) -> Option<&'a PaneNode> {
        self.direct_member(node).then_some(self.root)
    }

    /// A regional leaf is either the unsplit band or one direct child of its first-level row/column.
    fn direct_member(&self, node: NodeId) -> bool {
        self.root.id() == node
            || matches!(self.root.kind(), PaneRef::Split { axis, children, .. }
            if axis == self.axis && children.iter().any(|child| child.id() == node))
    }

    /// A split inside an existing cell is unrestricted; a new first-level fork uses the regional axis.
    /// Excluding `source` prevents moving the last regional peer from creating a forbidden root fork.
    fn drop_axis(
        &self,
        target: NodeId,
        source: PanelId,
        area: &DockArea,
        cx: &App,
    ) -> Option<Axis> {
        if !self.direct_member(target) {
            return None;
        }
        let PaneRef::Split { axis, children, .. } = self.root.kind() else {
            return Some(self.axis);
        };
        if axis != self.axis {
            // A historical nested-only subtree remains a local cell after Base removes unary framing.
            return None;
        }
        let slots = children
            .iter()
            .filter(|child| {
                let mut occupied = false;
                child.walk(&mut |node| {
                    if let PaneRef::Tabs { panels, .. } = node.kind() {
                        occupied |= panels.iter().any(|id| {
                            *id != source && area.panel(*id).is_some_and(|view| view.visible(cx))
                        });
                    }
                });
                occupied
            })
            .count();
        (slots < 2).then_some(self.axis)
    }
}

/// Locate a band around `node` in `tree`; Center requires the caller's elastic anchor identity.
/// Native regions use their root. Center uses the first branch separating the node from that anchor.
pub(super) fn region_band(
    tree: &PaneTree,
    region: DockPlacement,
    node: NodeId,
    anchor: Option<PanelId>,
) -> Option<Band<'_>> {
    if tree.find_node(node).is_none() {
        return None;
    }
    if region == DockPlacement::Center {
        return beside_anchor(tree.root(), node, tree.find_panel_node(anchor?)?);
    }
    Some(Band {
        root: content(tree.root()),
        axis: if region == DockPlacement::Bottom {
            Axis::Horizontal
        } else {
            Axis::Vertical
        },
    })
}

/// Ignore Base's single-child Center framing when determining actual regional hierarchy.
fn content(node: &PaneNode) -> &PaneNode {
    match node.kind() {
        PaneRef::Split { children, .. } if children.len() == 1 => content(&children[0]),
        _ => node,
    }
}

/// Derive a target band from the same current Base tree used by native hit testing and restore.
fn target_band(area: &DockArea, node: NodeId, anchor: Option<PanelId>) -> Option<Band<'_>> {
    [
        DockPlacement::Left,
        DockPlacement::Right,
        DockPlacement::Bottom,
        DockPlacement::Center,
    ]
    .into_iter()
    .find_map(|region| region_band(area.layout(region)?, region, node, anchor))
}

/// Restrict only new regional forks; concrete cells of an existing row/column accept nested layouts.
/// `source` is excluded before choosing the scope, so same-region moves cannot bypass the root rule.
pub(super) fn drop_axis(
    area: &DockArea,
    node: NodeId,
    source: PanelId,
    anchor: Option<PanelId>,
    cx: &App,
) -> Option<Axis> {
    target_band(area, node, anchor)?.drop_axis(node, source, area, cx)
}

/// An editor edge can create a region only when that side has no remaining visible window.
/// Every identity in `moving` is removed for this query: a whole group may relocate its own region.
/// Native docks and every center ancestor of `anchor` participate in the same side constraint.
pub(super) fn editor_side_available(
    area: &DockArea,
    anchor: PanelId,
    moving: &[PanelId],
    side: Placement,
    cx: &App,
) -> bool {
    let native = match side {
        Placement::Left => Some(DockPlacement::Left),
        Placement::Right => Some(DockPlacement::Right),
        Placement::Bottom => Some(DockPlacement::Bottom),
        Placement::Top => None,
    };
    if native.is_some_and(|region| {
        area.is_dock_open(region)
            && area
                .layout(region)
                .is_some_and(|tree| occupied_except(tree.root(), moving, anchor, area, cx))
    }) {
        return false;
    }
    area.layout(DockPlacement::Center).is_none_or(|tree| {
        tree.find_panel_node(anchor).is_none_or(|anchor_node| {
            !occupied_beside(tree.root(), anchor_node, side, moving, anchor, area, cx)
        })
    })
}

/// Inspect all siblings along the editor path, including outer bands above nested editor rows.
fn occupied_beside(
    node: &PaneNode,
    anchor_node: NodeId,
    side: Placement,
    moving: &[PanelId],
    anchor: PanelId,
    area: &DockArea,
    cx: &App,
) -> bool {
    let PaneRef::Split { axis, children, .. } = node.kind() else {
        return false;
    };
    let Some(index) = children
        .iter()
        .position(|child| contains(child, anchor_node))
    else {
        return false;
    };
    let before = matches!(side, Placement::Left | Placement::Top);
    if axis == side.axis()
        && children.iter().enumerate().any(|(other, child)| {
            (if before { other < index } else { other > index })
                && occupied_except(child, moving, anchor, area, cx)
        })
    {
        return true;
    }
    occupied_beside(
        &children[index],
        anchor_node,
        side,
        moving,
        anchor,
        area,
        cx,
    )
}

/// Hidden or exclusively replaced peers keep their registration without reserving a visible side.
fn occupied_except(
    node: &PaneNode,
    moving: &[PanelId],
    anchor: PanelId,
    area: &DockArea,
    cx: &App,
) -> bool {
    let mut occupied = false;
    node.walk(&mut |node| {
        if let PaneRef::Tabs { panels, .. } = node.kind() {
            occupied |= panels.iter().any(|id| {
                !moving.contains(id)
                    && *id != anchor
                    && area.panel(*id).is_some_and(|panel| panel.visible(cx))
            });
        }
    });
    occupied
}

/// Resolve a real pointer against the permitted split axis; `None` keeps all four editor edges.
/// Bounds and pointer use the same window-space logical pixels.
/// The middle remains invalid, and corner hits follow the region's permitted axis.
pub(super) fn edge_at(
    bounds: Bounds<Pixels>,
    pointer: Point<Pixels>,
    axis: Option<Axis>,
) -> Option<Placement> {
    if !bounds.contains(&pointer) {
        return None;
    }
    if axis != Some(Axis::Vertical) && pointer.x < bounds.left() + bounds.size.width * 0.35 {
        Some(Placement::Left)
    } else if axis != Some(Axis::Vertical) && pointer.x > bounds.left() + bounds.size.width * 0.65 {
        Some(Placement::Right)
    } else if axis != Some(Axis::Horizontal) && pointer.y < bounds.top() + bounds.size.height * 0.35
    {
        Some(Placement::Top)
    } else if axis != Some(Axis::Horizontal) && pointer.y > bounds.top() + bounds.size.height * 0.65
    {
        Some(Placement::Bottom)
    } else {
        None
    }
}

/// Return the first-level stacking axis for `node` in its current Base region.
/// A caller-supplied elastic anchor identifies center-local top/bottom and side bands without names.
/// This axis sizes/migrates whole bands; nested window drop restrictions use `drop_axis` instead.
pub(crate) fn stack_axis(
    area: &DockArea,
    node: NodeId,
    anchor: Option<gpui_base::dock::PanelId>,
) -> Option<Axis> {
    target_band(area, node, anchor).map(|band| band.axis)
}

/// Locate the entire existing band immediately left of Editor, independent of plugin IDs.
/// Hidden host panels still anchor their band; a moved Explorer never drags a new left panel rightward.
pub(super) fn left_sidebar_target(area: &DockArea, cx: &App) -> Option<(NodeId, Option<Pixels>)> {
    // Prefer a legacy center band until startup has migrated its entire subtree.
    // Current layouts use Base's independently sized left dock, including hidden peers.
    let band = area
        .layout(DockPlacement::Center)
        .and_then(|tree| {
            let editor = tree.panels().find(|id| {
                area.panel(*id)
                    .is_some_and(|panel| panel.panel_name(cx) == "Editor")
            })?;
            left_band(tree.root(), tree.find_panel_node(editor)?)
        })
        .or_else(|| {
            area.layout(DockPlacement::Left)
                .map(|tree| inner_band(tree.root()))
        })?;
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

/// A migrated custom outer column stays separate; new peers join the column nearest Editor.
fn inner_band(node: &PaneNode) -> &PaneNode {
    match node.kind() {
        PaneRef::Split {
            axis: Axis::Horizontal,
            children,
            ..
        } => children.last().map(inner_band).unwrap_or(node),
        _ => node,
    }
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

/// The nearest split separating a tool from Editor supplies its regional band, preserving nested cells.
fn beside_anchor(root: &PaneNode, target: NodeId, anchor: NodeId) -> Option<Band<'_>> {
    let PaneRef::Split { axis, children, .. } = root.kind() else {
        return None;
    };
    let target_child = children.iter().position(|child| contains(child, target))?;
    let anchor_child = children.iter().position(|child| contains(child, anchor))?;
    if target_child != anchor_child {
        Some(Band {
            root: content(&children[target_child]),
            axis: match axis {
                Axis::Horizontal => Axis::Vertical,
                Axis::Vertical => Axis::Horizontal,
            },
        })
    } else {
        beside_anchor(&children[target_child], target, anchor)
    }
}

/// Inspect public immutable nodes; mutations always stay behind Base's normalized edit methods.
fn contains(root: &PaneNode, target: NodeId) -> bool {
    let mut found = false;
    root.walk(&mut |node| found |= node.id() == target);
    found
}
