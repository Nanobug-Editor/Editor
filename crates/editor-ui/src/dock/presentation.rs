//! Regional exclusivity filters Base visibility while retaining the sole native layout tree.

use super::*;
use gpui_base::dock::{PaneNode, PaneRef, PanelId, PanelState, TabGroup};
use gpui_kit::FocusHandle;
use std::collections::HashSet;
use std::sync::RwLock;

/// Shared presentation intent for one editor's panels; no text, split sizes or layout are copied.
#[derive(Clone, Default)]
pub struct PanelPresentation(Arc<RwLock<PresentationState>>);

/// Activation order selects either one exclusive leaf or the region's ordinary panel combination.
#[derive(Default)]
struct PresentationState {
    /// Declared exclusive leaves, independent of their current placement and visibility intent.
    exclusive: HashSet<PanelId>,
    /// Explicit reveals win over earlier reveals; registration alone never changes selection.
    activated: HashMap<PanelId, u64>,
    /// Monotonic order is local to this editor and has no wall-clock dependency.
    sequence: u64,
    /// Temporary display filtering; underlying panels retain their saved show/hide preferences.
    suppressed: HashSet<PanelId>,
}

impl PanelPresentation {
    /// Wrap `panel` with this editor's display policy, preserving native identity and skin lookup.
    pub fn panel_handle<P: crate::dock::panel::Panel>(
        &self,
        panel: Entity<P>,
    ) -> Arc<dyn PanelView> {
        Arc::new(PresentedPanel {
            inner: crate::dock::panel::panel_handle(panel),
            presentation: self.clone(),
        })
    }

    /// Apply the current manifest property to `panel`; false retains ordinary shared layout.
    pub fn set_exclusive(&self, panel: PanelId, exclusive: bool) {
        let mut state = self.0.write().unwrap();
        if exclusive {
            state.exclusive.insert(panel);
        } else {
            state.exclusive.remove(&panel);
        }
    }

    /// Make an explicitly revealed `panel` the most recent selection in its current region.
    pub fn activate(&self, panel: PanelId) {
        let mut state = self.0.write().unwrap();
        state.sequence += 1;
        let sequence = state.sequence;
        state.activated.insert(panel, sequence);
    }

    /// Withdraw metadata for a removed `panel` so hot replacement retains no stale entity IDs.
    pub fn forget(&self, panel: PanelId) {
        let mut state = self.0.write().unwrap();
        state.exclusive.remove(&panel);
        state.activated.remove(&panel);
        state.suppressed.remove(&panel);
    }

    /// Whether `panel` is temporarily replaced by another surface, without changing its intent.
    pub fn is_suppressed(&self, panel: PanelId) -> bool {
        self.0.read().unwrap().suppressed.contains(&panel)
    }

    /// Restore saved selections by stable dump names after all native panel factories are bound.
    pub fn restore(&self, area: &DockArea, names: &[String], cx: &App) {
        for region in regions(area, cx) {
            for id in region {
                if area
                    .panel(id)
                    .is_some_and(|panel| names.contains(&panel.dump(cx).panel_name))
                {
                    self.activate(id);
                }
            }
        }
    }

    /// Recompute display from the live layout and raw visibility; return stable regional selections.
    pub fn synchronize(&self, area: &DockArea, cx: &App) -> (Vec<String>, bool) {
        // Sample before taking the write lock: a wrapped visibility getter reads this same policy.
        let regions = regions(area, cx).map(|ids| {
            ids.into_iter()
                .filter(|id| {
                    area.panel(*id).is_some_and(|panel| {
                        PanelHandle::of(panel)
                            .map_or_else(|| panel.visible(cx), |handle| handle.visible(cx))
                    })
                })
                .collect::<Vec<_>>()
        });
        let mut state = self.0.write().unwrap();
        let previous = std::mem::take(&mut state.suppressed);
        let mut selected_names = Vec::new();
        for ids in regions {
            let selected = ids.iter().copied().max_by_key(|id| {
                (
                    state.activated.get(id).copied().unwrap_or(0),
                    state.exclusive.contains(id),
                )
            });
            let Some(selected) = selected else { continue };
            if let Some(panel) = area.panel(selected) {
                selected_names.push(panel.dump(cx).panel_name);
            }
            let exclusive = state.exclusive.contains(&selected);
            for id in ids {
                if (exclusive && id != selected) || (!exclusive && state.exclusive.contains(&id)) {
                    state.suppressed.insert(id);
                }
            }
        }
        (selected_names, state.suppressed != previous)
    }
}

