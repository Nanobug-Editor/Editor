//! Only native item changes replace Base's item collection; cached menu/icon updates just repaint.
use super::TreeSnapshot;

/// Compare the fields consumed by `TreeItem` and its synthetic loading/retry/paging rows.
/// Resource epochs and node menus affect action authority, not the native selection/scroll model.
pub(super) fn items_changed(before: &TreeSnapshot, after: &TreeSnapshot) -> bool {
    if before.expanded != after.expanded
        || before.nodes.len() != after.nodes.len()
        || before.branches.len() != after.branches.len()
    {
        return true;
    }
    if before.nodes.iter().any(|(id, old)| {
        after.nodes.get(id).is_none_or(|new| {
            old.branch != new.branch || old.label != new.label || old.disabled != new.disabled
        })
    }) {
        return true;
    }
    before.branches.iter().any(|(parent, old)| {
        after.branches.get(parent).is_none_or(|new| {
            old.children != new.children
                || old.next != new.next
                || old.loaded != new.loaded
                || old.loading != new.loading
                || old.error != new.error
        })
    })
}
