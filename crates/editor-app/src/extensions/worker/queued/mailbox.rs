//! A finite FIFO retains terminal input and reports overload before a producer can resume.
use super::*;
use plugin_runtime::plugin_protocol as protocol;
use std::{
    collections::{BTreeSet, VecDeque},
    sync::Condvar,
    time::Instant,
};
const MAX_WORK: usize = 256;
const MAX_CONTROL: usize = 1024;
const MAX_BYTES: usize = 32 * 1024 * 1024;
/// Terminal receipts retain finite space even when ordinary serialized payloads fill their budget.
const CONTROL_BYTES: usize = 1024 * 1024;
/// Incarnation and panel bind pause/resume to exactly the native producer that overflowed.
type Surface = (String, u64, Option<String>);
/// Admission accounting travels with each FIFO entry and is released at dequeue.
struct Entry {
    envelope: Envelope,
    bytes: usize,
    control: bool,
}
/// Ordinary data, terminal cleanup and pause metadata all have independent finite limits.
struct State {
    queue: VecDeque<Entry>,
    bytes: usize,
    normal: usize,
    control: usize,
    senders: usize,
    receiver: bool,
    paused: BTreeSet<Surface>,
    resuming: BTreeSet<Surface>,
    fatal: bool,
    fatal_reported: bool,
    shutdown: Option<Envelope>,
}
/// A single consumer waits on readiness; native producers never wait for capacity.
struct Mailbox {
    state: Mutex<State>,
    ready: Condvar,
}
/// Nonblocking admission returns ownership of rejected work to its caller.
pub(super) struct Sender(Arc<Mailbox>);
/// Only the serialized runtime actor consumes admitted work.
pub(super) struct Receiver(Arc<Mailbox>);

