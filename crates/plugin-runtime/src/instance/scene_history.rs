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
fn visit(document: &ui::Document, mut visitor: impl FnMut(&ui::Node)) {
    document.root.visit(&mut visitor);
    if let Some(toolbar) = &document.editor_toolbar {
        toolbar.visit(&mut visitor);
    }
    if let Some(dialog) = &document.dialog {
        dialog.content.visit(&mut visitor);
    }
}

impl Instance {
    /// Accept observations separately from input: publishing alone never authorizes scene coordinates.
    pub(crate) fn accept_scene_event(
        &mut self,
        panel: &str,
        event: &ui::UiEvent,
    ) -> Result<(), Failure> {
        let input = match &event.action {
            ui::Action::Canvas(input)
            | ui::Action::ViewportInput(ui::ViewportInput { event: input, .. }) => input,
            _ => return Ok(()),
        };
        let Some(document) = self.views.get(panel) else {
            return Ok(());
        };
        let Some(node) = document.active_node(&event.node) else {
            return Ok(());
        };
        let ui::Kind::Canvas(canvas) = &node.kind else {
            return Ok(());
        };
        if canvas.scene.is_none() {
            return Ok(());
        }
        let key = (panel.to_owned(), node.id.clone());
        if let ui::CanvasEvent::ScenePresented { error, .. } = input {
            if error.is_none() {
                self.scene_presentations.insert(key, input.clone());
            } else {
                self.scene_presentations.remove(&key);
            }
        } else if matches!(
            input,
            ui::CanvasEvent::Pointer { .. } | ui::CanvasEvent::Wheel { .. }
        ) && !self
            .scene_presentations
            .get(&key)
            .is_some_and(|receipt| matches_presentation(receipt, document, node))
        {
            return Err(Failure::new(
                ErrorCode::StaleRevision,
                "Canvas coordinates have no current native presentation",
            ));
        }
        Ok(())
    }

    /// Remove stale/withdrawn observations after the complete reply has committed its documents.
    pub(super) fn reconcile_scene_presentations(&mut self) {
        self.scene_presentations.retain(|(panel, node), receipt| {
            self.views.get(panel).is_some_and(|document| {
                document
                    .active_node(node)
                    .is_some_and(|node| matches_presentation(receipt, document, node))
            })
        });
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

/// Presentation remains valid only for the same scene, source authority and viewport declaration.
fn matches_presentation(
    receipt: &ui::CanvasEvent,
    document: &ui::Document,
    node: &ui::Node,
) -> bool {
    let ui::Kind::Canvas(canvas) = &node.kind else {
        return false;
    };
    let Some(scene) = &canvas.scene else {
        return false;
    };
    matches!(receipt,ui::CanvasEvent::ScenePresented{identity,scene_revision,source,file,projection,error:None}
        if *identity==scene.identity&&*scene_revision==scene.revision&&*source==document.source&&*file==document.file&&*projection==node.viewport)
}
