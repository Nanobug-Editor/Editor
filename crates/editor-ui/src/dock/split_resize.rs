//! Resize visible neighbors through Base's public tree edit without removing retained hidden slots.
//! Base 0.8's handle indexes the preceding stored slot, even when that slot is hidden. Only splits
//! containing hidden children use this local input adapter; ordinary Base split behavior is retained.
//! Divider paint also follows visible siblings, so hidden leading slots cannot add an outer edge.

use super::*;
use gpui_base::dock::{PaneNode, PaneRef};
use gpui_kit::{
    Along, Axis, CursorStyle, HitboxBehavior, KeyDownEvent, MouseButton, MouseDownEvent, Point,
};

/// Geometry is frozen at press time; Base remains the sole owner of all live and persisted sizes.
#[derive(Clone)]
struct Gesture {
    /// Retained Base identities reject a structural edit made during the gesture.
    node: NodeId,
    children: Vec<NodeId>,
    /// Visibility changes also invalidate the original neighboring pair.
    shown: Vec<bool>,
    axis: Axis,
    /// Actual press-time allocations; hidden slots contribute zero without losing their preferences.
    sizes: Vec<Pixels>,
    pair: [usize; 2],
    /// Preserve the exact grab offset rather than snapping the divider to the pointer.
    origin: Pixels,
    /// Suppress duplicate size edits while the pointer is stationary or clamped.
    last: Vec<Pixels>,
}

/// One window can resize only one split at a time, including moves beyond its original bounds.
#[derive(Clone, Default)]
pub(super) struct SplitResize(Rc<RefCell<Option<Gesture>>>);

impl SplitResize {
    /// Gate host reflow and persistence until the final public layout event.
    pub fn active(&self) -> bool {
        self.0.borrow().is_some()
    }

    /// Escape restores the original measured allocation through the same public size edit.
    pub fn capture_escape(
        &self,
        frame: Stateful<Div>,
        area: WeakEntity<DockArea>,
    ) -> Stateful<Div> {
        let this = self.clone();
        frame.capture_key_down(move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape" && this.active() {
                let _ = area.update(cx, |area, cx| this.finish(false, area, window, cx));
                cx.stop_propagation();
            }
        })
    }

    /// Deferred hitboxes use this frame's leaf bounds and remain below menus and other overlays.
    pub fn surface(
        &self,
        node: NodeId,
        area: WeakEntity<DockArea>,
        frames: PaintedFrames,
    ) -> AnyElement {
        let measured_area = area.clone();
        let this = self.clone();
        canvas(
            move |clip, window, cx| {
                let Some(area) = measured_area.upgrade() else {
                    return Vec::new();
                };
                handles(node, area.read(cx), &frames.panels.borrow(), cx)
                    .into_iter()
                    .filter_map(|(bounds, gesture)| {
                        let bounds = bounds.intersect(&clip);
                        (bounds.size.width > px(0.) && bounds.size.height > px(0.)).then(|| {
                            (
                                window.insert_hitbox(bounds, HitboxBehavior::Normal),
                                gesture,
                            )
                        })
                    })
                    .collect::<Vec<_>>()
            },
            move |_, handles, window, _| {
                for (hitbox, gesture) in &handles {
                    window.set_cursor_style(
                        if gesture.axis == Axis::Horizontal {
                            CursorStyle::ResizeColumn
                        } else {
                            CursorStyle::ResizeRow
                        },
                        hitbox,
                    );
                }
                let started = this.clone();
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                    if !phase.bubble()
                        || event.button != MouseButton::Left
                        || started.active()
                        || cx.has_active_drag()
                    {
                        return;
                    }
                    if let Some((_, gesture)) = handles
                        .iter()
                        .find(|(hitbox, _)| hitbox.is_hovered_at(event.position, window))
                    {
                        let mut gesture = gesture.clone();
                        gesture.origin = event.position.along(gesture.axis);
                        *started.0.borrow_mut() = Some(gesture);
                        cx.stop_propagation();
                    }
                });
            },
        )
        .absolute()
        .inset_0()
        .into_any_element()
    }

    /// The root owns continuation so hiding or removing the originating split still retires input.
    pub fn tracker(&self, area: WeakEntity<DockArea>) -> AnyElement {
        let this = self.clone();
        canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                let moved = this.clone();
                let moved_area = area.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase.capture() && moved.active() {
                        let _ = moved_area.update(cx, |area, cx| {
                            if event.pressed_button == Some(MouseButton::Left) {
                                moved.resize(event.position, area, window, cx);
                            } else {
                                moved.finish(true, area, window, cx);
                            }
                        });
                        cx.stop_propagation();
                    }
                });
                let ended = this.clone();
                let ended_area = area.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                    if phase.capture() && ended.active() && event.button == MouseButton::Left {
                        let _ = ended_area.update(cx, |area, cx| {
                            ended.resize(event.position, area, window, cx);
                            ended.finish(true, area, window, cx);
                        });
                        cx.stop_propagation();
                    }
                });
            },
        )
        .absolute()
        .inset_0()
        .into_any_element()
    }

    /// Each move applies an absolute delta to the original pair, avoiding accumulated rounding drift.
    fn resize(
        &self,
        pointer: Point<Pixels>,
        area: &mut DockArea,
        window: &mut Window,
        cx: &mut Context<DockArea>,
    ) {
        let mut state = self.0.borrow_mut();
        let Some(gesture) = state.as_mut() else {
            return;
        };
        if !gesture.matches(area, cx) {
            state.take();
            drop(state);
            cx.emit(DockEvent::LayoutChanged);
            return;
        }
        let [before, after] = gesture.pair;
        let minimum = gpui_base::PANEL_MIN_SIZE;
        // A previously undersized slot must remain reachable without jumping on the first move.
        let delta = (pointer.along(gesture.axis) - gesture.origin).clamp(
            minimum.min(gesture.sizes[before]) - gesture.sizes[before],
            gesture.sizes[after] - minimum.min(gesture.sizes[after]),
        );
        let mut sizes = gesture.sizes.clone();
        sizes[before] += delta;
        sizes[after] -= delta;
        if sizes == gesture.last {
            return;
        }
        gesture.last = sizes.clone();
        let node = gesture.node;
        // Release before synchronous observers inspect the gesture's persistence gate.
        drop(state);
        area.set_split_sizes(node, sizes, window, cx);
    }

    /// Retire input state before the final event so the host persists one complete allocation.
    fn finish(
        &self,
        commit: bool,
        area: &mut DockArea,
        window: &mut Window,
        cx: &mut Context<DockArea>,
    ) {
        let Some(gesture) = self.0.borrow_mut().take() else {
            return;
        };
        if !gesture.matches(area, cx) {
            cx.emit(DockEvent::LayoutChanged);
            return;
        }
        if !commit && gesture.last != gesture.sizes {
            area.set_split_sizes(gesture.node, gesture.sizes, window, cx);
        } else if gesture.last != gesture.sizes {
            cx.emit(DockEvent::LayoutChanged);
        }
    }
}

