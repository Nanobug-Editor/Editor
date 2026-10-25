//! Gesture state and geometry shared by the local group and GPUI's native drag preview.

use gpui_kit::{App, Bounds, Pixels, Point, SharedString, Subscription, Window, point, px, size};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

/// Move `source` immediately before `before`, or to the end of the rendered list for `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReorderEvent {
    /// Stable ID of the pressed item.
    pub source: SharedString,
    /// Stable successor ID after removing the source; omitted at the visible list's end.
    pub before: Option<SharedString>,
}

/// Cloneable retained state for one mounted group; separate groups need separate states.
#[derive(Clone, Default)]
pub struct ReorderGroupState(pub(super) Rc<RefCell<State>>);

/// The current painted list is authoritative only for gesture validation, never persistence.
#[derive(Default)]
pub(super) struct State {
    /// Canonical visible order for this paint, used to reject unchanged drops.
    pub ids: Vec<SharedString>,
    /// Full measured item bounds preserve original midpoints for partly scrolled rows.
    pub item_bounds: BTreeMap<SharedString, Bounds<Pixels>>,
    /// The only area in which a release may commit an insertion.
    pub bounds: Bounds<Pixels>,
    /// Unclipped layout origin keeps the insertion line aligned after viewport scrolling.
    pub origin: Point<Pixels>,
    /// Choose the Y coordinate instead of X for insertion comparisons.
    pub vertical: bool,
    /// One pointer press, retained after cancellation until its release is swallowed.
    pub gesture: Option<Gesture>,
    /// Monotonic press generation that invalidates old delayed timers.
    pub next_token: u64,
}

/// A cancelled press is retained through release so it cannot activate a native child button.
pub(super) struct Gesture {
    /// Stable identity of the originally pressed child.
    pub source: SharedString,
    /// Initial position for rejecting movement before the hold threshold.
    pub start: Point<Pixels>,
    /// Latest pointer position, refreshed again from the release event before commit.
    pub position: Point<Pixels>,
    /// Generation captured by this press's delayed arming task.
    pub token: u64,
    /// True after the uninterrupted 350 ms hold.
    pub armed: bool,
    /// True only once GPUI has created this group's native drag preview.
    pub dragging: bool,
    /// A revoked press may no longer drag or activate its child.
    pub cancelled: bool,
    /// Window-scoped Escape interception exists only for this live pointer press.
    pub escape: Option<Subscription>,
}

impl ReorderGroupState {
    /// Cancel this group's press/drag when its owner closes or removes the group.
    /// Native drag cleanup affects only a drag actually started by this state.
    pub fn cancel(&self, window: &mut Window, cx: &mut App) {
        let was_dragging = {
            let mut state = self.0.borrow_mut();
            let Some(gesture) = state.gesture.as_mut() else {
                return;
            };
            let was_dragging = gesture.dragging;
            gesture.cancelled = true;
            gesture.dragging = false;
            gesture.escape.take();
            was_dragging
        };
        if was_dragging {
            cx.stop_active_drag(window);
        }
        window.refresh();
    }

    /// Replacing membership or orientation revokes a press from the previous painted list.
    pub(super) fn synchronize(
        &self,
        ids: Vec<SharedString>,
        vertical: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.0.borrow().ids != ids || self.0.borrow().vertical != vertical {
            self.cancel(window, cx);
            let mut state = self.0.borrow_mut();
            state.item_bounds.clear();
            state.ids = ids;
            state.vertical = vertical;
        }
    }

    /// The native drag begins only after the capture gate has accepted the long press.
    pub(super) fn start_drag(&self, window: &mut Window) {
        if let Some(gesture) = self.0.borrow_mut().gesture.as_mut()
            && gesture.armed
            && !gesture.cancelled
        {
            gesture.dragging = true;
            gesture.position = window.mouse_position();
        }
        window.refresh();
    }
}

impl State {
    /// Empty flex space is not a drop target; gaps between actual rows/items remain usable.
    fn drop_bounds(&self) -> Option<Bounds<Pixels>> {
        let bounds = self
            .ids
            .iter()
            .filter_map(|id| self.item_bounds.get(id))
            .copied()
            .reduce(|bounds, item| bounds.union(&item))?
            .intersect(&self.bounds);
        (bounds.size.width > px(0.) && bounds.size.height > px(0.)).then_some(bounds)
    }

    /// Resolve the pointer against painted and clipped bounds, skipping the dragged source.
    pub(super) fn target(&self) -> Option<Option<SharedString>> {
        let gesture = self.gesture.as_ref()?;
        if gesture.cancelled
            || !gesture.dragging
            || !self.drop_bounds()?.contains(&gesture.position)
        {
            return None;
        }
        let coordinate = if self.vertical {
            gesture.position.y
        } else {
            gesture.position.x
        };
        Some(
            self.ids
                .iter()
                .filter(|id| **id != gesture.source)
                .find(|id| {
                    self.item_bounds.get(*id).is_some_and(|bounds| {
                        coordinate
                            < if self.vertical {
                                bounds.center().y
                            } else {
                                bounds.center().x
                            }
                    })
                })
                .cloned(),
        )
    }

    /// Return a move only when it changes the visible order; dropping over oneself is a no-op.
    pub(super) fn completed_move(&self) -> Option<ReorderEvent> {
        let before = self.target()?;
        let gesture = self.gesture.as_ref()?;
        let index = self.ids.iter().position(|id| *id == gesture.source)?;
        if self.ids.get(index + 1) == before.as_ref() {
            return None;
        }
        Some(ReorderEvent {
            source: gesture.source.clone(),
            before,
        })
    }

    /// Draw an insertion line beside the stable successor, entirely inside the group bounds.
    pub(super) fn indicator(&self) -> Option<Bounds<Pixels>> {
        let before = self.target()?;
        let visible = self.drop_bounds()?;
        let item = before.as_ref().or_else(|| self.ids.last())?;
        let bounds = self.item_bounds.get(item)?;
        if self.vertical {
            let y = if before.is_some() {
                bounds.top()
            } else {
                bounds.bottom() - px(2.)
            }
            .max(visible.top())
            .min(visible.bottom() - px(2.));
            Some(Bounds::new(
                point(visible.left(), y),
                size(visible.size.width, px(2.)),
            ))
        } else {
            let x = if before.is_some() {
                bounds.left()
            } else {
                bounds.right() - px(2.)
            }
            .max(visible.left())
            .min(visible.right() - px(2.));
            Some(Bounds::new(
                point(x, visible.top()),
                size(px(2.), visible.size.height),
            ))
        }
    }
}
