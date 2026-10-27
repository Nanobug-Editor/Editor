//! Admission of native text chains and target generations is atomic with ordinary scene publication.
use super::*;
use plugin_protocol::ui::{
    CanvasCompositionPhase as Phase, CanvasInputKind as Kind, CanvasTextRange, TextInputSession,
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};

/// Bounded incarnation history prevents ABA and verifies drafts without claiming guest undo ownership.
#[derive(Default)]
pub(super) struct InputHistory {
    /// Retired IDs keep only their maximum generation, not drawing data or domain text.
    epochs: BTreeMap<(String, String, u64, String), u64>,
    /// Only currently declared native text targets retain pending drafts, at most 256/64 MiB.
    text: BTreeMap<(String, String, u64, String, u64), TextChain>,
    /// Native keyboard/IME authority follows explicit focus observations on the same shown target.
    focused: BTreeSet<(String, String, u64, String, u64)>,
}
/// Native commits are validated against their exact predecessor, independently of a later drawing ack.
#[derive(Clone)]
struct TextChain {
    binding: TextInputSession,
    source: Arc<str>,
    draft: Arc<str>,
    last: u64,
    versions: VecDeque<u64>,
    pending: VecDeque<(u64, Arc<str>)>,
    composition: Option<(CanvasTextRange, Arc<str>)>,
    /// A Text commit ends marked input before the separate End observation arrives.
    ended: bool,
}
impl InputHistory {
    /// Validate complete peer views against retained epochs and native acknowledgements without mutation.
    pub fn validate_publications(
        &self,
        views: &[api::View],
        previous: &BTreeMap<String, Arc<ui::Document>>,
    ) -> Result<(), api::Failure> {
        let mut result = Ok(());
        let mut new = BTreeSet::new();
        let mut sources = BTreeMap::new();
        for view in views {
            super::scene_history::visit(&view.document, |node| {
                if result.is_err() {
                    return;
                }
                let ui::Kind::Canvas(canvas) = &node.kind else {
                    return;
                };
                let Some(scene) = &canvas.scene else {
                    return;
                };
                for target in &scene.targets {
                    let epoch_key = (
                        view.panel.clone(),
                        node.id.clone(),
                        scene.identity,
                        target.id.clone(),
                    );
                    let current = previous
                        .get(&view.panel)
                        .and_then(|document| declared_node(document, &node.id))
                        .and_then(|node| {
                            if let ui::Kind::Canvas(canvas) = &node.kind {
                                canvas.scene.as_ref()
                            } else {
                                None
                            }
                        })
                        .filter(|before| before.identity == scene.identity)
                        .and_then(|before| before.targets.iter().find(|old| old.id == target.id));
                    if self.epochs.get(&epoch_key).is_some_and(|old| {
                        target.epoch < *old
                            || (target.epoch == *old
                                && current.is_none_or(|current| current.epoch != target.epoch))
                    }) {
                        result = Err(stale("Retired Canvas target requires a higher epoch"));
                        return;
                    }
                    if !self.epochs.contains_key(&epoch_key) {
                        new.insert(epoch_key);
                    }
                    if let Some(binding) = &target.text_input {
                        let text = scene
                            .paragraph_text(&binding.paragraph, &binding.layout_key)
                            .expect("validated text source");
                        let key = (
                            view.panel.clone(),
                            node.id.clone(),
                            scene.identity,
                            target.id.clone(),
                            target.epoch,
                        );
                        if let Some(chain) = self.text.get(&key).filter(|_| {
                            same_owner(
                                previous.get(&view.panel).map(Arc::as_ref),
                                &view.document,
                                &node.id,
                            )
                        }) {
                            if let Err(error) = chain.reconciled(binding, text) {
                                result = Err(error);
                                return;
                            }
                        } else if binding.acknowledged_sequence != 0 {
                            result = Err(conflict(
                                "New Canvas target cannot acknowledge another session",
                            ));
                            return;
                        }
                        sources.insert(key, text.len());
                    }
                }
            });
        }
        result?;
        // Include unchanged panels in the instance-wide budget instead of checking each reply alone.
        for (key, chain) in &self.text {
            if !views.iter().any(|view| view.panel == key.0) {
                sources.insert(key.clone(), chain.source.len());
            }
        }
        if self.epochs.len() + new.len() > 131072
            || sources.len() > 256
            || sources.values().sum::<usize>() > 16 * 1024 * 1024
        {
            return Err(limit(
                "Canvas target history or native text source budget exhausted",
            ));
        }
        Ok(())
    }
    /// Commit only previously validated declarations, retaining independent native pending text.
    pub fn remember(
        &mut self,
        views: &[api::View],
        previous: &BTreeMap<String, Arc<ui::Document>>,
    ) {
        let mut live = BTreeSet::new();
        for (panel, document) in previous
            .iter()
            .filter(|(panel, _)| !views.iter().any(|view| view.panel == **panel))
        {
            super::scene_history::visit(document, |node| collect_text_keys(panel, node, &mut live));
        }
        for view in views {
            super::scene_history::visit(&view.document, |node| {
                let ui::Kind::Canvas(canvas) = &node.kind else {
                    return;
                };
                let Some(scene) = &canvas.scene else {
                    return;
                };
                for target in &scene.targets {
                    self.epochs.insert(
                        (
                            view.panel.clone(),
                            node.id.clone(),
                            scene.identity,
                            target.id.clone(),
                        ),
                        target.epoch,
                    );
                    if let Some(binding) = &target.text_input {
                        let key = (
                            view.panel.clone(),
                            node.id.clone(),
                            scene.identity,
                            target.id.clone(),
                            target.epoch,
                        );
                        let source = scene
                            .paragraph_text(&binding.paragraph, &binding.layout_key)
                            .expect("validated source");
                        let chain = self
                            .text
                            .get(&key)
                            .filter(|_| {
                                same_owner(
                                    previous.get(&view.panel).map(Arc::as_ref),
                                    &view.document,
                                    &node.id,
                                )
                            })
                            .map_or_else(
                                || TextChain::new(binding, source),
                                |chain| {
                                    chain
                                        .reconciled(binding, source)
                                        .expect("publication was validated")
                                },
                            );
                        self.text.insert(key.clone(), chain);
                        live.insert(key);
                    }
                }
            });
        }
        self.text.retain(|key, _| live.contains(key));
        self.focused.retain(|key| {
            let document=views.iter().find(|view|view.panel==key.0).map(|view|&view.document)
                .or_else(||previous.get(&key.0).map(Arc::as_ref));
            document.filter(|document|same_owner(previous.get(&key.0).map(Arc::as_ref),document,&key.1)).and_then(|document|document.active_node(&key.1)).is_some_and(|node|
                matches!(&node.kind,ui::Kind::Canvas(canvas) if canvas.scene.as_ref().is_some_and(|scene|
                    scene.identity==key.2&&scene.targets.iter().any(|target|target.id==key.3&&target.epoch==key.4))))
        });
    }
    /// Cleanup consumes only the original source's transient state; it cannot clear a new source's focus.
    pub fn cancel(
        &mut self,
        panel: &str,
        node: &str,
        input: &ui::CanvasInput,
        current: Option<&ui::Document>,
    ) {
        if current
            .is_none_or(|document| input.source != document.source || input.file != document.file)
        {
            return;
        }
        let key = (
            panel.to_owned(),
            node.to_owned(),
            input.identity,
            input.target.clone(),
            input.epoch,
        );
        if matches!(input.kind, Kind::Focus { focused: false }) {
            self.focused.remove(&key);
        }
        if matches!(
            input.kind,
            Kind::Focus { focused: false } | Kind::Composition(_)
        ) {
            if let Some(chain) = self.text.get_mut(&key) {
                chain.composition = None;
                chain.ended = false;
            }
        }
    }
    /// Validate native text/selection/preedit before WASM; rejected input leaves every draft unchanged.
    pub fn accept(
        &mut self,
        panel: &str,
        node: &ui::Node,
        input: &ui::CanvasInput,
    ) -> Result<(), api::Failure> {
        let key = (
            panel.to_owned(),
            node.id.clone(),
            input.identity,
            input.target.clone(),
            input.epoch,
        );
        if matches!(
            input.kind,
            Kind::Text(_) | Kind::Composition(_) | Kind::Selection { .. } | Kind::Key { .. }
        ) && !self.focused.contains(&key)
        {
            return Err(stale("Native Canvas text/key target has no current focus"));
        }
        if let Kind::Focus { focused } = input.kind {
            if focused {
                self.focused.retain(|old| old.0 != key.0 || old.1 != key.1);
                self.focused.insert(key.clone());
            } else {
                self.focused.remove(&key);
            }
        }
        let Some(chain) = self.text.get(&key) else {
            return if matches!(
                input.kind,
                Kind::Text(_) | Kind::Composition(_) | Kind::Selection { .. }
            ) {
                Err(stale("Canvas text session was replaced"))
            } else {
                Ok(())
            };
        };
        let mut next = chain.clone();
        next.accept(input)?;
        let resident =
            self.text.values().map(TextChain::bytes).sum::<usize>() - chain.bytes() + next.bytes();
        if resident > 64 * 1024 * 1024 {
            return Err(limit("Canvas native draft budget exhausted"));
        }
        self.text.insert(key, next);
        Ok(())
    }
}
/// Pending drafts cannot cross even an identical target ID/epoch when source authority changes.
fn same_source(previous: Option<&ui::Document>, next: &ui::Document) -> bool {
    previous.is_none_or(|previous| previous.source == next.source && previous.file == next.file)
}
/// An unchanged inner scene cannot inherit native drafts/focus after an enclosing overlay is replaced.
fn same_owner(previous: Option<&ui::Document>, next: &ui::Document, node: &str) -> bool {
    same_source(previous, next)
        && previous.is_none_or(|before| {
            crate::ui_owners::lineage(before, node) == crate::ui_owners::lineage(next, node)
        })
}
/// Publications retain identities under temporary modal ownership; this lookup grants no input.
fn declared_node<'a>(document: &'a ui::Document, id: &str) -> Option<&'a ui::Node> {
    crate::ui_owners::find(document, id)
}
/// Preserve unchanged panels' current text identities without retaining a second document tree.
fn collect_text_keys(
    panel: &str,
    node: &ui::Node,
    keys: &mut BTreeSet<(String, String, u64, String, u64)>,
) {
    if let ui::Kind::Canvas(canvas) = &node.kind {
        if let Some(scene) = &canvas.scene {
            for target in &scene.targets {
                if target.text_input.is_some() {
                    keys.insert((
                        panel.into(),
                        node.id.clone(),
                        scene.identity,
                        target.id.clone(),
                        target.epoch,
                    ));
                }
            }
        }
    }
}
impl TextChain {
    /// Begin from a validated immutable guest paragraph; no native text has been committed yet.
    fn new(binding: &TextInputSession, text: &str) -> Self {
        let source: Arc<str> = text.into();
        Self {
            binding: binding.clone(),
            draft: source.clone(),
            source,
            last: 0,
            versions: VecDeque::from([binding.text_revision]),
            pending: VecDeque::new(),
            composition: None,
            ended: false,
        }
    }
    /// Bound retained snapshots conservatively even where Arc shares their actual allocation.
    fn bytes(&self) -> usize {
        self.source.len()
            + self.draft.len()
            + self
                .pending
                .iter()
                .map(|(_, text)| text.len())
                .sum::<usize>()
            + self.composition.as_ref().map_or(0, |(_, text)| text.len())
    }
    /// Validate cumulative acknowledgements as whole native snapshots, not range guesses on new source.
    fn reconciled(&self, binding: &TextInputSession, text: &str) -> Result<Self, api::Failure> {
        if binding.text_revision < self.binding.text_revision
            || binding.acknowledged_sequence < self.binding.acknowledged_sequence
        {
            return Err(stale("Canvas text acknowledgement moved backwards"));
        }
        let changed = text != self.source.as_ref();
        let advanced = binding.acknowledged_sequence > self.binding.acknowledged_sequence;
        if (changed || advanced) && binding.text_revision <= self.binding.text_revision {
            return Err(conflict(
                "Canvas text change requires a higher text revision",
            ));
        }
        if advanced {
            if !self.pending.iter().any(|(sequence, value)| {
                *sequence == binding.acknowledged_sequence && value.as_ref() == text
            }) {
                return Err(conflict(
                    "Canvas acknowledgement does not match a known native commit",
                ));
            }
        } else if changed && (!self.pending.is_empty() || self.composition.is_some()) {
            return Err(conflict(
                "Replace target epoch before changing text with pending native input",
            ));
        }
        let mut next = self.clone();
        if advanced {
            next.pending
                .retain(|(sequence, _)| *sequence > binding.acknowledged_sequence);
        }
        if changed {
            next.source = text.into();
            if !advanced {
                next.draft = next.source.clone();
            }
        }
        if binding.text_revision > self.binding.text_revision {
            next.versions.push_back(binding.text_revision);
            while next.versions.len() > 32 {
                next.versions.pop_front();
            }
        }
        next.binding = binding.clone();
        Ok(next)
    }
    /// Exact predecessor evidence tolerates queued native input created before a newer ack was displayed.
    fn predecessor(&self, revision: u64, sequence: u64) -> Result<(), api::Failure> {
        if !self.versions.contains(&revision)
            || if sequence == 0 {
                revision != self.binding.text_revision
                    || !self.pending.is_empty()
                    || self.draft != self.source
            } else {
                sequence != self.last
            }
        {
            return Err(stale("Canvas native text predecessor changed"));
        }
        Ok(())
    }
    /// Apply only temporary native state; guest committed source and domain undo remain guest-owned.
    fn accept(&mut self, input: &ui::CanvasInput) -> Result<(), api::Failure> {
        match &input.kind {
            Kind::Text(edit) => {
                self.predecessor(edit.text_revision, edit.base_sequence)?;
                if self.binding.read_only {
                    return Err(conflict("Canvas text is readonly"));
                }
                if let Some((range, _)) = &self.composition {
                    if edit.range != *range {
                        return Err(conflict("Native text did not replace its marked source"));
                    }
                }
                let next = replaced(&self.draft, &edit.range, &edit.text)?;
                edit.selection.to_utf16(&next).map_err(invalid)?;
                if self.pending.len() >= 32
                    || self
                        .pending
                        .iter()
                        .map(|(_, text)| text.len())
                        .sum::<usize>()
                        + next.len()
                        > 1024 * 1024
                {
                    return Err(limit("Canvas native acknowledgement queue exhausted"));
                }
                self.ended = self.composition.take().is_some();
                self.draft = next.into();
                self.last = input.sequence;
                self.pending.push_back((input.sequence, self.draft.clone()));
            }
            Kind::Composition(value) => {
                self.predecessor(value.text_revision, value.base_sequence)?;
                match value.phase {
                    Phase::Start | Phase::Update => {
                        if self.binding.read_only {
                            return Err(conflict("Canvas text is readonly"));
                        }
                        if (value.phase == Phase::Start && self.composition.is_some())
                            || (value.phase == Phase::Update
                                && self
                                    .composition
                                    .as_ref()
                                    .is_none_or(|(range, _)| *range != value.range))
                        {
                            return Err(conflict("Canvas native composition lifecycle mismatch"));
                        }
                        let visual = replaced(&self.draft, &value.range, &value.text)?;
                        CanvasTextRange {
                            start: value.range.start,
                            end: value.range.start + value.text.len(),
                        }
                        .to_utf16(&visual)
                        .map_err(invalid)?;
                        self.composition = Some((value.range.clone(), value.text.as_str().into()));
                        self.ended = false;
                    }
                    Phase::Cancel => {
                        if self.composition.take().is_none() {
                            return Err(stale("Canvas native preedit already ended"));
                        }
                        self.ended = false;
                    }
                    Phase::End => {
                        if !self.ended {
                            return Err(conflict(
                                "Canvas composition End requires its independent Text commit",
                            ));
                        }
                        self.ended = false;
                    }
                }
            }
            Kind::Selection {
                text_revision,
                base_sequence,
                range,
            } => {
                self.predecessor(*text_revision, *base_sequence)?;
                let visual = self
                    .composition
                    .as_ref()
                    .map(|(range, text)| replaced(&self.draft, range, text))
                    .transpose()?;
                range
                    .to_utf16(visual.as_deref().unwrap_or(&self.draft))
                    .map_err(invalid)?;
            }
            Kind::Focus { focused: false } => {
                self.composition = None;
                self.ended = false;
            }
            _ => {}
        }
        Ok(())
    }
}
/// Reserve the exact bounded result only after checking original whole-grapheme coordinates.
fn replaced(source: &str, range: &CanvasTextRange, text: &str) -> Result<String, api::Failure> {
    range.to_utf16(source).map_err(invalid)?;
    let length = source.len() - (range.end - range.start) + text.len();
    if length > 65536 {
        return Err(limit("Canvas native draft exceeds 64 KiB"));
    }
    let mut result = String::with_capacity(length);
    result.push_str(&source[..range.start]);
    result.push_str(text);
    result.push_str(&source[range.end..]);
    Ok(result)
}
/// Input coordinate errors are recoverable and do not retire the guest.
fn invalid(message: impl Into<String>) -> api::Failure {
    api::Failure::new(api::ErrorCode::InvalidRequest, message)
}
/// Old frame/session evidence cannot advance current native state.
fn stale(message: impl Into<String>) -> api::Failure {
    api::Failure::new(api::ErrorCode::StaleRevision, message)
}
/// Contradictory acknowledgements require explicit guest reconciliation.
fn conflict(message: impl Into<String>) -> api::Failure {
    api::Failure::new(api::ErrorCode::Conflict, message)
}
/// Queue/draft/history growth has an explicit producer-visible bound.
fn limit(message: impl Into<String>) -> api::Failure {
    api::Failure::new(api::ErrorCode::LimitExceeded, message)
}
