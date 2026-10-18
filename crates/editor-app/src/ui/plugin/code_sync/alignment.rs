//! Explicit 1.1 opt-in to bounded blank-row presentation over existing native source entities.
use super::*;
use crate::ui::controls::{
    aligned_editor::{AlignmentGroup, AlignmentPane},
    aligned_pointer,
};

impl Group {
    /// Content revisions retain the native gesture owner; only a different source incarnation
    /// replaces pane membership. Exact action authority is still checked by the owning renderer.
    fn same_members(&self, panes: &[Pane]) -> bool {
        self.alignment_requested
            && self.panes.len() == panes.len()
            && self.panes.iter().zip(panes).all(|(old, new)| {
                old.stamp.node == new.stamp.node
                    && old.stamp.entity == new.stamp.entity
                    && old
                        .stamp
                        .native
                        .as_ref()
                        .map(|source| (&source.id, &source.path))
                        == new
                            .stamp
                            .native
                            .as_ref()
                            .map(|source| (&source.id, &source.path))
            })
    }

    fn aligned(panes: Vec<Pane>, alignment: Option<AlignmentGroup>) -> Self {
        Self {
            pending: (0..panes.len()).map(|_| None).collect(),
            panes,
            maps: None,
            driver: Rc::new(Driver::default()),
            sequence: 0,
            anchor: None,
            sample_attempts: 0,
            geometry_failed: false,
            alignment,
            alignment_requested: true,
        }
    }
}

impl PluginView {
    /// A scene shares one comparison budget, including cached groups and native Change rebuilds.
    /// Unsupported membership or a missing root bridge remains an explicit independent fallback.
    pub(super) fn sync_aligned_code_groups(
        &mut self,
        grouped: BTreeMap<String, Vec<Pane>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut previous = std::mem::take(&mut self.code_sync.groups);
        let router = aligned_pointer::for_window(window, cx);
        for (id, panes) in grouped {
            if panes.len() < 2 {
                continue;
            }
            let old = previous.remove(&id);
            let alignment = if old.as_ref().is_some_and(|old| old.same_members(&panes)) {
                old.and_then(|old| old.alignment)
            } else {
                if let Some(group) = old.and_then(|old| old.alignment) {
                    group.retire(cx);
                }
                None
            };
            let mut alignment = alignment.or_else(|| {
                (router.is_some() && (2..=3).contains(&panes.len()))
                    .then(|| AlignmentGroup::new(cx))
            });
            let sources = panes
                .iter()
                .map(|pane| {
                    Some(AlignmentPane {
                        editor: pane.editor.clone()?,
                        revision: pane.stamp.generation,
                        label: pane.stamp.node.clone(),
                        input_factory: None,
                        native_authority_factory: None,
                    })
                })
                .collect::<Option<Vec<_>>>();
            if let (Some(group), Some(sources), Some(router)) =
                (&mut alignment, sources, router.as_ref())
            {
                // A rejected budget keeps the native fallback pane and the same source identity.
                // Subsequent source revisions can re-admit it without replacing document state.
                let _ = group.configure_shared(
                    sources,
                    router,
                    self.code_sync.alignment_budget.clone(),
                    cx,
                );
                group.refresh_native_overlays(cx);
            } else if let Some(group) = alignment.take() {
                group.retire(cx);
            }
            self.code_sync
                .groups
                .insert(id, Group::aligned(panes, alignment));
        }
        for group in previous.into_values() {
            if let Some(group) = group.alignment {
                group.retire(cx);
            }
        }
    }

    /// Borrow a local presentation without granting a new document or text mutation capability.
    /// The native source renderer remains responsible for its own SourceFrame and action gates.
    pub(in crate::ui::plugin) fn code_alignment_pane(
        &self,
        id: &str,
    ) -> Option<(AlignmentGroup, usize)> {
        self.code_sync.groups.values().find_map(|group| {
            let alignment = group.alignment.as_ref()?;
            let index = group.panes.iter().position(|pane| pane.stamp.node == id)?;
            Some((alignment.clone(), index))
        })
    }

    /// Keep preparation, budget failure and temporary native-menu pauses distinct in the UI.
    pub(in crate::ui::plugin) fn code_alignment_warning(&self, cx: &App) -> Option<&'static str> {
        use crate::ui::controls::aligned_editor::AlignmentStatus;
        self.code_sync
            .groups
            .values()
            .filter(|group| group.alignment_requested)
            .find_map(
                |group| match group.alignment.as_ref().map(|group| group.status(cx)) {
                    Some(AlignmentStatus::NativeOverlays) => {
                        Some("editor.diff_alignment_paused_overlays")
                    }
                    Some(AlignmentStatus::Disabled) => Some("editor.diff_alignment_paused_sync"),
                    Some(AlignmentStatus::Unavailable(_)) | None => {
                        Some("editor.diff_alignment_unavailable")
                    }
                    Some(AlignmentStatus::Ready | AlignmentStatus::Measuring) => None,
                },
            )
    }
}
