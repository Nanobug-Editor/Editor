//! Scene revisions and retirement remain instance-local and are checked before atomic publication.
use super::*;
use plugin_protocol::api::{ErrorCode, Failure};

/// Publication rejection is recoverable; it is distinct from traps and failed native imports.
#[derive(Debug)]
pub(super) struct RejectedScene(Failure);
impl std::fmt::Display for RejectedScene {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}
impl std::error::Error for RejectedScene {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// Preserve the typed admission reason while classifying this as an atomic publication failure.
pub(super) fn rejected(failure: Failure) -> anyhow::Error {
    RejectedScene(failure).into()
}

/// A structured scene opts into recoverable publication; legacy native documents keep their policy.
pub(super) fn has_scene(document: &ui::Document) -> bool {
    let mut found = false;
    visit(document, |node| {
        if let ui::Kind::Canvas(canvas) = &node.kind {
            found |= canvas.scene.is_some();
        }
    });
    found
}

/// Visit every declaration, including inactive modal content, without granting input authority.
pub(super) fn visit(document: &ui::Document, mut visitor: impl FnMut(&ui::Node)) {
    document.root.visit(&mut visitor);
    if let Some(toolbar) = &document.editor_toolbar {
        toolbar.visit(&mut visitor);
    }
    if let Some(dialog) = &document.dialog {
        dialog.content.visit(&mut visitor);
    }
}

impl Instance {
    /// Select an exact accepted snapshot only for a late successful native presentation receipt.
    /// Every other notification validates against the current publication as before.
    pub(crate) fn ui_event_document(
        &self,
        panel: &str,
        event: &ui::UiEvent,
    ) -> Result<std::sync::Arc<ui::Document>, Failure> {
        let current = self
            .views
            .get(panel)
            .ok_or_else(|| Failure::new(ErrorCode::InvalidHandle, "UI panel is unavailable"))?;
        let presentation = matches!(
            &event.action,
            ui::Action::Canvas(ui::CanvasEvent::ScenePresented { .. })
                | ui::Action::ViewportInput(ui::ViewportInput {
                    event: ui::CanvasEvent::ScenePresented { .. },
                    ..
                })
        );
        if presentation && event.revision < current.revision {
            // Do not fall back to the protocol's unchanged-scene observation exception after
            // history eviction or retirement: that would revive a pruned source/modal ABA proof.
            return self
                .scene_frames
                .accepted_document(panel, event, current)
                .ok_or_else(|| {
                    Failure::new(
                        ErrorCode::StaleRevision,
                        "Native Canvas presentation has no retained accepted snapshot",
                    )
                });
        }
        Ok(current.clone())
    }
    /// Gate shown-frame input before any guest call, retaining a narrow cleanup path after retirement.
    pub(crate) fn accept_scene_event(
        &mut self,
        panel: &str,
        event: &ui::UiEvent,
    ) -> Result<bool, Failure> {
        let action = match &event.action {
            ui::Action::Canvas(input)
            | ui::Action::ViewportInput(ui::ViewportInput { event: input, .. }) => input,
            _ => return Ok(true),
        };
        if let ui::CanvasEvent::Input { input } = action {
            self.scene_frames.validate(panel, event, input)?;
            if input.cleanup_kind().is_some()
                && !self.scene_frames.same_ancestry(
                    panel,
                    event,
                    input,
                    self.views.get(panel).map(std::sync::Arc::as_ref),
                )
            {
                self.scene_frames.commit(panel, event, input);
                return Ok(false);
            }
            if input.cleanup_kind().is_some() {
                self.scene_inputs.cancel(
                    panel,
                    &event.node,
                    input,
                    self.views.get(panel).map(std::sync::Arc::as_ref),
                );
            } else {
                let node = self
                    .views
                    .get(panel)
                    .and_then(|document| document.active_node(&event.node))
                    .ok_or_else(|| {
                        Failure::new(
                            ErrorCode::StaleRevision,
                            "Native Canvas target was replaced",
                        )
                    })?;
                self.scene_inputs.accept(panel, node, input)?;
            }
            self.scene_frames.commit(panel, event, input);
            return Ok(true);
        }
        let document = self.ui_event_document(panel, event)?;
        let Some(node) = document.active_node(&event.node) else {
            return Ok(true);
        };
        let ui::Kind::Canvas(canvas) = &node.kind else {
            return Ok(true);
        };
        let Some(scene) = &canvas.scene else {
            return Ok(true);
        };
        if matches!(action, ui::CanvasEvent::ScenePresented { .. }) {
            self.scene_frames
                .present(panel, event, action, scene, &document)?;
        } else if matches!(
            action,
            ui::CanvasEvent::Pointer { .. } | ui::CanvasEvent::Wheel { .. }
        ) && !self.scene_frames.current(panel, node, &document)
        {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Canvas coordinates have no current native presentation",
            ));
        }
        Ok(true)
    }
    /// Publication changes current drawing authority, independently of native frame/cleanup lifetime.
    pub(super) fn reconcile_scene_presentations(&mut self) {
        self.scene_frames.reconcile(&self.views);
        self.reconcile_transfer_offers();
    }
    /// Keep the accepted views and resources after a rejected live scene, while exposing diagnostics.
    /// Candidate activation and genuine WASM/native failures still follow ordinary retirement.
    pub(crate) fn preserve_rejected_scene(&mut self, error: &anyhow::Error) -> bool {
        if self.store.data().activation_committed && error.downcast_ref::<RejectedScene>().is_some()
        {
            self.error = Some(format!("{error:#}"));
            true
        } else {
            false
        }
    }
    /// Check the complete reply without mutating tombstones or any currently published document.
    pub(super) fn validate_scenes(&self, views: &[api::View]) -> Result<(), Failure> {
        self.scene_inputs
            .validate_publications(views, &self.views)?;
        let mut new = BTreeSet::new();
        let mut result = Ok(());
        for view in views {
            if let Some(previous) = self.views.get(&view.panel) {
                if (has_scene(previous) || has_scene(&view.document))
                    && view.document.revision <= previous.revision
                    && view.document != **previous
                {
                    return Err(Failure::new(
                        ErrorCode::StaleRevision,
                        "Full scene publications must advance their UI document revision",
                    ));
                }
            }
            visit(&view.document, |node| {
                if result.is_err() {
                    return;
                }
                let ui::Kind::Canvas(canvas) = &node.kind else {
                    return;
                };
                let Some(scene) = &canvas.scene else {
                    return;
                };
                let key = (view.panel.clone(), node.id.clone(), scene.identity);
                let mut current = false;
                if let Some(previous) = self.views.get(&view.panel) {
                    visit(previous, |old| {
                        if old.id != node.id {
                            return;
                        }
                        if let ui::Kind::Canvas(canvas) = &old.kind {
                            if let Some(before) = &canvas.scene {
                                current = before.identity == scene.identity;
                                if current
                                    && (scene.revision < before.revision
                                        || (scene.revision == before.revision && scene != before))
                                {
                                    result = Err(Failure::new(
                                        ErrorCode::StaleRevision,
                                        "Canvas scene revision did not advance with its contents",
                                    ));
                                }
                            }
                        }
                    });
                }
                if !current && self.scene_identities.contains(&key) {
                    result = Err(Failure::new(
                        ErrorCode::StaleRevision,
                        "Canvas scene identity was retired; publish a fresh identity",
                    ));
                }
                if !self.scene_identities.contains(&key) {
                    new.insert(key);
                }
            });
        }
        result?;
        if self.scene_identities.len() + new.len() > 4096 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Canvas scene identity budget exhausted; retain identities across drawing updates",
            ));
        }
        Ok(())
    }

    /// Commit tombstones only after every peer document and declared panel has passed validation.
    pub(super) fn remember_scenes(&mut self, views: &[api::View]) {
        self.scene_frames.remember(views, &self.views);
        self.scene_inputs.remember(views, &self.views);
        for view in views {
            visit(&view.document, |node| {
                if let ui::Kind::Canvas(canvas) = &node.kind {
                    if let Some(scene) = &canvas.scene {
                        self.scene_identities.insert((
                            view.panel.clone(),
                            node.id.clone(),
                            scene.identity,
                        ));
                    }
                }
            });
        }
    }
}
