//! Immutable original geometry supplies visual markers and one release-time Base layout edit.
use super::*;
use gpui_base::dock::{DockLayout, PaneNode, PaneRef, PaneTree};
use gpui_kit::Axis;

mod group;

/// Original relationships and actual dimensions live only for one gesture.
pub(super) struct Snapshot {
    /// Root allocation converts window-space marker geometry into surface-local coordinates.
    pub area_bounds: Bounds<Pixels>,
    /// Existing regions before any gesture-time markers are painted.
    pub regions: Vec<Region>,
    /// Original shared handles keep native/plugin entities alive throughout a gesture.
    pub views: HashMap<PanelId, Arc<dyn PanelView>>,
    /// Freeze original visible membership, including exclusivity filtering, across target switches.
    pub visible: std::collections::HashSet<PanelId>,
    /// Pre-gesture logical-pixel allocations, indexed only by their original node identities.
    pub frames: HashMap<NodeId, Bounds<Pixels>>,
    /// Actual Base region allocations exclude any child content overflow from available space.
    allocations: HashMap<DockPlacement, Bounds<Pixels>>,
}

/// One Base region's original tree, peripheral size and open state.
pub(super) struct Region {
    /// Native region containing this immutable tree.
    pub placement: DockPlacement,
    /// Original relationships remain authoritative until release.
    pub tree: PaneTree,
    /// Width for side regions or height for Bottom; Center has no peripheral extent.
    extent: Option<Pixels>,
    /// Restore whether the native region originally contributed its allocation.
    open: bool,
}

/// A local insertion wraps only the target leaf, leaving every unrelated subtree intact.
struct Insertion<'a> {
    /// The only target leaf wrapped by this insertion.
    target: PanelId,
    /// Side of the target on which the moving subtree is installed.
    placement: Placement,
    /// Original source subtree, filtered to the identities in `members`.
    group: &'a PaneNode,
    /// Identities chosen for the current target: one leaf or its eligible whole regional combination.
    members: &'a [PanelId],
    /// Preferred moving size along the split axis, constrained by Base during layout.
    thickness: Pixels,
    /// Target allocation after source removal; a collapsed sibling can reclaim its original cell.
    bounds: Bounds<Pixels>,
    /// Only the moved band's first level adopts this axis; nested layouts keep their own orientation.
    stacking: Axis,
}

impl Snapshot {
    /// Capture live handles and actual allocations without modifying the Base area.
    pub fn capture(area: &DockArea, frames: &PaintedFrames, cx: &App) -> Self {
        let mut views = HashMap::new();
        let regions = REGIONS
            .into_iter()
            .filter_map(|placement| {
                let tree = area.layout(placement)?.clone();
                for id in tree.panels() {
                    if let Some(view) = area.panel(id) {
                        views.insert(id, view.clone());
                    }
                }
                Some(Region {
                    placement,
                    tree,
                    extent: area.dock_size(placement),
                    open: area.is_dock_open(placement),
                })
            })
            .collect();
        let visible = views
            .iter()
            .filter_map(|(id, view)| view.visible(cx).then_some(*id))
            .collect();
        let mut allocations = frames.regions.borrow().clone();
        // Base's center frame also contains Bottom. Subtract its painted allocation rather than
        // treating overflowing child contents as extra space available to a same-region move.
        if let Some(mut center) = allocations.get(&DockPlacement::Center).copied() {
            if !area.is_empty(DockPlacement::Bottom, cx) {
                if let Some(bottom) = allocations.get(&DockPlacement::Bottom) {
                    center.size.height = (center.size.height - bottom.size.height).max(px(0.));
                }
            }
            allocations.insert(DockPlacement::Center, center);
        }
        Self {
            area_bounds: area.bounds(),
            regions,
            views,
            visible,
            frames: frames.panels.borrow().clone(),
            allocations,
        }
    }

    /// Resolve stable panel membership rather than retaining a renderer's short-lived node id.
    pub fn region(&self, id: PanelId) -> Option<&Region> {
        self.regions
            .iter()
            .find(|region| region.tree.find_panel_node(id).is_some())
    }

    /// Borrow `id`'s first-level combination when eligible, otherwise just its original tab node.
    /// `anchor` identifies the elastic panel around which Center regions are derived.
    pub fn drag_band(&self, id: PanelId, anchor: Option<PanelId>) -> Option<&PaneNode> {
        let region = self.region(id)?;
        let node = region.tree.find_panel_node(id)?;
        placement::region_band(&region.tree, region.placement, node, anchor)
            .and_then(|band| band.drag_group(node))
            .or_else(|| region.tree.find_node(node))
    }

