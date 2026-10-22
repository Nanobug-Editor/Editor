//! Editor-local columns keep their pixels while the editor absorbs regional width changes.
//! All trees, view identities and lasting resize preferences remain in the original Base DockArea.
use super::*;
use gpui_base::dock::{DockLayout, PaneNode, PaneRef, PanelId};
use gpui_kit::{Axis, Size};

/// Before a root frame, keep measured side-column widths and give the new available width to Editor.
/// Native split gestures continue to own their intermediate sizes; only external width changes reflow.
pub(super) fn prepare(
    area: &mut DockArea,
    frames: &HashMap<NodeId, Bounds<Pixels>>,
    viewport_width: Pixels,
    elastic: Option<PanelId>,
    window: &mut Window,
    cx: &mut Context<DockArea>,
) {
    if cx.has_active_drag() {
        return;
    }
    let Some(tree) = area.layout(DockPlacement::Center) else {
        return;
    };
    let Some(anchor) = editor_node(area, elastic) else {
        return;
    };
    if !has_columns(area, tree.root(), anchor, cx) {
        return;
    }
    let Some(measured) = extent(area, tree.root(), frames, None, None, cx)
        .map(|size| size.width)
        .or_else(|| preferred_width(area, tree.root(), cx))
    else {
        return;
    };
    if (measured - available_width(area, viewport_width, cx)).abs() < px(0.5) {
        return;
    }
    install(area, frames, viewport_width, elastic, window, cx);
}

/// Before the first restore paint, Base's persisted horizontal weights retain the intended pixels.
/// Using them prevents a temporary initial window size from permanently scaling a saved tool column.
fn preferred_width(area: &DockArea, node: &PaneNode, cx: &App) -> Option<Pixels> {
    if !visible(area, node, cx) {
        return Some(px(0.));
    }
    match node.kind() {
        PaneRef::Tabs { .. } => None,
        PaneRef::Split {
            axis: Axis::Horizontal,
            children,
            sizes,
        } => children
            .iter()
            .enumerate()
            .try_fold(px(0.), |total, (index, child)| {
                let width = if visible(area, child, cx) {
                    sizes
                        .get(index)
                        .copied()
                        .flatten()
                        .or_else(|| preferred_width(area, child, cx))?
                } else {
                    px(0.)
                };
                Some(total + width)
            }),
        PaneRef::Split { children, .. } => children
            .iter()
            .filter_map(|child| preferred_width(area, child, cx))
            .reduce(|a, b| a.max(b)),
    }
}

/// Recompose a single Base description from live handles and measurements; no local layout is retained.
/// The caller's elastic panel absorbs width changes while tool columns retain their preferences.
pub(super) fn install(
    area: &mut DockArea,
    frames: &HashMap<NodeId, Bounds<Pixels>>,
    viewport_width: Pixels,
    elastic: Option<PanelId>,
    window: &mut Window,
    cx: &mut Context<DockArea>,
) {
    let Some(anchor) = editor_node(area, elastic) else {
        return;
    };
    let width = available_width(area, viewport_width, cx);
    if width <= px(0.) {
        return;
    }
    let Some(tree) = area.layout(DockPlacement::Center) else {
        return;
    };
    let height = [DockPlacement::Bottom]
        .into_iter()
        .fold(area.bounds().size.height, |height, side| {
            if area.is_dock_open(side) && !area.is_empty(side, cx) {
                height - area.dock_size(side).unwrap_or(px(0.))
            } else {
                height
            }
        })
        .max(px(0.));
    let Some(layout) = compose(area, tree.root(), anchor, size(width, height), frames, cx) else {
        return;
    };
    area.set_center(layout, window, cx);
}

/// Center and its bottom dock share the width left by actually visible, open peripheral docks.
fn available_width(area: &DockArea, viewport: Pixels, cx: &App) -> Pixels {
    [DockPlacement::Left, DockPlacement::Right]
        .into_iter()
        .fold(viewport, |width, side| {
            if area.is_dock_open(side) && !area.is_empty(side, cx) {
                width - area.dock_size(side).unwrap_or(px(0.))
            } else {
                width
            }
        })
        .max(px(0.))
}

/// Locate the host editor by its public native panel identity, independent of plugin IDs.
fn editor_node(area: &DockArea, elastic: Option<PanelId>) -> Option<NodeId> {
    let tree = area.layout(DockPlacement::Center)?;
    tree.find_panel_node(elastic?)
}

/// A horizontal ancestor with a visible non-editor branch is a row whose tool columns stay fixed.
fn has_columns(area: &DockArea, node: &PaneNode, anchor: NodeId, cx: &App) -> bool {
    let PaneRef::Split { axis, children, .. } = node.kind() else {
        return false;
    };
    let Some(index) = children.iter().position(|child| contains(child, anchor)) else {
        return false;
    };
    (axis == Axis::Horizontal
        && children
            .iter()
            .enumerate()
            .any(|(i, child)| i != index && visible(area, child, cx)))
        || has_columns(area, &children[index], anchor, cx)
}

