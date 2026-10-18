//! Native form delivery keeps text drafts separate from scene-bound business activations.
use super::*;
use futures::channel::oneshot;
use gpui_kit::{EntityId, Task};

/// The actor acknowledges actual Manager admission, not whether the guest echoes a field value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FormReceipt {
    Delivered,
    Stale,
    Retired,
    Rejected,
}

/// An optional private host transport leaves ordinary standalone EventSink consumers unchanged.
pub(crate) type FormChangeSink = Rc<dyn Fn(UiEvent, &mut App) -> oneshot::Receiver<FormReceipt>>;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Binding {
    entity: EntityId,
    value_revision: u64,
}

struct Flight {
    sequence: u64,
    revision: u64,
    /// Removing/overriding a field cancels its waiter without retaining its native entity.
    task: Option<Task<()>>,
}

struct Draft {
    binding: Binding,
    value: String,
    sequence: u64,
    flight: Option<Flight>,
    /// A rejected event waits for a strictly newer mounted scene, never spins on its old token.
    stale_revision: Option<u64>,
}

/// At most one flight and one latest undelivered value exist per current native form field.
#[derive(Default)]
pub(super) struct Forms {
    sink: Option<FormChangeSink>,
    drafts: BTreeMap<String, Draft>,
    sequence: u64,
    queued: bool,
}

impl Forms {
    pub(super) fn cancel(&mut self, id: &str) {
        self.drafts.remove(id);
    }
}

impl PluginView {
    /// Bind actual actor receipts when a host surface owns one; standalone views keep EventSink.
    pub(crate) fn with_form_changes(mut self, sink: Option<FormChangeSink>) -> Self {
        self.forms.sink = sink;
        self
    }

    pub(super) fn cancel_form_delivery(&mut self) {
        self.forms.drafts.clear();
    }

    /// Entity identity rejects type replacement; value_revision is the public explicit-reset epoch.
    fn form_binding(&self, id: &str) -> Option<Binding> {
        let node = self.document.active_node(id)?;
        let (entity, value_revision) = match &node.kind {
            Kind::Input(input) => (self.inputs.get(id)?.state.entity_id(), input.value_revision),
            Kind::Textarea(input) => (
                self.textareas.get(id)?.state.entity_id(),
                input.value_revision,
            ),
            _ => return None,
        };
        Some(Binding {
            entity,
            value_revision,
        })
    }

    /// Click/Submit can follow a latest Change already queued for this scene: the host's single
    /// Worker FIFO keeps that Change first. A coalesced value or an older scene's unacknowledged
    /// flight has not established that ordering, so business input must wait for a new gesture.
    pub(super) fn form_activation_blocked(&self) -> bool {
        self.forms.drafts.iter().any(|(id, draft)| {
            self.form_binding(id) == Some(draft.binding)
                && draft.flight.as_ref().is_none_or(|flight| {
                    flight.sequence != draft.sequence || flight.revision != self.document.revision
                })
        })
    }

    /// A latest queued Change remains ahead of later native activations. Further typing keeps
    /// one pending value while the prior flight waits for its actual Manager admission receipt.
    pub(super) fn form_changed(
        &mut self,
        id: &str,
        entity: EntityId,
        value_revision: u64,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let binding = Binding {
            entity,
            value_revision,
        };
        if self.form_binding(id) != Some(binding) {
            self.forms.cancel(id);
            return;
        }
        let was_blocked = self.form_activation_blocked();
        self.forms.sequence = self.forms.sequence.wrapping_add(1);
        let sequence = self.forms.sequence;
        let draft = self.forms.drafts.entry(id.into()).or_insert_with(|| Draft {
            binding,
            value: String::new(),
            sequence,
            flight: None,
            stale_revision: None,
        });
        draft.value = value;
        draft.sequence = sequence;
        self.flush_form_delivery(window, cx);
        if was_blocked != self.form_activation_blocked() {
            cx.notify();
        }
    }

    /// Disablement, modal withdrawal and node/type/reset changes revoke unsent delivery authority.
    pub(super) fn reconcile_form_delivery(&mut self, cx: &mut Context<Self>) {
        let invalid = self
            .forms
            .drafts
            .iter()
            .filter_map(|(id, draft)| {
                (self.form_binding(id) != Some(draft.binding)).then_some(id.clone())
            })
            .collect::<Vec<_>>();
        for id in invalid {
            self.forms.cancel(&id);
        }
        self.schedule_form_delivery(cx);
    }

