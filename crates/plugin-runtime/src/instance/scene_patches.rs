//! Stage bounded retained artwork before ordinary panel, source and resource publication checks.
use super::*;
use plugin_protocol::api::{ErrorCode, Failure};
use std::collections::{BTreeMap, BTreeSet};

impl Instance {
    /// Merge UI and artwork deltas against immutable accepted views, never newly supplied artwork.
    /// A conflicting publication or invalid peer rejects the entire completion and preserves old pixels.
    pub(super) fn restore_scene_patches(&self, output: &mut api::Output) -> Result<(), Failure> {
        if output.scene_patches.is_empty() {
            return Ok(());
        }
        if self.store.data().language_pure {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Pure language callbacks cannot publish scene deltas",
            ));
        }
        if self
            .store
            .data()
            .api
            .capabilities
            .get("ui.canvas")
            .is_none_or(|version| *version < semver::Version::new(1, 10, 0))
        {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Scene deltas require ui.canvas 1.10",
            ));
        }
        let panels = output
            .views
            .iter()
            .map(|v| v.panel.as_str())
            .chain(output.view_patches.iter().map(|v| v.panel.as_str()))
            .chain(output.scene_patches.iter().map(|v| v.panel.as_str()))
            .collect::<BTreeSet<_>>();
        if panels.len() > 8
            || output.scene_patches.len() > 64
            || serde_json::to_vec(&output.scene_patches)
                .map_err(|error| Failure::new(ErrorCode::InvalidRequest, error.to_string()))?
                .len()
                > ui::MAX_SCENE_PATCH_BYTES
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Scene delta output budget exceeded",
            ));
        }
        let mut grouped = BTreeMap::<String, Vec<ui::CanvasScenePatch>>::new();
        for patch in std::mem::take(&mut output.scene_patches) {
            grouped.entry(patch.panel.clone()).or_default().push(patch);
        }
        let mut expanded = 0usize;
        for (panel, patches) in grouped {
            if !self.panels.contains(&panel) {
                return Err(Failure::new(
                    ErrorCode::PermissionDenied,
                    "Scene delta panel is not owned by this instance",
                ));
            }
            if output.views.iter().any(|v| v.panel == panel)
                || output
                    .view_patches
                    .iter()
                    .filter(|v| v.panel == panel)
                    .count()
                    > 1
            {
                return Err(Failure::new(
                    ErrorCode::Conflict,
                    "Conflicting full or duplicate UI publication",
                ));
            }
            let previous = self.views.get(&panel).ok_or_else(|| {
                Failure::new(
                    ErrorCode::StaleRevision,
                    "Scene delta has no accepted document base; resynchronize with a full View",
                )
            })?;
            let first = &patches[0];
            let mut ids = BTreeSet::new();
            if patches.iter().any(|p| {
                p.base_ui_revision != first.base_ui_revision
                    || p.next_ui_revision != first.next_ui_revision
                    || !ids.insert(p.canvas.clone())
            }) {
                return Err(Failure::new(
                    ErrorCode::Conflict,
                    "Scene deltas must share one UI transaction and distinct Canvas owners",
                ));
            }
            let mut staged = previous.as_ref().clone();
            if let Some(index) = output.view_patches.iter().position(|v| v.panel == panel) {
                if !self
                    .store
                    .data()
                    .api
                    .capabilities
                    .contains_key("ui.incremental")
                {
                    return Err(Failure::new(
                        ErrorCode::CapabilityUnavailable,
                        "Paired UI patches require ui.incremental",
                    ));
                }
                // Consume this peer here, so the ordinary patch loop cannot replay its UI revision.
                let patch = output.view_patches.remove(index);
                if patch.base_revision != first.base_ui_revision
                    || patch.document.revision != first.next_ui_revision
                    || previous.revision != patch.base_revision
                {
                    return Err(Failure::new(
                        ErrorCode::StaleRevision,
                        "Scene/UI transaction bases do not match",
                    ));
                }
                staged = patch.document;
                ui::incremental::restore(previous, &mut staged, &patch.reused)
                    .map_err(|message| Failure::new(ErrorCode::InvalidRequest, message))?;
            }
            // Outer overlay owners restore their native child before the child's independent delta.
            for patch in &patches {
                staged = patch.apply_staged_document(previous, &staged)?.document;
            }
            let mut charge = |node: &ui::Node| {
                if ids.contains(&node.id)
                    && let ui::Kind::Canvas(canvas) = &node.kind
                {
                    expanded = expanded
                        .saturating_add(serde_json::to_vec(canvas).map_or(usize::MAX, |v| v.len()));
                }
            };
            staged.root.visit(&mut charge);
            if let Some(toolbar) = &staged.editor_toolbar {
                toolbar.visit(&mut charge);
            }
            if let Some(dialog) = &staged.dialog {
                dialog.content.visit(&mut charge);
            }
            if expanded > ui::MAX_SCENE_PATCH_BYTES {
                return Err(Failure::new(
                    ErrorCode::LimitExceeded,
                    "Scene delta aggregate expansion budget exceeded",
                ));
            }
            staged
                .validate()
                .map_err(|message| Failure::new(ErrorCode::InvalidRequest, message))?;
            output.views.push(api::View {
                panel,
                document: staged,
            });
        }
        Ok(())
    }
}
