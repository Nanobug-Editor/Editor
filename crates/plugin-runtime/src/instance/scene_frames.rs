//! Bounded native frame proofs separate accepted artwork from displayed input and revocable cleanup.
use super::*;
use plugin_protocol::ui::{CanvasCleanupKind as Cleanup, CanvasInputKind as Kind};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// One incarnation's displayed frames and monotonic input order, never source document authority.
#[derive(Default)]
pub(super) struct Frames {
    shown: BTreeMap<(String, String), VecDeque<Frame>>,
    sequence: BTreeMap<(String, String), u64>,
}
/// Original source, projection and target membership cannot be reinterpreted by later publications.
struct Frame {
    /// A replacement enclosing overlay cannot inherit this child Canvas's original native proof.
    lineage: crate::ui_owners::Lineage,
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
    fn same_owner(&self, receipt: &ui::CanvasEvent) -> bool {
        matches!((&self.receipt,receipt),
            (ui::CanvasEvent::ScenePresented{identity:a,source:sa,file:fa,..},ui::CanvasEvent::ScenePresented{identity:b,source:sb,file:fb,..})
            if a==b&&sa==sb&&fa==fb&&!self.retired)
    }
}
impl Frames {
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
            .filter(|frame| frame.same_owner(receipt) && frame.lineage == lineage)
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
        let lineage = frames
            .iter()
            .find(|frame| frame.matches(event, input))
            .map(|frame| frame.lineage.clone());
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
            if lineage.as_ref() != Some(&frame.lineage) {
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
        let current =
            document.and_then(|document| crate::ui_owners::lineage(document, &event.node));
        self.shown
            .get(&(panel.into(), event.node.clone()))
            .is_some_and(|frames| {
                frames.iter().any(|frame| {
                    frame.matches(event, input) && current.as_ref() == Some(&frame.lineage)
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
            !frame.retired&&matches!(&frame.receipt,ui::CanvasEvent::ScenePresented{identity,scene_revision,source,file,projection,error:None}
                if *identity==scene.identity&&*scene_revision==scene.revision&&*source==document.source&&*file==document.file&&*projection==node.viewport))
    }
    /// Ordinary drawing updates retain proofs; source/modal/scene retirement retains only active cleanup.
    pub fn reconcile(&mut self, views: &BTreeMap<String, std::sync::Arc<ui::Document>>) {
        self.shown.retain(|(panel,id),frames| {
            let document=views.get(panel);
            for frame in frames.iter_mut() {
                let live=document.and_then(|document|document.active_node(id).map(|node|(document,node)))
                    .is_some_and(|(document,node)|!node.disabled&&crate::ui_owners::lineage(document,id).as_ref()==Some(&frame.lineage)&&matches!((&frame.receipt,&node.kind),
                        (ui::CanvasEvent::ScenePresented{identity,source,file,error:None,..},ui::Kind::Canvas(canvas))
                        if *source==document.source&&*file==document.file&&canvas.scene.as_ref().is_some_and(|scene|scene.identity==*identity)));
                if !live {retire(frame);}
            }
            frames.retain(|frame|!frame.retired||!frame.cleanup.is_empty());!frames.is_empty()
        });
    }
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
