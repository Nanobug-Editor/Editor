//! Hidden size preferences complement Base's sole tree without reserving invisible split space.
//! Base normalizes every concrete weight, so hidden slots temporarily carry zero; only their
//! previous preference lives here until reveal, removal, or serialization into the original tree.

use super::*;
use gpui_base::dock::{DockLayout, PaneNode, PaneRef, PanelInfo};
use gpui_kit::Axis;
use std::collections::HashSet;

/// A slot survives Base node replacement through its axis and stable contained panel identities.
#[derive(Clone, PartialEq, Eq, Hash)]
struct SlotKey {
    horizontal: bool,
    panels: Vec<PanelId>,
}

impl SlotKey {
    /// Describe only this slot's size ownership, never its parent, position or surrounding layout.
    fn new(axis: Axis, node: &PaneNode) -> Self {
        let mut panels = Vec::new();
        node.walk(&mut |node| {
            if let PaneRef::Tabs { panels: ids, .. } = node.kind() {
                panels.extend_from_slice(ids);
            }
        });
        panels.sort_unstable();
        Self {
            horizontal: axis == Axis::Horizontal,
            panels,
        }
    }
}

/// Only hidden preferences are retained; visible widths and every structural edit belong to Base.
#[derive(Clone, Default)]
pub(super) struct HiddenSizes(Rc<RefCell<HashMap<SlotKey, Option<Pixels>>>>);

impl HiddenSizes {
    /// An explicit session load supplies new preferences and must not inherit the previous projection.
    pub fn clear(&self) {
        self.0.borrow_mut().clear();
    }

    /// Project hidden slots to zero and restore newly visible slots before any layout measurement.
    /// Mutations use Base's public edits; unconstrained siblings retain their original `None` values.
    pub fn prepare(
        &self,
        area: &mut DockArea,
        anchor: Option<PanelId>,
        window: &mut Window,
        cx: &mut Context<DockArea>,
    ) {
        let mut edits = Vec::new();
        {
            let mut retained = self.0.borrow_mut();
            let mut live = HashSet::new();
            for placement in REGIONS {
                let Some(tree) = area.layout(placement) else {
                    continue;
                };
                let mut changes = HashMap::new();
                tree.root().walk(&mut |node| {
                    let PaneRef::Split {
                        axis,
                        children,
                        sizes,
                    } = node.kind()
                    else {
                        return;
                    };
                    let alternatives = alternatives(node, anchor, area, cx);
                    let mut updated = sizes.to_vec();
                    for (index, child) in children.iter().enumerate() {
                        let key = SlotKey::new(axis, child);
                        live.insert(key.clone());
                        let current = sizes[index];
                        updated[index] = if alternatives[index] {
                            retained.remove(&key);
                            Some(px(0.))
                        } else if !visible(child, area, cx) {
                            retained.entry(key).or_insert(current);
                            Some(px(0.))
                        } else if let Some(saved) = retained.remove(&key) {
                            // A move may already have assigned this newly visible slot a new size.
                            // Only our own zero/None projection is eligible for restoration.
                            if current.is_none_or(|size| size == px(0.)) {
                                saved
                            } else {
                                current
                            }
                        } else {
                            current
                        };
                    }
                    if updated != sizes {
                        changes.insert(node.id(), updated);
                    }
                });
                if !changes.is_empty() {
                    edits.push((placement, changes));
                }
            }
            // A removed panel, changed group membership or changed axis cannot own an old preference.
            retained.retain(|key, _| live.contains(key));
        }
        // Emit layout events only after releasing the overlay borrow: subscribers may dump immediately.
        for (placement, changes) in edits {
            if changes
                .values()
                .all(|sizes| sizes.iter().all(Option::is_some))
            {
                for (node, sizes) in changes {
                    area.set_split_sizes(node, sizes.into_iter().flatten().collect(), window, cx);
                }
            } else {
                let layout = area
                    .layout(placement)
                    .and_then(|tree| describe(tree.root(), area, &changes, cx));
                if let Some(layout) = layout {
                    area.set_dock(placement, layout, window, cx);
                }
            }
        }
    }

    /// Save Base's live layout with hidden preferences restored in its existing size fields.
    /// Expanded visible siblings use tree weights while peers are hidden, retaining their ratio.
    pub fn dump(&self, area: &DockArea, anchor: Option<PanelId>, cx: &App) -> DockAreaState {
        let mut state = area.dump(cx);
        let retained = self.0.borrow();
        if let Some(tree) = area.layout(DockPlacement::Center) {
            restore_dump(tree.root(), &mut state.center, &retained, anchor, area, cx);
        }
        for (placement, slot) in [
            (DockPlacement::Left, &mut state.left_dock),
            (DockPlacement::Right, &mut state.right_dock),
            (DockPlacement::Bottom, &mut state.bottom_dock),
        ] {
            let Some(dock) = slot.take() else { continue };
            let mut panel = dock.panel().clone();
            if let Some(tree) = area.layout(placement) {
                restore_dump(tree.root(), &mut panel, &retained, anchor, area, cx);
            }
            *slot = Some(DockState::new(
                panel,
                dock.placement(),
                dock.size(),
                dock.open(),
            ));
        }
        state
    }
}