    /// List `node`'s originally visible identities in tree order, excluding hidden/exclusive peers.
    pub fn members(&self, node: &PaneNode) -> Vec<PanelId> {
        let mut members = Vec::new();
        node.walk(&mut |node| {
            if let PaneRef::Tabs { panels, .. } = node.kind() {
                members.extend(
                    panels
                        .iter()
                        .filter(|id| self.visible.contains(id))
                        .copied(),
                );
            }
        });
        members
    }

    /// Read one original outer allocation, including its native title.
    pub fn panel_bounds(&self, id: PanelId) -> Option<Bounds<Pixels>> {
        let node = self.region(id)?.tree.find_panel_node(id)?;
        self.frames.get(&node).copied()
    }

    /// Measure visible original leaves; invisible exclusive peers and hidden panels occupy no pixels.
    pub fn bounds(&self, node: &PaneNode, cx: &App) -> Option<Bounds<Pixels>> {
        match node.kind() {
            PaneRef::Tabs { panels, .. } => panels
                .iter()
                .any(|id| self.visible.contains(id))
                .then(|| self.frames.get(&node.id()).copied())
                .flatten(),
            PaneRef::Split { children, .. } => children
                .iter()
                .filter_map(|node| self.bounds(node, cx))
                .reduce(union),
        }
    }

    /// Visibility is independent of paint availability; unseen registered members keep their tree slot.
    fn visible_node(&self, node: &PaneNode) -> bool {
        let mut visible = false;
        node.walk(&mut |node| {
            if let PaneRef::Tabs { panels, .. } = node.kind() {
                visible |= panels.iter().any(|id| self.visible.contains(id));
            }
        });
        visible
    }

    /// Move only the explicit gesture members through Base builders, preserving unrelated subtrees.
    pub fn place(
        &self,
        source: PanelId,
        group: &PaneNode,
        members: &[PanelId],
        target: Target,
        thickness: Pixels,
        area: &mut DockArea,
        window: &mut Window,
        cx: &mut Context<DockArea>,
    ) {
        let origin = self.region(source).unwrap();
        let destination = self.region(target.panel).unwrap();
        let mut source_tree = origin.tree.clone();
        for id in members {
            source_tree.remove_panel(*id);
        }
        let target_tree = if origin.placement == destination.placement {
            &source_tree
        } else {
            &destination.tree
        };
        let bounds = if origin.placement == destination.placement {
            let remaining = source_tree
                .panels()
                .filter(|id| self.visible.contains(id))
                .collect::<Vec<_>>();
            self.allocations
                .get(&origin.placement)
                .copied()
                .or_else(|| self.bounds(origin.tree.root(), cx))
                .and_then(|bounds| {
                    self.group_rectangles(source_tree.root(), &remaining, bounds, None, cx)
                        .into_iter()
                        .find(|(id, _)| *id == target.panel)
                        .map(|(_, bounds)| bounds)
                })
                .unwrap_or(target.bounds)
        } else {
            target.bounds
        };
        let insertion = Insertion {
            target: target.panel,
            placement: target.placement,
            group,
            members,
            thickness,
            bounds,
            stacking: target.stacking,
        };
        let layout = self
            .layout(target_tree.root(), Some(&insertion), None, cx)
            .unwrap_or_else(DockLayout::tabs);
        // Destination first keeps the original view registered when its old source branch is removed.
        self.install(destination, layout, area, window, cx);
        if origin.placement != destination.placement {
            let layout = self
                .layout(source_tree.root(), None, None, cx)
                .unwrap_or_else(DockLayout::tabs);
            self.install(origin, layout, area, window, cx);
            release_empty(area, origin.placement, window, cx);
            // A nested child leaving a column must keep the region's original width. Only legacy
            // parallel columns can release an entire column's width; content overflow is not a resize.
            if matches!(origin.placement, DockPlacement::Left | DockPlacement::Right)
                && matches!(
                    origin.tree.root().kind(),
                    PaneRef::Split {
                        axis: Axis::Horizontal,
                        ..
                    }
                )
            {
                if let Some(bounds) = self.bounds(source_tree.root(), cx) {
                    area.set_dock_size(origin.placement, bounds.size.width, window, cx);
                }
            }
        }
    }

