//! Native pointer selection honors the resource's immutable public gesture preference.
use super::*;

impl TreeView {
    /// Select `id` only on its admitted pointer gesture; `click_count` comes from the native event.
    /// Focus, keyboard behavior and disclosure remain in Base. Status rows retain ordinary pointer
    /// selection for retry/paging even when provider rows require a double click; disabled rows stay inert.
    pub(super) fn press_row(
        &mut self,
        id: &str,
        click_count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let status = self.sentinels.contains_key(id);
        if !status && self.snapshot.nodes.get(id).is_none_or(|node| node.disabled) {
            return;
        }
        self.resource.dismiss_initial_selection();
        if self.resource.single_click_expand()
            && self.snapshot.nodes.get(id).is_some_and(|node| node.branch)
        {
            self.tree.update(cx, |tree, cx| tree.focus(window, cx));
            // The second native press belongs to the same double-click sequence and must not undo
            // the directory toggle performed by its first press.
            if click_count == 1 {
                self.toggle(id, window, cx);
            }
            return;
        }
        let double = self.resource.selection_gesture() == api::SelectionGesture::DoubleClick;
        let select = status || !double || click_count == 2;
        let same = self.selected.as_deref() == Some(id);
        self.tree.update(cx, |tree, cx| {
            if select {
                tree.set_selected_index(tree.index_of(&id.to_string().into()), cx);
            }
            tree.focus(window, cx);
        });
        // The native observer reports new IDs. A deliberate reselection still needs one event,
        // including a double click on an initially highlighted default that has never filtered.
        if select && same && (click_count != 2 || double) {
            self.emit(
                api::Event::Selected {
                    tree: self.resource.handle().clone(),
                    epoch: self.snapshot.epoch,
                    node: id.into(),
                },
                cx,
            );
        }
        if click_count == 2 {
            self.activate(id, window, cx);
        }
    }
}
