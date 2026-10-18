//! Validate deferred scroll proposals without reading their source owner during native paint.
use super::*;

impl PluginView {
    /// A painted proposal belongs to one scene, membership and prepared source version. Check the
    /// authoritative owner only after its outer update has ended, before any native scroll write.
    /// A stale source revokes all legacy native maps and pending locators; readonly peers cannot
    /// continue following a retired source merely because they still have valid pixel geometry.
    pub(super) fn code_scroll_authorized(
        &mut self,
        generation: u64,
        revision: u64,
        source: &Option<DocumentVersion>,
        cx: &mut Context<Self>,
    ) -> bool {
        if generation != self.code_sync.generation
            || revision != self.document.revision
            || !self.scene_current.get()
            || self.code_sync.source.as_ref().map(|(source, _)| source) != source.as_ref()
        {
            return false;
        }
        let legacy_native = self.code_sync.groups.values().any(|group| {
            group.maps.is_some() && group.panes.iter().any(|pane| pane.stamp.native.is_some())
        });
        if !legacy_native {
            return true;
        }
        let current = source.as_ref().is_some_and(|source| {
            self.action_source_current
                .as_ref()
                .is_some_and(|current| current(source, cx))
        });
        if !current {
            for group in self.code_sync.groups.values_mut().filter(|group| {
                group.maps.is_some() && group.panes.iter().any(|pane| pane.stamp.native.is_some())
            }) {
                group.pending.iter_mut().for_each(|pending| *pending = None);
                group.maps = None;
            }
            cx.notify();
        }
        current
    }
}