    /// Reconciliation has installed the current document before this deferred original-window check.
    pub(super) fn schedule_form_delivery(&mut self, cx: &mut Context<Self>) {
        if self.forms.queued
            || !self.scene_current.get()
            || !self.forms.drafts.values().any(|draft| {
                draft.flight.is_none()
                    && draft
                        .stale_revision
                        .is_none_or(|revision| self.document.revision > revision)
            })
        {
            return;
        }
        self.forms.queued = true;
        let owner = cx.entity().downgrade();
        let window = self.window;
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                let _ = owner.update(cx, |view, cx| {
                    view.forms.queued = false;
                    view.flush_form_delivery(window, cx);
                });
            });
        });
    }

    /// Construct a new Change from the current scene; no old business event is retargeted.
    fn flush_form_delivery(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.scene_current.get() {
            return;
        }
        let was_blocked = self.form_activation_blocked();
        let ids = self.forms.drafts.keys().cloned().collect::<Vec<_>>();
        for id in ids {
            let Some(draft) = self.forms.drafts.get(&id) else {
                continue;
            };
            if self.form_binding(&id) != Some(draft.binding) {
                self.forms.cancel(&id);
                continue;
            }
            if draft.flight.is_some()
                || draft
                    .stale_revision
                    .is_some_and(|revision| self.document.revision <= revision)
            {
                continue;
            }
            let binding = draft.binding;
            let sequence = draft.sequence;
            let event = UiEvent {
                revision: self.document.revision,
                node: id.clone(),
                action: Action::Change(draft.value.clone()),
            };
            if self.document.validate_event(&event).is_err()
                || !self.event_source_current_in(&id, window, cx)
            {
                continue;
            }
            let Some(sink) = self.forms.sink.clone() else {
                // The legacy callback can acknowledge only its synchronous local gate. Never retry
                // a value it already forwarded merely because a guest chooses not to echo it.
                if self.emit_version_in(&id, event.revision, event.action, window, cx) {
                    self.forms.cancel(&id);
                }
                continue;
            };
            let revision = event.revision;
            self.forms.drafts.get_mut(&id).unwrap().flight = Some(Flight {
                sequence,
                revision,
                task: None,
            });
            let receipt = sink(event, cx);
            let task_id = id.clone();
            let task = cx.spawn_in(window, async move |view, cx| {
                let receipt = receipt.await.unwrap_or(FormReceipt::Retired);
                let _ = view.update_in(cx, |view, _, cx| {
                    view.form_received(&task_id, binding, sequence, receipt, cx);
                });
            });
            if let Some(flight) = self
                .forms
                .drafts
                .get_mut(&id)
                .and_then(|draft| draft.flight.as_mut())
            {
                flight.task = Some(task);
            }
        }
        // Repaint only native action affordances; disabling the editable field would revoke
        // the binding and discard the very draft whose delivery this gate protects.
        if was_blocked != self.form_activation_blocked() {
            cx.notify();
        }
    }

    /// Only a typed stale receipt retries. Success terminates even when a plugin ignores/defers echo.
    fn form_received(
        &mut self,
        id: &str,
        binding: Binding,
        sequence: u64,
        receipt: FormReceipt,
        cx: &mut Context<Self>,
    ) {
        let was_blocked = self.form_activation_blocked();
        let Some(draft) = self.forms.drafts.get_mut(id).filter(|draft| {
            draft.binding == binding
                && draft
                    .flight
                    .as_ref()
                    .is_some_and(|flight| flight.sequence == sequence)
        }) else {
            return;
        };
        let revision = draft.flight.take().unwrap().revision;
        match receipt {
            FormReceipt::Delivered if draft.sequence != sequence => draft.stale_revision = None,
            FormReceipt::Stale => draft.stale_revision = Some(revision),
            _ => {
                self.forms.cancel(id);
            }
        }
        self.schedule_form_delivery(cx);
        if was_blocked != self.form_activation_blocked() {
            cx.notify();
        }
    }
}