/// Scan all native regions; closed outer docks retain the same child preferences as open ones.
const REGIONS: [DockPlacement; 4] = [
    DockPlacement::Center,
    DockPlacement::Left,
    DockPlacement::Right,
    DockPlacement::Bottom,
];

/// Base uses registered panel visibility for slot occupancy, including exclusive suppression.
fn visible(node: &PaneNode, area: &DockArea, cx: &App) -> bool {
    match node.kind() {
        PaneRef::Tabs { panels, .. } => panels
            .iter()
            .any(|id| area.panel(*id).is_some_and(|panel| panel.visible(cx))),
        PaneRef::Split { children, .. } => children.iter().any(|child| visible(child, area, cx)),
    }
}

/// Exclusive alternatives share ordinary space, while complete regions beside Editor keep their size.
fn alternatives(node: &PaneNode, anchor: Option<PanelId>, area: &DockArea, cx: &App) -> Vec<bool> {
    let PaneRef::Split { children, .. } = node.kind() else {
        return Vec::new();
    };
    let mut contains_anchor = false;
    node.walk(&mut |child| {
        if let PaneRef::Tabs { panels, .. } = child.kind() {
            contains_anchor |= anchor.is_some_and(|anchor| panels.contains(&anchor));
        }
    });
    let mut exclusive = children
        .iter()
        .map(|child| exclusive_only(child, area, cx))
        .collect::<Vec<_>>();
    if contains_anchor || exclusive.iter().all(|exclusive| *exclusive) {
        exclusive.fill(false);
    }
    exclusive
}

/// Mixed subtrees retain ordinary ownership even while an exclusive descendant is selected.
fn exclusive_only(node: &PaneNode, area: &DockArea, cx: &App) -> bool {
    match node.kind() {
        PaneRef::Tabs { panels, .. } => {
            !panels.is_empty()
                && panels.iter().all(|id| {
                    area.panel(*id)
                        .is_some_and(|panel| presentation::is_exclusive(panel, cx))
                })
        }
        PaneRef::Split { children, .. } => {
            !children.is_empty() && children.iter().all(|child| exclusive_only(child, area, cx))
        }
    }
}

/// Rebuild only when Base's pixel-only setter cannot preserve an unconstrained sibling.
fn describe(
    node: &PaneNode,
    area: &DockArea,
    changes: &HashMap<NodeId, Vec<Option<Pixels>>>,
    cx: &App,
) -> Option<DockLayout> {
    match node.kind() {
        PaneRef::Tabs { panels, active_ix } => {
            let mut layout = DockLayout::tabs().active_index(active_ix);
            for id in panels {
                layout = layout.panel_view(area.panel(*id)?.clone(), cx);
            }
            Some(layout)
        }
        PaneRef::Split {
            axis,
            children,
            sizes,
        } => {
            let sizes = changes.get(&node.id()).map(Vec::as_slice).unwrap_or(sizes);
            let mut layout = if axis == Axis::Horizontal {
                DockLayout::h_split()
            } else {
                DockLayout::v_split()
            };
            for (index, child) in children.iter().enumerate() {
                layout = layout.child(describe(child, area, changes, cx)?, sizes[index]);
            }
            Some(layout)
        }
    }
}

/// Match Base's unchanged serialized shape; the overlay supplies sizes, never nodes or panel content.
fn restore_dump(
    node: &PaneNode,
    state: &mut PanelState,
    retained: &HashMap<SlotKey, Option<Pixels>>,
    anchor: Option<PanelId>,
    area: &DockArea,
    cx: &App,
) {
    let PaneRef::Split {
        axis,
        children,
        sizes,
    } = node.kind()
    else {
        return;
    };
    let PanelInfo::Stack {
        sizes: saved_sizes, ..
    } = &mut state.info
    else {
        return;
    };
    let alternatives = alternatives(node, anchor, area, cx);
    let projected = children.iter().enumerate().any(|(index, child)| {
        alternatives[index]
            || !visible(child, area, cx)
            || retained.contains_key(&SlotKey::new(axis, child))
    });
    if projected {
        for (index, child) in children.iter().enumerate() {
            let Some(saved) = saved_sizes.get_mut(index) else {
                continue;
            };
            if alternatives[index] {
                *saved = px(0.);
                continue;
            }
            let original = retained
                .get(&SlotKey::new(axis, child))
                .copied()
                .filter(|_| sizes[index].is_none_or(|size| size == px(0.)))
                .unwrap_or(sizes[index]);
            // Base's persisted zero sentinel restores None. A flexible sibling must stay
            // flexible rather than save the expanded measurement produced while peers hide.
            *saved = original.unwrap_or(px(0.));
        }
    }
    for (child, saved) in children.iter().zip(&mut state.children) {
        restore_dump(child, saved, retained, anchor, area, cx);
    }
}