impl Gesture {
    /// A visibility or structural edit invalidates the captured neighbors rather than resizing new ones.
    fn matches(&self, area: &DockArea, cx: &App) -> bool {
        if area.is_locked() || area.is_zoomed() {
            return false;
        }
        let Some(node) = find_node(area, self.node) else {
            return false;
        };
        let PaneRef::Split { axis, children, .. } = node.kind() else {
            return false;
        };
        axis == self.axis
            && children.len() == self.children.len()
            && children
                .iter()
                .zip(&self.children)
                .all(|(child, id)| child.id() == *id)
            && children
                .iter()
                .zip(&self.shown)
                .all(|(child, shown)| visible(child, area, cx) == *shown)
    }
}

/// Find a retained split by identity without relying on its current dock placement.
fn find_node(area: &DockArea, node: NodeId) -> Option<&PaneNode> {
    [
        DockPlacement::Center,
        DockPlacement::Left,
        DockPlacement::Right,
        DockPlacement::Bottom,
    ]
    .into_iter()
    .find_map(|placement| area.layout(placement)?.find_node(node))
}

/// Hidden nested groups have no visible leaf allocation and must not contribute stale frames.
fn visible(node: &PaneNode, area: &DockArea, cx: &App) -> bool {
    match node.kind() {
        PaneRef::Tabs { panels, .. } => panels
            .iter()
            .any(|id| area.panel(*id).is_some_and(|panel| panel.visible(cx))),
        PaneRef::Split { children, .. } => children.iter().any(|child| visible(child, area, cx)),
    }
}

/// Visible leaf frames include their titles, so a nested group is their enclosing allocation.
fn bounds(
    node: &PaneNode,
    area: &DockArea,
    frames: &HashMap<NodeId, Bounds<Pixels>>,
    cx: &App,
) -> Option<Bounds<Pixels>> {
    if !visible(node, area, cx) {
        return None;
    }
    match node.kind() {
        PaneRef::Tabs { .. } => frames.get(&node.id()).copied(),
        PaneRef::Split { children, .. } => children
            .iter()
            .filter_map(|child| bounds(child, area, frames, cx))
            .reduce(|a, b| a.union(&b)),
    }
}

