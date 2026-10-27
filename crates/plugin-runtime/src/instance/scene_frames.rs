//! Bounded native frame proofs separate accepted artwork from displayed input and revocable cleanup.
use super::*;
use plugin_protocol::ui::{CanvasCleanupKind as Cleanup, CanvasInputKind as Kind};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
#[cfg(test)]
mod lifetimes_tests;

/// One incarnation's displayed frames and monotonic input order, never source document authority.
#[derive(Default)]
pub(super) struct Frames {
    shown: BTreeMap<(String, String), VecDeque<Frame>>,
    sequence: BTreeMap<(String, String), u64>,
    /// Four quota-validated immutable documents across the instance, never four per Canvas/panel.
    /// Arc retention avoids copying or re-encoding complete artwork on every small scene patch.
    accepted: VecDeque<Accepted>,
}
/// A historical document can lose individual Canvas lifetimes permanently while siblings remain live.
struct Accepted {
    panel: String,
    document: std::sync::Arc<ui::Document>,
    canvases: BTreeSet<String>,
}
/// Original source, projection and target membership cannot be reinterpreted by later publications.
struct Frame {
    /// A replacement enclosing overlay cannot inherit this child Canvas's original native proof.
    lineage: crate::ui_owners::Lineage,
    /// Editor rails bind literal and document actions to one exact outer UI revision.
    action_revision: Option<u64>,
    receipt: ui::CanvasEvent,
    revision: u64,
    targets: BTreeSet<(String, u64)>,
    /// Focus=1, preedit=2, five pressed pointer buttons=bits 2..6; only observed ownership can cancel.
    cleanup: BTreeMap<(String, u64), u8>,
    /// Retirement never reverses, including source A→B→A without another native presentation.
    retired: bool,
}
impl Frame {
    /// Match every original native addressing field, not merely an accepted scene revision.
    fn matches(&self, event: &ui::UiEvent, input: &ui::CanvasInput) -> bool {
        self.revision == event.revision
            && self.targets.contains(&(input.target.clone(), input.epoch))
            && matches!(&self.receipt,ui::CanvasEvent::ScenePresented{identity,scene_revision,source,file,projection,error:None}
                if *identity==input.identity&&*scene_revision==input.presented_revision&&*source==input.source&&*file==input.file&&*projection==input.projection)
    }
    /// New paint may carry active target ownership only within the same source/scene lifetime.
    fn same_owner(&self, receipt: &ui::CanvasEvent, action_revision: Option<u64>) -> bool {
        matches!((&self.receipt,receipt),
            (ui::CanvasEvent::ScenePresented{identity:a,source:sa,file:fa,..},ui::CanvasEvent::ScenePresented{identity:b,source:sb,file:fb,..})
            if a==b&&sa==sb&&fa==fb&&!self.retired&&self.action_revision==action_revision)
    }
}
impl Frames {
    /// Preserve the immediately preceding accepted document before atomic publication replaces it.
    /// The existing 32 MiB reply quota bounds every retained document; at most four are kept total.
    pub(super) fn remember(
        &mut self,
        views: &[api::View],
        current: &BTreeMap<String, std::sync::Arc<ui::Document>>,
    ) {
        for view in views {
            let Some(document) = current
                .get(&view.panel)
                .filter(|old| old.revision != view.document.revision)
            else {
                continue;
            };
            let mut canvases = BTreeSet::new();
            super::scene_history::visit(document, |node| {
                // Ordinary UI nodes need no Canvas history. Avoid rescanning their whole
                // document for lineage on every publication of a large native list/form.
                if matches!(&node.kind, ui::Kind::Canvas(canvas) if canvas.scene.is_some())
                    && same_lifetime(document, document, &node.id)
                {
                    canvases.insert(node.id.clone());
                }
            });
            if !canvases.is_empty() {
                self.accepted.push_back(Accepted {
                    panel: view.panel.clone(),
                    document: document.clone(),
                    canvases,
                });
                while self.accepted.len() > 4 {
                    self.accepted.pop_front();
                }
            }
        }
    }
    /// Only successful truthful receipts may consult history. Old failures must not retire a newer
    /// successful proof; all input, legacy coordinates and failed receipts retain their current gate.
    pub(super) fn accepted_document(
        &self,
        panel: &str,
        event: &ui::UiEvent,
        current: &ui::Document,
    ) -> Option<std::sync::Arc<ui::Document>> {
        if event.revision >= current.revision
            || !matches!(
                &event.action,
                ui::Action::Canvas(ui::CanvasEvent::ScenePresented { error: None, .. })
                    | ui::Action::ViewportInput(ui::ViewportInput {
                        event: ui::CanvasEvent::ScenePresented { error: None, .. },
                        ..
                    })
            )
        {
            return None;
        }
        self.accepted
            .iter()
            .rev()
            .find(|accepted| {
                accepted.panel == panel
                    && accepted.document.revision == event.revision
                    && accepted.canvases.contains(&event.node)
                    && same_lifetime(&accepted.document, current, &event.node)
            })
            .map(|accepted| accepted.document.clone())
    }
    /// Native drag uses the latest actually shown target membership, independently of accepted artwork.
    pub(super) fn transfer_current(
        &self,
        panel: &str,
        address: &plugin_protocol::transfer::Address,
    ) -> bool {
        let Some(anchor) = &address.canvas else {
            return false;
        };
        self.shown.get(&(panel.into(),address.node.clone())).and_then(|frames|frames.back()).is_some_and(|frame|
            !frame.retired&&frame.revision==address.ui_revision&&frame.targets.contains(&(anchor.target.id.clone(),anchor.target.epoch))
                &&matches!(&frame.receipt,ui::CanvasEvent::ScenePresented{identity,scene_revision,source,file,error:None,..}
                    if *identity==anchor.identity&&*scene_revision==anchor.scene_revision&&*source==address.source&&*file==address.file))
    }
    /// Remember a successful native frame with a four-frame queue; failures revoke input first.
    pub fn present(
        &mut self,
        panel: &str,
        event: &ui::UiEvent,
        receipt: &ui::CanvasEvent,
        scene: &ui::CanvasScene,
        document: &ui::Document,
    ) -> Result<(), api::Failure> {
        let key = (panel.to_owned(), event.node.clone());
        let lineage = crate::ui_owners::lineage(document, &event.node).unwrap_or_default();
        let action_revision = document
            .is_editor_action(&event.node)
            .then_some(document.revision);
        let ui::CanvasEvent::ScenePresented { error, .. } = receipt else {
            return Ok(());
        };
        if error.is_some() {
            if let Some(frames) = self.shown.get_mut(&key) {
                for frame in frames {
                    retire(frame);
                }
            }
            return Ok(());
        }
        if !self.shown.contains_key(&key) && self.shown.len() >= 256 {
            return Err(api::Failure::new(
                api::ErrorCode::LimitExceeded,
                "Native Canvas frame budget exhausted",
            ));
        }
        if !self.sequence.contains_key(&key) && self.sequence.len() >= 4096 {
            return Err(api::Failure::new(
                api::ErrorCode::LimitExceeded,
                "Native Canvas sequence lifetime budget exhausted",
            ));
        }
        self.sequence.entry(key.clone()).or_default();
        let frames = self.shown.entry(key).or_default();
        if frames.back().is_some_and(|frame| {
            frame.receipt == *receipt && frame.revision == event.revision && !frame.retired
        }) {
            return Ok(());
        }
        let targets: BTreeSet<_> = scene
            .targets
            .iter()
            .map(|target| (target.id.clone(), target.epoch))
            .collect();
        let cleanup = frames
            .back()
            .filter(|frame| frame.same_owner(receipt, action_revision) && frame.lineage == lineage)
            .map(|frame| {
                frame
                    .cleanup
                    .iter()
                    .filter(|(key, _)| targets.contains(*key))
                    .map(|(key, bits)| (key.clone(), *bits))
                    .collect()
            })
            .unwrap_or_default();
        frames.push_back(Frame {
            lineage,
            action_revision,
            receipt: receipt.clone(),
            revision: event.revision,
            targets,
            cleanup,
            retired: false,
        });
        while frames.len() > 4 {
            frames.pop_front();
        }
        Ok(())
    }
    /// Check frame membership/order without mutation so a malformed native text edit stays atomic.
    pub fn validate(
        &self,
        panel: &str,
        event: &ui::UiEvent,
        input: &ui::CanvasInput,
    ) -> Result<(), api::Failure> {
        let key = (panel.to_owned(), event.node.clone());
        let cleanup = input.cleanup_kind();
        let proven = self.shown.get(&key).is_some_and(|frames| {
            frames.iter().any(|frame| {
                frame.matches(event, input)
                    && cleanup.map_or(!frame.retired, |kind| {
                        frame
                            .cleanup
                            .get(&(input.target.clone(), input.epoch))
                            .is_some_and(|bits| *bits & bit(kind) != 0)
                    })
            })
        });
        if !proven || input.sequence <= self.sequence.get(&key).copied().unwrap_or(0) {
            return Err(api::Failure::new(
                api::ErrorCode::StaleRevision,
                "Native target has no matching displayed frame, active cleanup lease or input sequence",
            ));
        }
        Ok(())
    }
    /// Advance only after text admission succeeds; terminal observations consume all matching old leases.
    pub fn commit(&mut self, panel: &str, event: &ui::UiEvent, input: &ui::CanvasInput) {
        self.sequence
            .insert((panel.to_owned(), event.node.clone()), input.sequence);
        let Some(frames) = self.shown.get_mut(&(panel.to_owned(), event.node.clone())) else {
            return;
        };
        let owner = frames
            .iter()
            .find(|frame| frame.matches(event, input))
            .map(|frame| (frame.lineage.clone(), frame.action_revision));
        let (add, remove) = match &input.kind {
            Kind::Focus { focused: true } => (1, 0),
            Kind::Focus { focused: false } => (0, 1),
            Kind::Composition(value) => match value.phase {
                ui::CanvasCompositionPhase::Start => (2, 0),
                ui::CanvasCompositionPhase::End | ui::CanvasCompositionPhase::Cancel => (0, 2),
                _ => (0, 0),
            },
            Kind::Pointer(value) => match value.phase {
                ui::CanvasPointerPhase::Down => {
                    (value.button.map_or(0, |button| 1 << (button + 2)), 0)
                }
                ui::CanvasPointerPhase::Up => {
                    (0, value.button.map_or(0, |button| 1 << (button + 2)))
                }
                ui::CanvasPointerPhase::Cancel => (0, bit(Cleanup::Pointer)),
                _ => (0, 0),
            },
            _ => (0, 0),
        };
        for frame in frames {
            if owner.as_ref().is_none_or(|(lineage, revision)| {
                *lineage != frame.lineage || *revision != frame.action_revision
            }) {
                continue;
            }
            let same = matches!(&frame.receipt,ui::CanvasEvent::ScenePresented{identity,source,file,..}
                if *identity==input.identity&&*source==input.source&&*file==input.file);
            if !same || !frame.targets.contains(&(input.target.clone(), input.epoch)) {
                continue;
            }
            if add & 1 != 0 {
                for bits in frame.cleanup.values_mut() {
                    *bits &= !1;
                }
            }
            let target = (input.target.clone(), input.epoch);
            let bits = frame.cleanup.entry(target.clone()).or_default();
            *bits = (*bits | add) & !remove;
            if *bits == 0 {
                frame.cleanup.remove(&target);
            }
        }
    }
    /// Retired native cleanup may seal its old lease, but cannot mutate a replacement child's focus/draft.
    pub fn same_ancestry(
        &self,
        panel: &str,
        event: &ui::UiEvent,
        input: &ui::CanvasInput,
        document: Option<&ui::Document>,
    ) -> bool {
        let current = document.and_then(|document| {
            crate::ui_owners::lineage(document, &event.node).map(|lineage| {
                (
                    lineage,
                    document
                        .is_editor_action(&event.node)
                        .then_some(document.revision),
                )
            })
        });
        self.shown
            .get(&(panel.into(), event.node.clone()))
            .is_some_and(|frames| {
                frames.iter().any(|frame| {
                    frame.matches(event, input)
                        && current.as_ref().is_some_and(|(lineage, revision)| {
                            *lineage == frame.lineage && *revision == frame.action_revision
                        })
                })
            })
    }
    /// Old legacy pointer coordinates still require the exact current scene and viewport.
    pub fn current(&self, panel: &str, node: &ui::Node, document: &ui::Document) -> bool {
        let ui::Kind::Canvas(canvas) = &node.kind else {
            return false;
        };
        let Some(scene) = &canvas.scene else {
            return false;
        };
        self.shown.get(&(panel.to_owned(),node.id.clone())).and_then(|frames|frames.back()).is_some_and(|frame|
            !frame.retired&&frame.action_revision==document.is_editor_action(&node.id).then_some(document.revision)&&matches!(&frame.receipt,ui::CanvasEvent::ScenePresented{identity,scene_revision,source,file,projection,error:None}
                if *identity==scene.identity&&*scene_revision==scene.revision&&*source==document.source&&*file==document.file&&*projection==node.viewport))
    }
    /// Ordinary drawing updates retain proofs; source/modal/scene retirement retains only active cleanup.
    pub fn reconcile(&mut self, views: &BTreeMap<String, std::sync::Arc<ui::Document>>) {
        // Prune on every accepted transition. Returning to an old source/modal/overlay identity
        // cannot reinsert an ID into this historical snapshot and revive its old authority.
        self.accepted.retain_mut(|accepted| {
            accepted.canvases.retain(|id| {
                views
                    .get(&accepted.panel)
                    .is_some_and(|current| same_lifetime(&accepted.document, current, id))
            });
            !accepted.canvases.is_empty()
        });
        self.shown.retain(|(panel,id),frames| {
            let document=views.get(panel);
            for frame in frames.iter_mut() {
                let live=document.and_then(|document|document.active_node(id).map(|node|(document,node)))
                    .is_some_and(|(document,node)|!node.disabled&&frame.action_revision==document.is_editor_action(id).then_some(document.revision)&&crate::ui_owners::lineage(document,id).as_ref()==Some(&frame.lineage)&&matches!((&frame.receipt,&node.kind),
                        (ui::CanvasEvent::ScenePresented{identity,source,file,error:None,..},ui::Kind::Canvas(canvas))
                        if *source==document.source&&*file==document.file&&canvas.scene.as_ref().is_some_and(|scene|scene.identity==*identity)));
                if !live {retire(frame);}
            }
            frames.retain(|frame|!frame.retired||!frame.cleanup.is_empty());!frames.is_empty()
        });
    }
}
/// Drawing revision/projection may advance, while source, foreground ownership and scene incarnation
/// must still be exactly the same. Historical document validation checks its original projection.
fn same_lifetime(before: &ui::Document, current: &ui::Document, id: &str) -> bool {
    if before.source != current.source
        || before.file != current.file
        || before.dialog.is_some()
        || before.menu.is_some()
        || current.dialog.is_some()
        || current.menu.is_some()
        || before.is_editor_action(id)
        || current.is_editor_action(id)
        || crate::ui_owners::lineage(before, id) != crate::ui_owners::lineage(current, id)
    {
        return false;
    }
    let (Some(old), Some(new)) = (before.active_node(id), current.active_node(id)) else {
        return false;
    };
    !old.disabled
        && !new.disabled
        && matches!((&old.kind,&new.kind),(ui::Kind::Canvas(a),ui::Kind::Canvas(b))
        if a.scene.as_ref().zip(b.scene.as_ref()).is_some_and(|(a,b)|a.identity==b.identity))
}
/// Retired proofs discard all unneeded geometry membership while keeping bounded cancellation evidence.
fn retire(frame: &mut Frame) {
    frame.retired = true;
    frame.targets.retain(|key| frame.cleanup.contains_key(key));
}
/// Explicit native ownership categories never depend on enum discriminants or transport layout.
fn bit(kind: Cleanup) -> u8 {
    match kind {
        Cleanup::Focus => 1,
        Cleanup::Composition => 2,
        Cleanup::Pointer => 0b0111_1100,
    }
}
