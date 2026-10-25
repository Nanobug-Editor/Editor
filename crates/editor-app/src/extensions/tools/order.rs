//! Workspace ordering merges visible moves with retained, temporarily unavailable window slots.

use super::*;
use editor_ui::reorder_group::ReorderEvent;

impl EditorApp {
    /// Commit a validated move in the supplied visible snapshot, preserving hidden plugin slots.
    /// `before = None` means after that snapshot's current final member, not after every overflow entry.
    /// Returns false when the source or a captured member is no longer available.
    pub(super) fn reorder_window_buttons(
        &mut self,
        event: &ReorderEvent,
        snapshot: &[String],
        cx: &mut Context<Self>,
    ) -> bool {
        // Only identities are needed at commit; artwork remains the shell's rendering choice.
        let entries = self.window_entries(Icon::default(), cx);
        let current: Vec<_> = entries.iter().map(|entry| entry.id.clone()).collect();
        // A package lifecycle change invalidates a captured gesture; do not save a stale list.
        if !snapshot.iter().all(|id| current.contains(id))
            || !snapshot.iter().any(|id| id == event.source.as_ref())
            || event.before.as_ref().is_some_and(|before| {
                before == &event.source || !snapshot.iter().any(|id| id == before.as_ref())
            })
        {
            return false;
        }
        let mut order = self.session_state.window_button_order.clone();
        let mut seen = std::collections::HashSet::new();
        order.retain(|id| seen.insert(id.clone()));
        for id in &current {
            if seen.insert(id.clone()) {
                order.push(id.clone());
            }
        }
        order.retain(|id| id != event.source.as_ref());
        let index = if let Some(before) = &event.before {
            order.iter().position(|id| id == before.as_ref())
        } else {
            // A popup may have accepted earlier moves without reopening. Resolve its end
            // from the current canonical projection, not the order captured on first open.
            current
                .iter()
                .rev()
                .find(|id| id.as_str() != event.source.as_ref() && snapshot.contains(id))
                .and_then(|last| {
                    order
                        .iter()
                        .position(|id| id == last)
                        .map(|index| index + 1)
                })
        };
        let Some(index) = index else { return false };
        order.insert(index, event.source.to_string());
        if order != self.session_state.window_button_order {
            self.session_state.window_button_order = order;
            self.persist_session();
            cx.notify();
        }
        true
    }
}