/// Skin Base's handle without changing its hitbox, drag state or one-logical-pixel extent.
/// `area` supplies live visibility; `frames` is read during paint, after all leaves were measured.
pub(super) fn separator(
    handle: &gpui_base::ResizeHandleContext,
    area: WeakEntity<DockArea>,
    frames: PaintedFrames,
) -> AnyElement {
    let axis = handle.axis();
    let active = handle.is_active();
    canvas(
        |_, _, _| (),
        move |line, _, window, cx| {
            let Some(area) = area.upgrade() else { return };
            let area = area.read(cx);
            let frames = frames.panels.borrow();
            let mut shared = false;
            for placement in [
                DockPlacement::Center,
                DockPlacement::Left,
                DockPlacement::Right,
                DockPlacement::Bottom,
            ] {
                if let Some(tree) = area.layout(placement) {
                    tree.root().walk(&mut |node| {
                        if let PaneRef::Split {
                            axis: split_axis,
                            children,
                            ..
                        } = node.kind()
                            && split_axis == axis
                        {
                            let shown: Vec<_> = children
                                .iter()
                                .filter_map(|child| bounds(child, area, &frames, cx))
                                .collect();
                            shared |= shown.windows(2).any(|pair| {
                                let following = pair[1];
                                // Match the entire edge, not just its coordinate: nested splits
                                // can share an origin while spanning different rows or columns.
                                (following.origin.x - line.origin.x).abs() < px(0.01)
                                    && (following.origin.y - line.origin.y).abs() < px(0.01)
                                    && if axis == Axis::Horizontal {
                                        (following.size.height - line.size.height).abs() < px(0.01)
                                    } else {
                                        (following.size.width - line.size.width).abs() < px(0.01)
                                    }
                            });
                        }
                    });
                }
            }
            if shared {
                let theme = gpui_base::Theme::global(cx);
                let color = if active {
                    theme
                        .resizable
                        .active_handle
                        .unwrap_or(theme.tokens.colors.ring)
                } else {
                    theme.resizable.handle.unwrap_or(theme.tokens.colors.border)
                };
                window.paint_quad(gpui_kit::fill(line, color));
            }
        },
    )
    .flex_none()
    .when(axis == Axis::Horizontal, |line| line.w(px(1.)).h_full())
    .when(axis == Axis::Vertical, |line| line.w_full().h(px(1.)))
    .into_any_element()
}

/// Paint-time geometry skips hidden slots while retaining their indexes in the public size vector.
fn handles(
    node: NodeId,
    area: &DockArea,
    frames: &HashMap<NodeId, Bounds<Pixels>>,
    cx: &App,
) -> Vec<(Bounds<Pixels>, Gesture)> {
    if area.is_locked() || area.is_zoomed() {
        return Vec::new();
    }
    let Some(split) = find_node(area, node) else {
        return Vec::new();
    };
    let PaneRef::Split { axis, children, .. } = split.kind() else {
        return Vec::new();
    };
    let shown: Vec<_> = children
        .iter()
        .map(|child| visible(child, area, cx))
        .collect();
    if shown.iter().all(|shown| *shown) {
        return Vec::new();
    }
    let measured: Vec<_> = children
        .iter()
        .map(|child| bounds(child, area, frames, cx))
        .collect();
    if shown
        .iter()
        .zip(&measured)
        .any(|(shown, bounds)| *shown && bounds.is_none())
    {
        return Vec::new();
    }
    let sizes: Vec<_> = measured
        .iter()
        .map(|bounds| bounds.map_or(px(0.), |bounds| bounds.size.along(axis)))
        .collect();
    let visible: Vec<_> = shown
        .iter()
        .enumerate()
        .filter_map(|(index, shown)| shown.then_some(index))
        .collect();
    visible
        .windows(2)
        .map(|pair| {
            let mut hit = measured[pair[1]].unwrap();
            match axis {
                Axis::Horizontal => {
                    hit.origin.x -= px(4.);
                    hit.size.width = px(8.);
                }
                Axis::Vertical => {
                    hit.origin.y -= px(4.);
                    hit.size.height = px(8.);
                }
            }
            (
                hit,
                Gesture {
                    node,
                    children: children.iter().map(PaneNode::id).collect(),
                    shown: shown.clone(),
                    axis,
                    last: sizes.clone(),
                    sizes: sizes.clone(),
                    pair: [pair[0], pair[1]],
                    origin: px(0.),
                },
            )
        })
        .collect()
}