    /// Install one region with its original peripheral extent and open state.
    fn install(
        &self,
        region: &Region,
        layout: DockLayout,
        area: &mut DockArea,
        window: &mut Window,
        cx: &mut Context<DockArea>,
    ) {
        area.set_dock(region.placement, layout, window, cx);
        if let Some(extent) = region.extent {
            area.set_dock_size(region.placement, extent, window, cx);
            if area.is_dock_open(region.placement) != region.open {
                area.toggle_dock(region.placement, window, cx);
            }
        }
    }

    /// Reuse Base's views and split preferences; an insertion affects only its named target leaf.
    fn layout(
        &self,
        node: &PaneNode,
        insertion: Option<&Insertion>,
        only: Option<&[PanelId]>,
        cx: &App,
    ) -> Option<DockLayout> {
        let mut layout = match node.kind() {
            PaneRef::Tabs { panels, active_ix } => {
                let mut tabs = DockLayout::tabs().active_index(active_ix);
                let mut count = 0;
                for id in panels
                    .iter()
                    .filter(|id| only.is_none_or(|ids| ids.contains(id)))
                {
                    let view = self.views.get(id)?.clone();
                    tabs = tabs.panel_view(view, cx);
                    count += 1;
                }
                if count == 0 {
                    return None;
                }
                tabs
            }
            PaneRef::Split {
                axis,
                children,
                sizes,
            } => {
                let mut split = if axis == Axis::Horizontal {
                    DockLayout::h_split()
                } else {
                    DockLayout::v_split()
                };
                for (index, child) in children.iter().enumerate() {
                    if let Some(layout) = self.layout(child, insertion, only, cx) {
                        let measured = self.measured_extent(child, axis, cx);
                        split = split.child(
                            layout,
                            // An unmeasured hidden slot must remain unconstrained. A stale pixel
                            // preference makes Base rescale visible members against invisible space.
                            self.visible_node(child)
                                .then(|| measured.or_else(|| sizes.get(index).copied().flatten()))
                                .flatten(),
                        );
                    }
                }
                split
            }
        };
        if let Some(insert) = insertion.filter(|insert| matches!(node.kind(), PaneRef::Tabs { panels, .. } if panels.contains(&insert.target))) {
            let group = self.group_layout(insert.group, insert.members, insert.stacking, cx)?;
            let axis = insert.placement.axis();
            let available = if axis == Axis::Horizontal { insert.bounds.size.width } else { insert.bounds.size.height };
            let mut split = if axis == Axis::Horizontal { DockLayout::h_split() } else { DockLayout::v_split() };
            // Base clamps each pane to its minimum. Subtract that same committed size so a half-size
            // below 100px cannot make the two children larger than their containing cell.
            let minimum = gpui_base::PANEL_MIN_SIZE;
            let thickness = insert.thickness.clamp(minimum, (available - minimum).max(minimum));
            let remaining = (available - thickness).max(minimum);
            split = if matches!(insert.placement, Placement::Left | Placement::Top) {
                split.child(group, Some(thickness)).child(layout, Some(remaining))
            } else { split.child(layout, Some(remaining)).child(group, Some(thickness)) };
            layout = split;
        }
        Some(layout)
    }

    /// Retain ancestor slot weights when a descendant expands after source removal.
    /// Applying the expanded target's pixels to an ancestor would redistribute unrelated siblings twice.
    fn measured_extent(&self, child: &PaneNode, axis: Axis, cx: &App) -> Option<Pixels> {
        let extent = |bounds: Bounds<Pixels>| {
            if axis == Axis::Horizontal {
                bounds.size.width
            } else {
                bounds.size.height
            }
        };
        Some(extent(self.bounds(child, cx)?))
    }
}

/// Release a now-empty source only after destination installation preserves every moving identity.
fn release_empty(
    area: &mut DockArea,
    placement: DockPlacement,
    window: &mut Window,
    cx: &mut Context<DockArea>,
) {
    if placement == DockPlacement::Center || !area.is_empty(placement, cx) {
        return;
    }
    if placement == DockPlacement::Bottom {
        area.remove_dock(placement, window, cx);
    } else if area.is_dock_open(placement) {
        area.toggle_dock(placement, window, cx);
    }
}

/// Bounding union includes the title and gaps between sibling allocations.
pub(super) fn union(a: Bounds<Pixels>, b: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        gpui_kit::point(a.left().min(b.left()), a.top().min(b.top())),
        size(
            a.right().max(b.right()) - a.left().min(b.left()),
            a.bottom().max(b.bottom()) - a.top().min(b.top()),
        ),
    )
}
