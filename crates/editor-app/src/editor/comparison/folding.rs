//! Generic persistent comparison controls consume only their issuer's admitted package artwork.
use super::*;
use editor_ui::aligned_editor::UnchangedFolds;
use std::sync::Arc;

/// One view's retained folding choices and immutable provider artwork, released with that view.
#[derive(Clone)]
pub(super) struct FoldControls {
    /// Checked public descriptor allows a same-view update to retain manual fold choices.
    descriptor: api::ComparisonFolding,
    /// Both light and dark variants have already passed owned-path, byte and SVG admission.
    collapse: [Arc<[u8]>; 2],
    expand: [Arc<[u8]>; 2],
    /// The aligned display shares only slot ranges and these choices, never an editable buffer.
    model: UnchangedFolds,
    /// Last global all-regions choice applied to this model; individual disclosures do not alter it.
    collapsed: bool,
}

impl FoldControls {
    /// Identical provider controls can retain their already mounted row/folding plan.
    pub(super) fn same_descriptor(&self, other: &Self) -> bool {
        self.descriptor == other.descriptor && self.collapsed == other.collapsed
    }

    /// Freeze validated request artwork, failing before replacing an existing comparison.
    pub(super) fn admit(
        descriptor: &api::ComparisonFolding,
        request: &plugin_runtime::EditorRequest,
        collapsed: bool,
    ) -> Result<Self, api::Failure> {
        let resolve = |icon: &plugin_runtime::plugin_protocol::ui::ToolIcon| -> Result<[Arc<[u8]>; 2], api::Failure> {
            let bytes = |path: &String| request.comparison_icons().get(path).cloned().ok_or_else(|| {
                api::Failure::new(api::ErrorCode::PermissionDenied, "Comparison artwork was not admitted")
            });
            Ok([bytes(&icon.light)?, bytes(&icon.dark)?])
        };
        let expand = resolve(&descriptor.expand)?;
        Ok(Self {
            descriptor: descriptor.clone(),
            collapse: resolve(&descriptor.collapse)?,
            model: UnchangedFolds::new(descriptor.context_lines, expand.clone())
                .initially_collapsed(collapsed),
            collapsed,
            expand,
        })
    }

    /// Keep the provider's actual theme artwork; ordinary host icon paths are not substituted.
    pub(super) fn icon(&self, collapsed: bool, dark: bool) -> Icon {
        Icon::default().data(
            &if collapsed {
                &self.collapse
            } else {
                &self.expand
            }[usize::from(dark)],
        )
    }
}

impl EditorApp {
    /// Apply a successful profile preference change to this window's active comparison.
    pub(crate) fn apply_comparison_fold_preference(
        &mut self,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) {
        if self.comparison_collapsed != collapsed {
            self.comparison_collapsed = collapsed;
            self.mount_comparison_folding(cx);
            cx.notify();
        }
    }

    /// Resolve controls only from the current live retained comparison owner.
    pub(super) fn comparison_folding(&self) -> Option<&FoldControls> {
        let id = self.active_comparison.as_ref()?;
        self.comparison_views
            .iter()
            .find(|view| &view.id == id && view.authority.is_live())?
            .folding
            .as_ref()
    }

    /// Attach choices after exact source admission; switching tabs restores the same view model.
    pub(super) fn mount_comparison_folding(&mut self, cx: &mut App) {
        if let Some(controls) = self
            .comparison_views
            .iter_mut()
            .find(|view| Some(&view.id) == self.active_comparison.as_ref())
            .and_then(|view| view.folding.as_mut())
            && controls.collapsed != self.comparison_collapsed
        {
            controls.collapsed = self.comparison_collapsed;
            controls.model =
                UnchangedFolds::new(controls.descriptor.context_lines, controls.expand.clone())
                    .initially_collapsed(controls.collapsed);
        }
        if let (Some(controls), Some(comparison)) =
            (self.comparison_folding(), &self.document_comparison)
        {
            comparison.alignment.set_folding(controls.model.clone(), cx);
        }
    }

    /// Apply `collapsed` only while that action can change the current authorized display model.
    pub(super) fn fold_comparison_unchanged(&mut self, collapsed: bool, cx: &mut Context<Self>) {
        if self.comparison_folding().is_some_and(|controls| {
            let (collapse, expand) = controls.model.available();
            if collapsed { collapse } else { expand }
        }) && self.document_comparison.is_some()
        {
            if self.comparison_collapsed != collapsed
                && let Some(profile) = self
                    .session_state
                    .file_path()
                    .and_then(|path| path.parent().map(Path::to_owned))
                && let Err(error) =
                    app::editor_preferences::set_comparison_collapsed(&profile, collapsed, cx)
            {
                self.report_host_message(
                    app::messages::MessageLevel::Warning,
                    error.to_string(),
                    cx,
                );
                return;
            }
            self.comparison_collapsed = collapsed;
            if let Some(controls) = self
                .comparison_views
                .iter_mut()
                .find(|view| Some(&view.id) == self.active_comparison.as_ref())
                .and_then(|view| view.folding.as_mut())
            {
                controls.collapsed = collapsed;
            }
            let comparison = self.document_comparison.as_ref().unwrap();
            comparison.alignment.fold_unchanged(collapsed, cx);
            cx.notify();
        }
    }

    /// Keep both actions visible, disabling each independently from current per-region choices.
    /// Individual disclosures and source navigation update the same retained availability model.
    pub(super) fn comparison_fold_buttons(
        &self,
        cx: &mut Context<Self>,
    ) -> Vec<gpui_kit::AnyElement> {
        let Some(controls) = self.comparison_folding() else {
            return Vec::new();
        };
        let available = controls.model.available();
        [
            (true, "diff-collapse-unchanged", available.0),
            (false, "diff-expand-unchanged", available.1),
        ]
        .into_iter()
        .map(|(collapsed, id, enabled)| {
            let label = if collapsed {
                t!("editor.diff_collapse_unchanged")
            } else {
                t!("editor.diff_expand_unchanged")
            };
            Button::new(id)
                .debug_selector(move || id.into())
                .icon(controls.icon(collapsed, cx.theme().is_dark()))
                .tooltip(label.to_string())
                .small()
                .compact()
                .ghost()
                .disabled(!enabled)
                .on_click(cx.listener(move |this, _, _, cx| {
                    // Recheck the live view; stale availability never authorizes a no-op fold.
                    this.fold_comparison_unchanged(collapsed, cx);
                }))
                .into_any_element()
        })
        .collect()
    }
}

/// Retain manual choices when a provider republishes the same descriptor under its existing key.
pub(super) fn retain_choices(
    prior: Option<&FoldControls>,
    mut admitted: FoldControls,
) -> FoldControls {
    if let Some(prior) = prior.filter(|prior| prior.same_descriptor(&admitted)) {
        admitted.model = prior.model.clone();
    }
    admitted
}
