//! Reusable dock gesture: unchanged allocations, source/target markers and release-only placement on the same Base area.
use super::*;
use gpui_base::dock::{PaneNode, PanelId};
use gpui_kit::{Axis, DispatchPhase, FocusHandle, KeyDownEvent, Point};

mod markers;
mod preview;
mod snapshot;
use snapshot::Snapshot;

/// Every dock surface uses the same transaction and pointer routing, including plugin panels.
#[derive(Clone, Default)]
pub(super) struct LiveDrag(Rc<RefCell<Option<Session>>>, Rc<Cell<Option<PanelId>>>);

/// Stable panel identity and its unchanged allocation define a drop preview target.
#[derive(Clone, Copy, PartialEq)]
struct Target {
    /// Registered identity locates the target when Base rebuilds containers on release.
    panel: PanelId,
    /// Local edge chosen inside that target, with no regional direction rewriting.
    placement: Placement,
    /// Current full allocation before any release-time insertion.
    bounds: Bounds<Pixels>,
    /// Members of a moved band stack along the destination region's permitted axis.
    stacking: Axis,
    /// Split ordinary peers, or select the whole actual side when either window is exclusive.
    mode: DropMode,
}

/// Preview and commit carry the same explicit placement meaning through the gesture.
#[derive(Clone, Copy, PartialEq)]
enum DropMode {
    /// Existing ordinary edge placement leaves both windows displayed.
    Split,
    /// One incoming window fills the target region while retained peers are temporarily hidden.
    Replace,
}

/// Read-only pre-drag data sizes markers and supplies one final Base edit.
struct Session {
    /// Immutable original relationships and actual geometry for release-time placement.
    original: Snapshot,
    /// The title actually grabbed; it remains primary even when its first-level peers move together.
    source: PanelId,
    /// Eligible first-level combination, or the grabbed leaf for a nested child or ordinary surface.
    band: PaneNode,
    /// Marked visible identities in original order, also named by compact floating cards.
    members: Vec<PanelId>,
    /// Current valid placement, or no blue preview while the pointer is outside an edge.
    target: Option<Target>,
    /// GPUI's original grab offset is corrected for compact title cards.
    grab: Point<Pixels>,
    /// The focused native control before the gesture, restored when it ends.
    focus: Option<FocusHandle>,
    /// Distinguish a retiring GPUI preview from a later gesture's live state.
    token: u64,
    /// The caller chooses the elastic editor-like panel; ordinary surfaces need no anchor.
    anchor: Option<PanelId>,
}

/// Scan Center first so editor-local slots retain the same hit priority after regional moves.
const REGIONS: [DockPlacement; 4] = [
    DockPlacement::Center,
    DockPlacement::Left,
    DockPlacement::Right,
    DockPlacement::Bottom,
];

impl LiveDrag {
    /// Configure the elastic panel whose edges share the one-region-per-side constraint.
    pub fn set_anchor(&self, panel: PanelId) {
        self.1.set(Some(panel));
    }

    /// Expose the configured anchor identity to the renderer and width preparation.
    pub fn anchor(&self) -> Option<PanelId> {
        self.1.get()
    }
    /// Attach the reusable native gesture to any caller-owned title without exposing its preview view type.
    pub fn attach(
        &self,
        title: Stateful<Div>,
        drag: DragPanel,
        area: WeakEntity<DockArea>,
        frames: PaintedFrames,
        title_height: f32,
    ) -> Stateful<Div> {
        let controller = self.clone();
        title.on_drag(drag, move |drag, offset, window, cx| {
            drag.set_drag_offset(offset);
            cx.stop_propagation();
            let _ = area.update(cx, |area, cx| {
                controller.begin(drag, area, &frames, window, cx)
            });
            controller.preview(
                area.clone(),
                frames.clone(),
                title_height,
                drag.drag_session_id(),
                window,
                cx,
            )
        })
    }
    /// Host persistence and visibility reconciliation pause until a gesture ends.
    pub fn active(&self) -> bool {
        self.0
            .try_borrow()
            .map_or(true, |session| session.is_some())
    }

