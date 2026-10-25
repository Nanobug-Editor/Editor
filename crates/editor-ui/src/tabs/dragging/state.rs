//! Pure insertion geometry and derived gesture state for the native file-tab viewport.

use gpui_kit::{Along as _, App, Axis, Bounds, Pixels, Point, Window, px};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

/// Move `source` immediately before `before`, or after the last reorderable tab for `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileTabMove {
    /// Stable identity of the dragged visible document tab.
    pub source: u64,
    /// Stable successor after removing the source; absent at the document strip's end.
    pub before: Option<u64>,
}

/// Retained state for one file-tab strip; owners call `cancel` when the strip is unmounted.
#[derive(Clone, Default)]
pub struct FileTabDragState(pub(super) Rc<RefCell<State>>);

/// Painted canonical geometry stays fixed while visual children move around the target gap.
pub(super) struct State {
    /// Layout and gesture calculations share one principal axis; text is never rotated.
    pub axis: Axis,
    /// Caller-owned visible order, used only to validate and derive a possible move.
    pub ids: Vec<u64>,
    /// Full static item allocations, including portions outside the scroll viewport.
    pub items: BTreeMap<u64, Bounds<Pixels>>,
    /// Clipped visible viewport in window coordinates; releases outside it cancel.
    pub viewport: Bounds<Pixels>,
    /// The single active native drag owned by this strip.
    pub session: Option<Session>,
    /// Next generation prevents an old preview's release from retiring a later drag.
    pub generation: u64,
    /// Escape or membership cancellation swallows the eventual release without activating tabs.
    pub suppress_release: bool,
}

/// Immutable source size and latest pointer belong to one native drag preview.
pub(super) struct Session {
    /// Stable identity of the originally grabbed tab.
    pub source: u64,
    /// Latest real pointer position, rechecked on release.
    pub pointer: Point<Pixels>,
    /// Native preview lifetime token.
    pub token: u64,
    /// Full source allocation fixes both preview size and the gap extent during a gesture.
    pub source_bounds: Bounds<Pixels>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            axis: Axis::Horizontal,
            ids: Vec::new(),
            items: BTreeMap::new(),
            viewport: Bounds::default(),
            session: None,
            generation: 0,
            suppress_release: false,
        }
    }
}

impl FileTabDragState {
    /// Whether this strip owns a live drag; canonical order remains caller-owned throughout.
    pub fn active(&self) -> bool {
        self.0.borrow().session.is_some()
    }

    /// Cancel this strip's native drag and previews in `window`/`cx`, without emitting a move.
    /// Invoke from a focused ancestor's Escape handler or when removing the entire strip.
    pub fn cancel(&self, window: &mut Window, cx: &mut App) {
        let active = self.0.borrow_mut().retire(None);
        if active {
            cx.stop_active_drag(window);
            window.refresh();
        }
    }

    /// Layout or canonical-order changes revoke old geometry; return whether the axis changed.
    pub(super) fn synchronize(
        &self,
        ids: Vec<u64>,
        axis: Axis,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let axis_changed = self.0.borrow().axis != axis;
        if axis_changed || self.0.borrow().ids != ids {
            self.cancel(window, cx);
            let mut state = self.0.borrow_mut();
            state.axis = axis;
            state.ids = ids;
            state.items.clear();
        }
        axis_changed
    }
}

impl State {
    /// Drop this generation, or any generation for `None`; cancellation never emits a move.
    pub(super) fn retire(&mut self, token: Option<u64>) -> bool {
        if self
            .session
            .as_ref()
            .is_some_and(|session| token.is_none_or(|token| token == session.token))
        {
            self.session = None;
            self.suppress_release = true;
            true
        } else {
            false
        }
    }

    /// Resolve against the canonical midpoints so moving neighbors cannot move the drop threshold.
    pub(super) fn target(&self) -> Option<Option<u64>> {
        let session = self.session.as_ref()?;
        if !self.viewport.contains(&session.pointer) {
            return None;
        }
        Some(
            self.ids
                .iter()
                .filter(|id| **id != session.source)
                .find(|id| {
                    self.items.get(id).is_some_and(|bounds| {
                        session.pointer.along(self.axis) < bounds.center().along(self.axis)
                    })
                })
                .copied(),
        )
    }

    /// Emit only a changed order, including a validated move after the final document tab.
    pub(super) fn completed(&self) -> Option<FileTabMove> {
        let before = self.target()?;
        let source = self.session.as_ref()?.source;
        let index = self.ids.iter().position(|id| *id == source)?;
        if self.ids.get(index + 1).copied() == before {
            return None;
        }
        Some(FileTabMove { source, before })
    }

    /// Hide the grabbed tab and move only neighbors crossed by its insertion position.
    pub(super) fn presentation(&self, id: u64) -> (bool, Pixels) {
        let Some(session) = self.session.as_ref() else {
            return (false, px(0.));
        };
        if id == session.source {
            return (true, px(0.));
        }
        let Some(before) = self.target() else {
            return (false, px(0.));
        };
        let source = self
            .ids
            .iter()
            .position(|id| *id == session.source)
            .unwrap_or(0);
        let target = before
            .and_then(|before| self.ids.iter().position(|id| *id == before))
            .unwrap_or(self.ids.len());
        let index = self
            .ids
            .iter()
            .position(|item| *item == id)
            .unwrap_or(self.ids.len());
        let extent = session.source_bounds.size.along(self.axis);
        let shift = if index > source && index < target {
            -extent
        } else if index >= target && index < source {
            extent
        } else {
            px(0.)
        };
        (false, shift)
    }

    /// Place the full-size source gap where insertion would put it after removing the source.
    pub(super) fn placeholder(&self) -> Option<Bounds<Pixels>> {
        let before = self.target()?;
        let session = self.session.as_ref()?;
        let source = self.ids.iter().position(|id| *id == session.source)?;
        let (leading, target) = if let Some(before) = before {
            (
                self.items.get(&before)?.origin.along(self.axis),
                self.ids.iter().position(|id| *id == before)?,
            )
        } else {
            let last = self.items.get(self.ids.last()?)?;
            (
                last.origin.along(self.axis) + last.size.along(self.axis),
                self.ids.len(),
            )
        };
        let mut gap = session.source_bounds;
        gap.origin = gap
            .origin
            .apply_along(self.axis, |_| {
                leading
                    - if target > source {
                        gap.size.along(self.axis)
                    } else {
                        px(0.)
                    }
            })
            .apply_along(self.axis.invert(), |_| {
                self.viewport.origin.along(self.axis.invert())
            });
        Some(gap)
    }
}
