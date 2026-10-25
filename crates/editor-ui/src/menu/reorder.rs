//! Opt-in menu ordering reuses the native long-press group without changing menu activation.

use super::*;
use crate::reorder_group::{ReorderEvent, ReorderGroup, ReorderGroupState};

/// A popup owns its transient drag state; the caller owns durable order and membership.
pub(super) struct ReorderRows {
    /// Closing the popup cancels a pending hold or active native drag.
    pub(super) state: ReorderGroupState,
    /// Notify the owner only after a valid drop; menu selection does not fire for this gesture.
    sink: Rc<dyn Fn(&ReorderEvent, &mut Window, &mut App) -> bool>,
}

impl PopupMenu {
    /// Enable long-press ordering of plain rows; `sink` receives stable source and next-row IDs.
    /// Return true after accepting the move; false dismisses a stale list without changing its rows.
    /// This mode is intended for an unfiltered list without headings, footers or secondary actions.
    pub fn reorderable(
        mut self,
        sink: impl Fn(&ReorderEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.reorder = Some(ReorderRows {
            state: Default::default(),
            sink: Rc::new(sink),
        });
        self
    }

    /// Build a vertical gesture surface; the existing popup continues to own focus and scrolling.
    pub(super) fn reorder_group(&self, cx: &Context<Self>) -> Option<ReorderGroup> {
        let reorder = self.reorder.as_ref()?;
        Some(
            ReorderGroup::new("menu-row-order", reorder.state.clone())
                .vertical()
                .w_full()
                .on_reorder(cx.listener(|this, event: &ReorderEvent, window, cx| {
                    let sink = this.reorder.as_ref().unwrap().sink.clone();
                    if !sink(event, window, cx) {
                        this.finish(Action::Dismiss, window, cx);
                        return;
                    }
                    let Some(source) = this
                        .items
                        .iter()
                        .position(|item| item.id == event.source.as_ref())
                    else {
                        return;
                    };
                    if event.before.as_ref().is_some_and(|before| {
                        !this.items.iter().any(|item| item.id == before.as_ref())
                    }) {
                        return;
                    }
                    let selected = this
                        .selected
                        .and_then(|index| this.items.get(index))
                        .map(|item| item.id.clone());
                    let item = this.items.remove(source);
                    let target = event
                        .before
                        .as_ref()
                        .and_then(|before| {
                            this.items
                                .iter()
                                .position(|item| item.id == before.as_ref())
                        })
                        .unwrap_or(this.items.len());
                    this.items.insert(target, item);
                    this.selected =
                        selected.and_then(|id| this.items.iter().position(|item| item.id == id));
                    cx.notify();
                })),
        )
    }
}