    /// Install Escape on an ancestor of focused content; a sibling canvas cannot receive key dispatch.
    pub fn capture_escape(
        &self,
        element: Stateful<Div>,
        area: WeakEntity<DockArea>,
    ) -> Stateful<Div> {
        let controller = self.clone();
        element.capture_key_down(move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape" && controller.active() {
                let _ = area.update(cx, |area, cx| controller.finish(false, area, window, cx));
                cx.stop_active_drag(window);
                cx.stop_propagation();
            }
        })
    }

    /// Capture real dimensions without hiding views, moving leaves or emitting a layout change.
    pub fn begin(
        &self,
        drag: &DragPanel,
        area: &mut DockArea,
        frames: &PaintedFrames,
        window: &mut Window,
        cx: &mut Context<DockArea>,
    ) {
        if self.active() || area.is_locked() || area.is_zoomed() {
            return;
        }
        let original = Snapshot::capture(area, frames, cx);
        let Some(band) = original.drag_band(drag.panel(), self.anchor()).cloned() else {
            return;
        };
        *self.0.borrow_mut() = Some(Session {
            original,
            source: drag.panel(),
            band,
            members: vec![drag.panel()],
            target: None,
            grab: drag.drag_offset(),
            focus: window.focused(cx),
            token: drag.drag_session_id(),
            anchor: self.anchor(),
        });
        cx.notify();
    }

    /// Route all pointer positions, including gaps and positions outside the dock, before Base drop handlers.
    pub fn surface(&self, area: WeakEntity<DockArea>, frames: PaintedFrames) -> AnyElement {
        let this = self.clone();
        canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                let move_state = this.clone();
                let move_area = area.clone();
                let move_frames = frames.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture
                        && move_state.active()
                        && cx.has_active_drag()
                    {
                        let _ = move_area.update(cx, |area, cx| {
                            move_state.hover(event.position, area, &move_frames, window, cx)
                        });
                        window.refresh();
                    }
                });
                let end_state = this.clone();
                let end_area = area.clone();
                let end_frames = frames.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture && end_state.active() {
                        let _ = end_area.update(cx, |area, cx| {
                            end_state.hover(event.position, area, &end_frames, window, cx);
                            end_state.finish(true, area, window, cx);
                        });
                        cx.stop_active_drag(window);
                        cx.stop_propagation();
                    }
                });
            },
        )
        .absolute()
        .inset_0()
        .into_any_element()
    }

    /// A valid release edits Base exactly once; cancellation only clears visual gesture state.
    fn finish(
        &self,
        commit: bool,
        area: &mut DockArea,
        window: &mut Window,
        cx: &mut Context<DockArea>,
    ) {
        let Some(session) = self.0.borrow_mut().take() else {
            return;
        };
        if commit && !area.is_locked() && session.original.owns_layout(area, cx) {
            if let Some(target) = session.target {
                session.original.place(
                    session.source,
                    &session.band,
                    &session.members,
                    target,
                    session.thickness(target, cx),
                    area,
                    window,
                    cx,
                );
                super::columns::install(
                    area,
                    &HashMap::new(),
                    window.viewport_size().width,
                    session.anchor,
                    window,
                    cx,
                );
                cx.emit(DockEvent::LayoutChanged);
            }
        }
        if let Some(focus) = session.focus {
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    /// Hover selects markers and compact cards while the actual dock tree stays unchanged.
    fn hover(
        &self,
        pointer: Point<Pixels>,
        area: &mut DockArea,
        frames: &PaintedFrames,
        window: &mut Window,
        cx: &mut Context<DockArea>,
    ) {
        let mut state = self.0.borrow_mut();
        let Some(session) = state.as_mut() else {
            return;
        };
        if area.is_locked() || !session.original.owns_layout(area, cx) {
            drop(state);
            self.finish(false, area, window, cx);
            // Retire Base's payload too: leaving it alive would let its fallback drop listener
            // place the source after our stale transaction and markers have already been cancelled.
            cx.stop_active_drag(window);
            return;
        }
        // MouseUp rechecks current occupancy even at an unchanged pointer: a newly revealed
        // native region must invalidate an earlier preview before any layout is committed.
        let (target, members) = session
            .hit(pointer, area, &frames.panels.borrow(), cx)
            .map_or_else(
                || (None, vec![session.source]),
                |(target, members)| (Some(target), members),
            );
        if target == session.target && members == session.members {
            return;
        }
        session.target = target;
        session.members = members;
        cx.notify();
    }
}

impl Session {
    /// Resolve edges against real painted windows, excluding the grabbed leaf and central merge zone.
    fn hit(
        &self,
        pointer: Point<Pixels>,
        area: &DockArea,
        frames: &HashMap<NodeId, Bounds<Pixels>>,
        cx: &App,
    ) -> Option<(Target, Vec<PanelId>)> {
        for region in REGIONS {
            if region != DockPlacement::Center && !area.is_dock_open(region) {
                continue;
            }
            let Some(tree) = area.layout(region) else {
                continue;
            };
            for id in tree.panels().filter(|id| *id != self.source) {
                if !area.panel(id).is_some_and(|panel| panel.visible(cx)) {
                    continue;
                }
                let Some(bounds) = tree
                    .find_panel_node(id)
                    .and_then(|node| frames.get(&node))
                    .copied()
                else {
                    continue;
                };
                if bounds.contains(&pointer) {
                    let axis = if Some(id) == self.anchor {
                        None
                    } else {
                        placement::drop_axis(
                            area,
                            tree.find_panel_node(id)?,
                            self.source,
                            self.anchor,
                            cx,
                        )
                    };
                    if Some(id) != self.anchor
                        && [self.source, id].into_iter().any(|panel| {
                            area.panel(panel)
                                .is_some_and(|view| super::presentation::is_exclusive(view, cx))
                        })
                    {
                        let bounds =
                            self.original
                                .replacement_bounds(area, id, self.source, frames, cx)?;
                        let stacking = axis.unwrap_or(Axis::Horizontal);
                        return Some((
                            Target {
                                panel: id,
                                placement: if stacking == Axis::Horizontal {
                                    Placement::Right
                                } else {
                                    Placement::Bottom
                                },
                                bounds,
                                stacking,
                                mode: DropMode::Replace,
                            },
                            vec![self.source],
                        ));
                    }
                    let placement = placement::edge_at(bounds, pointer, axis)?;
                    let members = if Some(id) == self.anchor {
                        self.editor_members(area, id, placement, cx)?
                    } else {
                        vec![self.source]
                    };
                    return Some((
                        Target {
                            panel: id,
                            placement,
                            bounds,
                            stacking: axis.unwrap_or_else(|| {
                                if placement.axis() == Axis::Horizontal {
                                    Axis::Vertical
                                } else {
                                    Axis::Horizontal
                                }
                            }),
                            mode: DropMode::Split,
                        },
                        members,
                    ));
                }
            }
        }
        None
    }

    /// Prefer one window on a free edge. A blocked own-side edge may instead carry its whole band.
    /// Promotion succeeds only if excluding every candidate leaves that side empty; an unrelated
    /// region still blocks it. `band` is a leaf for nested children, so they can never promote peers.
    fn editor_members(
        &self,
        area: &DockArea,
        anchor: PanelId,
        side: Placement,
        cx: &App,
    ) -> Option<Vec<PanelId>> {
        if placement::editor_side_available(area, anchor, &[self.source], side, cx) {
            return Some(vec![self.source]);
        }
        let members = self.original.members(&self.band);
        (members.len() > 1 && placement::editor_side_available(area, anchor, &members, side, cx))
            .then_some(members)
    }

    /// Measure the current moving set's frozen outer allocation, including titles and group gaps.
    fn source_bounds(&self, cx: &App) -> Option<Bounds<Pixels>> {
        if self.members.len() > 1 {
            self.original.bounds(&self.band, cx)
        } else {
            self.original.panel_bounds(self.source)
        }
    }

    /// Retain actual pre-drag width/height; reaching or exceeding the whole target falls back to half.
    fn thickness(&self, target: Target, cx: &App) -> Pixels {
        let original = self
            .source_bounds(cx)
            .map(|bounds| bounds.size)
            .unwrap_or(target.bounds.size);
        let (desired, available) = if target.placement.axis() == Axis::Horizontal {
            (original.width, target.bounds.size.width)
        } else {
            (original.height, target.bounds.size.height)
        };
        if desired >= available {
            available / 2.
        } else {
            desired
        }
    }

    /// Return the blue group's allocation inside the unchanged target window.
    fn destination(&self, cx: &App) -> Option<Bounds<Pixels>> {
        let target = self.target?;
        if target.mode == DropMode::Replace {
            return Some(target.bounds);
        }
        let thickness = self.thickness(target, cx);
        let mut bounds = target.bounds;
        match target.placement {
            Placement::Left => bounds.size.width = thickness,
            Placement::Right => {
                bounds.origin.x = bounds.right() - thickness;
                bounds.size.width = thickness;
            }
            Placement::Top => bounds.size.height = thickness,
            Placement::Bottom => {
                bounds.origin.y = bounds.bottom() - thickness;
                bounds.size.height = thickness;
            }
        }
        Some(bounds)
    }

    /// Reorient the first-level group while preserving every nested child's original layout and ratios.
    fn rectangles(&self, destination: Bounds<Pixels>, cx: &App) -> Vec<(PanelId, Bounds<Pixels>)> {
        let Some(target) = self.target else {
            return Vec::new();
        };
        self.original.group_rectangles(
            &self.band,
            &self.members,
            destination,
            Some(target.stacking),
            cx,
        )
    }
}