/// Read visibility through Base's registered handles without retaining another visibility model.
fn visible(area: &DockArea, node: &PaneNode, cx: &App) -> bool {
    match node.kind() {
        PaneRef::Tabs { panels, .. } => panels
            .iter()
            .any(|id| area.panel(*id).is_some_and(|view| view.visible(cx))),
        PaneRef::Split { children, .. } => children.iter().any(|node| visible(area, node, cx)),
    }
}

/// Immutable node containment is used only while assembling this one normalized Base edit.
fn contains(node: &PaneNode, target: NodeId) -> bool {
    let mut found = false;
    node.walk(&mut |node| found |= node.id() == target);
    found
}

/// Reuse live views while the anchor absorbs space released by a departing peripheral region.
/// Both axes retain tool dimensions; a group's own children scale together in their original ratio.
fn compose(
    area: &DockArea,
    node: &PaneNode,
    anchor: NodeId,
    allocation: Size<Pixels>,
    frames: &HashMap<NodeId, Bounds<Pixels>>,
    cx: &App,
) -> Option<DockLayout> {
    match node.kind() {
        PaneRef::Tabs { panels, active_ix } => {
            let mut tabs = DockLayout::tabs().active_index(active_ix);
            for id in panels {
                tabs = tabs.panel_view(area.panel(*id)?.clone(), cx);
            }
            Some(tabs)
        }
        PaneRef::Split {
            axis,
            children,
            sizes,
        } => {
            let dimensions = children
                .iter()
                .enumerate()
                .map(|(index, child)| {
                    if !visible(area, child, cx) {
                        return Some(px(0.));
                    }
                    extent(area, child, frames, None, None, cx)
                        .map(|extent| axis_extent(extent, axis))
                        .or_else(|| sizes.get(index).copied().flatten())
                        .or_else(|| {
                            if axis == Axis::Horizontal {
                                preferred_width(area, child, cx)
                            } else {
                                None
                            }
                        })
                })
                .collect::<Option<Vec<_>>>()?;
            let editor = children.iter().position(|child| contains(child, anchor));
            let fixed = dimensions
                .iter()
                .enumerate()
                .filter(|(i, _)| Some(*i) != editor)
                .fold(px(0.), |total, (_, value)| total + *value);
            let available = axis_extent(allocation, axis);
            // Extremely small targets still obey Base's minimum pane constraints. With normal room,
            // the editor absorbs reclaimed space instead of proportionally enlarging the moved tools.
            let factor = if fixed <= px(0.) {
                1.
            } else if editor.is_some() {
                ((available - gpui_base::PANEL_MIN_SIZE).max(px(0.)) / fixed).min(1.)
            } else {
                available / fixed
            };
            let mut layout = if axis == Axis::Horizontal {
                DockLayout::h_split()
            } else {
                DockLayout::v_split()
            };
            for (index, child) in children.iter().enumerate() {
                let along = if Some(index) == editor {
                    (available - fixed * factor).max(px(0.))
                } else {
                    dimensions[index] * factor
                };
                let child_size = if axis == Axis::Horizontal {
                    size(along, allocation.height)
                } else {
                    size(allocation.width, along)
                };
                let weight = if available > px(0.) {
                    Some(along)
                } else {
                    sizes.get(index).copied().flatten()
                };
                layout = layout.child(
                    compose(area, child, anchor, child_size, frames, cx)?,
                    weight,
                );
            }
            Some(layout)
        }
    }
}

/// Select the allocation dimension along the Base split axis.
fn axis_extent(extent: Size<Pixels>, axis: Axis) -> Pixels {
    if axis == Axis::Horizontal {
        extent.width
    } else {
        extent.height
    }
}

/// Sum the outer allocations along a split axis; hidden or explicitly removed leaves occupy no space.
/// Missing paint data cancels preparation until the next frame instead of guessing or storing a second tree.
fn extent(
    area: &DockArea,
    node: &PaneNode,
    frames: &HashMap<NodeId, Bounds<Pixels>>,
    removed: Option<PanelId>,
    replacement: Option<(PanelId, Size<Pixels>)>,
    cx: &App,
) -> Option<Size<Pixels>> {
    match node.kind() {
        PaneRef::Tabs { panels, .. } => {
            if !panels.iter().any(|id| {
                Some(*id) != removed && area.panel(*id).is_some_and(|view| view.visible(cx))
            }) {
                return Some(size(px(0.), px(0.)));
            }
            if let Some((_, slot)) = replacement.filter(|(id, _)| panels.contains(id)) {
                return Some(slot);
            }
            frames.get(&node.id()).map(|bounds| bounds.size)
        }
        PaneRef::Split { axis, children, .. } => {
            children
                .iter()
                .try_fold(size(px(0.), px(0.)), |total, child| {
                    let child = extent(area, child, frames, removed, replacement, cx)?;
                    Some(if axis == Axis::Horizontal {
                        size(total.width + child.width, total.height.max(child.height))
                    } else {
                        size(total.width.max(child.width), total.height + child.height)
                    })
                })
        }
    }
}