/// Allocate one finite ingress with a separate idempotent worker-shutdown slot.
pub(super) fn channel() -> (Sender, Receiver) {
    let mailbox = Arc::new(Mailbox {
        state: Mutex::new(State {
            queue: VecDeque::new(),
            bytes: 0,
            normal: 0,
            control: 0,
            senders: 1,
            receiver: true,
            paused: BTreeSet::new(),
            resuming: BTreeSet::new(),
            fatal: false,
            fatal_reported: false,
            shutdown: None,
        }),
        ready: Condvar::new(),
    });
    (Sender(mailbox.clone()), Receiver(mailbox))
}
impl Clone for Sender {
    fn clone(&self) -> Self {
        self.0.state.lock().unwrap().senders += 1;
        Self(self.0.clone())
    }
}
impl Drop for Sender {
    fn drop(&mut self) {
        self.0.state.lock().unwrap().senders -= 1;
        self.0.ready.notify_all();
    }
}
impl Drop for Receiver {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap();
        state.receiver = false;
        for entry in state.queue.drain(..) {
            if let Some(observation) = entry.envelope.observation {
                observation.finish(Outcome::Cancelled);
            }
        }
        if let Some(Envelope {
            observation: Some(observation),
            ..
        }) = state.shutdown.take()
        {
            observation.finish(Outcome::Cancelled);
        }
        self.0.ready.notify_all();
    }
}
impl Sender {
    /// Preserve FIFO for admitted work and reject overflow synchronously without blocking the UI.
    pub(super) fn send(&self, envelope: Envelope) -> Result<(), Envelope> {
        let bytes = encoded_bytes(&envelope.work);
        let owner = surface(&envelope.work);
        let control = terminal(&envelope.work);
        let mut state = self.0.state.lock().unwrap();
        if !state.receiver {
            return Err(envelope);
        }
        if matches!(&envelope.work, Work::Shutdown(_)) {
            if state
                .shutdown
                .as_ref()
                .is_some_and(|pending| matches!(&pending.work, Work::Shutdown(Some(_))))
            {
                if matches!(&envelope.work, Work::Shutdown(Some(_))) {
                    return Err(envelope);
                }
                if let Some(observation) = envelope.observation {
                    observation.finish(Outcome::Cancelled);
                }
            } else {
                if let Some(Envelope {
                    observation: Some(observation),
                    ..
                }) = state.shutdown.replace(envelope)
                {
                    observation.finish(Outcome::Cancelled);
                }
            }
            self.0.ready.notify_one();
            return Ok(());
        }
        if state.fatal {
            return Err(envelope);
        }
        if owner.as_ref().is_some_and(|key| state.paused.contains(key)) && !control {
            return Err(envelope);
        }
        if let Some(previous) = state.queue.back()
            && same_movement(&previous.envelope.work, &envelope.work)
            && state.bytes - previous.bytes + bytes <= MAX_BYTES
        {
            let previous = state.queue.pop_back().unwrap();
            state.bytes -= previous.bytes;
            if let Some(observation) = previous.envelope.observation {
                observation.finish(Outcome::Cancelled);
            }
            state.bytes += bytes;
            state.queue.push_back(Entry {
                envelope,
                bytes,
                control: false,
            });
            self.0.ready.notify_one();
            return Ok(());
        }
        let byte_limit = MAX_BYTES + if control { CONTROL_BYTES } else { 0 };
        let full = bytes > byte_limit
            || state.bytes.saturating_add(bytes) > byte_limit
            || if control {
                state.control >= MAX_CONTROL
            } else {
                state.normal >= MAX_WORK
            };
        if full {
            if let Some(key) = owner {
                if state.paused.len() >= MAX_WORK * 4 && !state.paused.contains(&key) {
                    state.fatal = true;
                } else if state.paused.insert(key.clone()) {
                    push_marker(&mut state, key, false);
                }
            } else if !state.queue.iter().any(|entry| {
                matches!(
                    &entry.envelope.work,
                    Work::InputBackpressure {
                        plugin: None,
                        resume: false,
                        ..
                    }
                )
            }) {
                push_marker(&mut state, (String::new(), 0, None), false);
            }
            self.0.ready.notify_one();
            return Err(envelope);
        }
        state.bytes += bytes;
        if control {
            state.control += 1;
        } else {
            state.normal += 1;
        }
        state.queue.push_back(Entry {
            envelope,
            bytes,
            control,
        });
        self.0.ready.notify_one();
        Ok(())
    }
    /// A paused producer must discard unconfirmed drafts before requesting a FIFO resume marker.
    pub(super) fn input_paused(&self, plugin: &str, epoch: u64, panel: Option<&str>) -> bool {
        let state = self.0.state.lock().unwrap();
        state.fatal
            || state
                .paused
                .contains(&(plugin.into(), epoch, panel.map(str::to_owned)))
    }
    /// Repeated UI polls enqueue at most one resume receipt per paused surface.
    pub(super) fn resume_input(&self, plugin: &str, epoch: u64, panel: Option<&str>) -> bool {
        let mut state = self.0.state.lock().unwrap();
        let key = (plugin.into(), epoch, panel.map(str::to_owned));
        if state.fatal || !state.receiver {
            return false;
        }
        if state.paused.contains(&key) && state.resuming.insert(key.clone()) {
            push_marker(&mut state, key, true);
            self.0.ready.notify_one();
        }
        true
    }
    /// Coalescing may prefetch resume; only actor dispatch acknowledges the completed FIFO prefix.
    pub(super) fn acknowledge_resume(&self, plugin: &str, epoch: u64, panel: Option<&str>) {
        let mut state = self.0.state.lock().unwrap();
        let key = (plugin.into(), epoch, panel.map(str::to_owned));
        state.paused.remove(&key);
        state.resuming.remove(&key);
    }
    /// Cleanup follows real runtime ownership publication, not guessed panel lifetimes.
    pub(super) fn retain_input_owners(&self, mut current: impl FnMut(&str, u64) -> bool) {
        let mut state = self.0.state.lock().unwrap();
        state
            .paused
            .retain(|(plugin, epoch, _)| current(plugin, *epoch));
        state
            .resuming
            .retain(|(plugin, epoch, _)| current(plugin, *epoch));
    }
}
impl Receiver {
    /// Return the next FIFO entry without waiting; resize sampling uses this same boundary.
    pub(super) fn try_recv(&self) -> Result<Envelope, mpsc::TryRecvError> {
        let mut state = self.0.state.lock().unwrap();
        if let Some(envelope) = take(&mut state) {
            return Ok(envelope);
        }
        Err(if state.senders == 0 {
            mpsc::TryRecvError::Disconnected
        } else {
            mpsc::TryRecvError::Empty
        })
    }
    /// Wait only for readiness or disconnect, preserving the actor's business-timer deadline.
    pub(super) fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Envelope, mpsc::RecvTimeoutError> {
        let deadline = Instant::now() + timeout;
        let mut state = self.0.state.lock().unwrap();
        loop {
            if let Some(envelope) = take(&mut state) {
                return Ok(envelope);
            }
            if state.senders == 0 {
                return Err(mpsc::RecvTimeoutError::Disconnected);
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(mpsc::RecvTimeoutError::Timeout);
            }
            state = self.0.ready.wait_timeout(state, deadline - now).unwrap().0;
        }
    }
}
/// A finite control reserve holds cancellation/error/resume after the original FIFO prefix.
fn push_marker(state: &mut State, key: Surface, resume: bool) {
    if state.control >= MAX_CONTROL {
        state.fatal = true;
        return;
    }
    let (plugin, epoch, panel) = key;
    state.control += 1;
    state.queue.push_back(Entry {
        envelope: Envelope {
            work: Work::InputBackpressure {
                plugin: (!plugin.is_empty()).then_some(plugin),
                epoch,
                panel,
                resume,
            },
            observation: None,
        },
        bytes: 0,
        control: true,
    });
}
/// Release admission accounting before yielding work, then report catastrophic overload once.
fn take(state: &mut State) -> Option<Envelope> {
    if let Some(entry) = state.queue.pop_front() {
        state.bytes -= entry.bytes;
        if entry.control {
            state.control -= 1;
        } else {
            state.normal -= 1;
        }
        return Some(entry.envelope);
    }
    if state.fatal && !state.fatal_reported {
        state.fatal_reported = true;
        return Some(Envelope {
            work: Work::InputBackpressure {
                plugin: None,
                epoch: 0,
                panel: None,
                resume: false,
            },
            observation: None,
        });
    }
    state.shutdown.take()
}
/// Count event bytes without allocating another copy of its text or IME payload.
fn encoded_bytes(work: &Work) -> usize {
    struct Count(usize);
    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(bytes.len());
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    if let Work::ExecutionInput { bytes, .. } | Work::PresentedInput { bytes, .. } = work {
        bytes.len().saturating_add(256)
    } else if let Work::Event(_, _, _, event) = work {
        let mut count = Count(0);
        if serde_json::to_writer(&mut count, event).is_err() {
            return usize::MAX;
        }
        count.0.saturating_add(256)
    } else if let Work::FormChange { event, .. } | Work::CanvasResize { event, .. } = work {
        let mut count = Count(0);
        if serde_json::to_writer(&mut count, event).is_err() {
            return usize::MAX;
        }
        count.0.saturating_add(256)
    } else {
        256
    }
}
fn surface(work: &Work) -> Option<Surface> {
    match work {
        Work::Event(plugin, epoch, panel, api::Notification::Ui(_)) => {
            Some((plugin.clone(), *epoch, panel.clone()))
        }
        Work::Transfer {
            plugin,
            epoch,
            panel,
            ..
        } => Some((plugin.clone(), *epoch, Some(panel.clone()))),
        Work::FormChange {
            plugin,
            epoch,
            panel,
            ..
        }
        | Work::CanvasResize {
            plugin,
            epoch,
            panel,
            ..
        } => Some((plugin.clone(), *epoch, Some(panel.clone()))),
        _ => None,
    }
}
/// Real releases, cancellation and OS drop completion never coalesce or use ordinary capacity.
fn terminal(work: &Work) -> bool {
    match work {
        Work::Transfer { ingress, .. } => matches!(
            ingress,
            crate::ui::plugin::transfer::Ingress::Update {
                phase: protocol::transfer::Phase::Leave
                    | protocol::transfer::Phase::Drop
                    | protocol::transfer::Phase::Cancel,
                ..
            } | crate::ui::plugin::transfer::Ingress::FinishDrag { .. }
        ),
        Work::Event(_, _, _, api::Notification::Ui(event)) => match canvas_action(&event.action) {
            Some(ui::CanvasEvent::Input { input }) => matches!(
                &input.kind,
                ui::CanvasInputKind::Focus { focused: false }
                    | ui::CanvasInputKind::Key {
                        phase: ui::CanvasKeyPhase::Up,
                        ..
                    }
                    | ui::CanvasInputKind::Composition(ui::CanvasComposition {
                        phase: ui::CanvasCompositionPhase::Cancel,
                        ..
                    })
                    | ui::CanvasInputKind::Pointer(ui::CanvasPointerInput {
                        phase: ui::CanvasPointerPhase::Up | ui::CanvasPointerPhase::Cancel,
                        ..
                    })
            ),
            Some(
                ui::CanvasEvent::Focus { focused: false }
                | ui::CanvasEvent::Pointer {
                    phase: ui::PointerPhase::Up,
                    ..
                },
            ) => true,
            _ => false,
        },
        _ => false,
    }
}
/// Only adjacent samples with identical semantic owners and gesture/modifier states can replace.
fn same_movement(a: &Work, b: &Work) -> bool {
    let (
        Work::Event(ap, ae, asurface, api::Notification::Ui(a)),
        Work::Event(bp, be, bsurface, api::Notification::Ui(b)),
    ) = (a, b)
    else {
        return false;
    };
    if (ap, ae, asurface, &a.node, a.revision) != (bp, be, bsurface, &b.node, b.revision) {
        return false;
    }
    match (canvas_action(&a.action), canvas_action(&b.action)) {
        (
            Some(ui::CanvasEvent::Scroll2D {
                identity: ai,
                source: asrc,
                file: af,
                epoch: ae,
                ..
            }),
            Some(ui::CanvasEvent::Scroll2D {
                identity: bi,
                source: bsrc,
                file: bf,
                epoch: be,
                ..
            }),
        ) => (ai, asrc, af, ae) == (bi, bsrc, bf, be),
        (Some(ui::CanvasEvent::Input { input: a }), Some(ui::CanvasEvent::Input { input: b })) => {
            let (ui::CanvasInputKind::Pointer(ap), ui::CanvasInputKind::Pointer(bp)) =
                (&a.kind, &b.kind)
            else {
                return false;
            };
            ap.phase == ui::CanvasPointerPhase::Move
                && bp.phase == ui::CanvasPointerPhase::Move
                && (
                    a.identity,
                    a.presented_revision,
                    &a.target,
                    a.epoch,
                    &a.source,
                    &a.file,
                    &a.projection,
                ) == (
                    b.identity,
                    b.presented_revision,
                    &b.target,
                    b.epoch,
                    &b.source,
                    &b.file,
                    &b.projection,
                )
                && (ap.pointer, &ap.device, ap.button, &ap.buttons, ap.modifiers)
                    == (bp.pointer, &bp.device, bp.button, &bp.buttons, bp.modifiers)
        }
        _ => false,
    }
}
/// Viewport-wrapped Canvas input retains the same terminal and coalescing guarantees.
fn canvas_action(action: &ui::Action) -> Option<&ui::CanvasEvent> {
    match action {
        ui::Action::Canvas(event) | ui::Action::ViewportInput(ui::ViewportInput { event, .. }) => {
            Some(event)
        }
        _ => None,
    }
}