/// A missing hidden exclusive leaf can be registered safely while its real region retains peers.
/// Raw visibility includes ordinary peers currently replaced by an exclusive surface.
pub fn has_visible_region(area: &DockArea, placement: DockPlacement, cx: &App) -> bool {
    let index = match placement {
        DockPlacement::Left => 0,
        DockPlacement::Right => 1,
        DockPlacement::Bottom => 2,
        DockPlacement::Center => return false,
    };
    regions(area, cx)[index].iter().any(|id| {
        area.panel(*id).is_some_and(|panel| {
            PanelHandle::of(panel).map_or_else(|| panel.visible(cx), |handle| handle.visible(cx))
        })
    })
}

/// Find the actual bands around Editor, plus peripheral docks, without relying on manifest hints.
fn regions(area: &DockArea, cx: &App) -> [Vec<PanelId>; 4] {
    let mut regions: [Vec<PanelId>; 4] = Default::default();
    for (index, placement) in [
        DockPlacement::Left,
        DockPlacement::Right,
        DockPlacement::Bottom,
    ]
    .into_iter()
    .enumerate()
    {
        if let Some(tree) = area.layout(placement) {
            regions[index].extend(tree.panels());
        }
    }
    if let Some(tree) = area.layout(DockPlacement::Center) {
        let editor = tree.panels().find(|id| {
            area.panel(*id)
                .is_some_and(|panel| panel.panel_name(cx) == "Editor")
        });
        if let Some(editor) = editor {
            collect_bands(tree.root(), editor, &mut regions);
        }
    }
    regions
}

/// Partition siblings along the Editor path into left/right/bottom/top regions; Editor stays visible.
fn collect_bands(node: &PaneNode, editor: PanelId, regions: &mut [Vec<PanelId>; 4]) -> bool {
    match node.kind() {
        PaneRef::Tabs { panels, .. } => panels.contains(&editor),
        PaneRef::Split { axis, children, .. } => {
            let Some(index) = children
                .iter()
                .position(|child| collect_bands(child, editor, regions))
            else {
                return false;
            };
            for (sibling, child) in children.iter().enumerate() {
                if sibling == index {
                    continue;
                }
                let region = match (axis, sibling < index) {
                    (gpui_kit::Axis::Horizontal, true) => 0,
                    (gpui_kit::Axis::Horizontal, false) => 1,
                    (gpui_kit::Axis::Vertical, false) => 2,
                    (gpui_kit::Axis::Vertical, true) => 3,
                };
                child.walk(&mut |node| {
                    if let PaneRef::Tabs { panels, .. } = node.kind() {
                        regions[region].extend(panels);
                    }
                });
            }
            true
        }
    }
}

/// Only visibility is filtered; callbacks, resources and the presentation downcast stay native.
pub(super) struct PresentedPanel {
    /// Original component handle owns title rendering and all Base lifecycle callbacks.
    pub(super) inner: Arc<dyn PanelView>,
    /// Shared per-editor selection, never a second persistent layout.
    presentation: PanelPresentation,
}

impl PanelView for PresentedPanel {
    fn panel_name(&self, cx: &App) -> &'static str {
        self.inner.panel_name(cx)
    }
    fn panel_id(&self, cx: &App) -> PanelId {
        self.inner.panel_id(cx)
    }
    fn visible(&self, cx: &App) -> bool {
        self.inner.visible(cx) && !self.presentation.is_suppressed(self.panel_id(cx))
    }
    fn closable(&self, cx: &App) -> bool {
        self.inner.closable(cx)
    }
    fn zoomable(&self, cx: &App) -> bool {
        self.inner.zoomable(cx)
    }
    fn set_active(&self, active: bool, window: &mut Window, cx: &mut App) {
        self.inner.set_active(active, window, cx)
    }
    fn set_zoomed(&self, zoomed: bool, window: &mut Window, cx: &mut App) {
        self.inner.set_zoomed(zoomed, window, cx)
    }
    fn on_added_to(&self, group: WeakEntity<TabGroup>, window: &mut Window, cx: &mut App) {
        self.inner.on_added_to(group, window, cx)
    }
    fn on_removed(&self, window: &mut Window, cx: &mut App) {
        self.inner.on_removed(window, cx)
    }
    fn view(&self) -> AnyView {
        self.inner.view()
    }
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.inner.focus_handle(cx)
    }
    fn dump(&self, cx: &App) -> PanelState {
        self.inner.dump(cx)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Read a wrapped leaf's declared mode without treating last frame's suppression as its intent.
pub(super) fn is_exclusive(panel: &Arc<dyn PanelView>, cx: &App) -> bool {
    panel
        .as_any()
        .downcast_ref::<PresentedPanel>()
        .is_some_and(|view| {
            view.presentation
                .0
                .read()
                .unwrap()
                .exclusive
                .contains(&view.panel_id(cx))
        })
}
