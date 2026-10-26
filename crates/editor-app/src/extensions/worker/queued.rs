//! Timed mailboxes preserve Work delivery, resize coalescing and immediate cancellation paths.
use super::*;
use plugin_runtime::observability::{Observation, Outcome, Owner, Stage};
mod mailbox;

/// The admission clock travels with the actual message, never in a shadow keyed queue.
pub(in crate::extensions) struct Envelope {
    work: Work,
    observation: Option<Observation>,
}

/// Real actors time envelope delivery; recorded UI transports retain their original inert messages.
#[derive(Clone)]
pub(in crate::extensions) enum WorkSender {
    Timed {
        sender: mailbox::Sender,
        observations: Arc<Mutex<plugin_runtime::RuntimeObservations>>,
        scope: String,
    },
    #[cfg(test)]
    Recorded(mpsc::Sender<Work>),
}
impl WorkSender {
    /// Preserve the existing recorded UI-message adapter without starting any runtime work.
    #[cfg(test)]
    pub(super) fn recording(sender: mpsc::Sender<Work>) -> Self {
        Self::Recorded(sender)
    }

    /// Send `work`, measuring only attributable messages; failed delivery returns the original Work.
    pub(in crate::extensions) fn send(&self, work: Work) -> Result<(), mpsc::SendError<Work>> {
        match self {
            Self::Timed {
                sender,
                observations,
                scope,
            } => {
                let plugin = match &work {
                    Work::Event(id, ..) => Some(id.clone()),
                    _ => work.lifecycle().map(|progress| progress.id),
                };
                let observation = plugin.map(|id| {
                    observations.lock().unwrap().begin(
                        Owner::host_operation(&id, scope),
                        Stage::Queue,
                        None,
                    )
                });
                sender
                    .send(Envelope { work, observation })
                    .map_err(|error| {
                        let Envelope { work, observation } = error;
                        if let Some(observation) = observation {
                            observation.finish(Outcome::Failed);
                        }
                        mpsc::SendError(work)
                    })
            }
            #[cfg(test)]
            Self::Recorded(sender) => sender.send(work),
        }
    }
    /// Sibling actors route timing to their actual shared store once opening is complete.
    pub(super) fn bind(&self, sink: plugin_runtime::RuntimeObservations) {
        match self {
            Self::Timed { observations, .. } => *observations.lock().unwrap() = sink,
            #[cfg(test)]
            Self::Recorded(_) => {}
        }
    }
    /// Paused producers cannot extend an unconfirmed native input chain.
    pub(in crate::extensions) fn input_paused(
        &self,
        plugin: &str,
        epoch: u64,
        panel: Option<&str>,
    ) -> bool {
        match self {
            Self::Timed { sender, .. } => sender.input_paused(plugin, epoch, panel),
            #[cfg(test)]
            Self::Recorded(_) => false,
        }
    }
    /// Resume follows original input and native cancellation receipts in FIFO order.
    pub(in crate::extensions) fn resume_input(
        &self,
        plugin: &str,
        epoch: u64,
        panel: Option<&str>,
    ) -> bool {
        match self {
            Self::Timed { sender, .. } => sender.resume_input(plugin, epoch, panel),
            #[cfg(test)]
            Self::Recorded(_) => true,
        }
    }
    /// Resume acknowledgement comes from serialized actor dispatch, never an eager dequeue.
    pub(super) fn acknowledge_input_resume(&self, plugin: &str, epoch: u64, panel: Option<&str>) {
        if let Self::Timed { sender, .. } = self {
            sender.acknowledge_resume(plugin, epoch, panel);
        }
    }
    /// Retired incarnations cannot retain pause metadata or reopen a replacement's input chain.
    pub(super) fn retain_input_owners(&self, published: &Published) {
        if let Self::Timed { sender, .. } = self {
            sender.retain_input_owners(|plugin, epoch| {
                published.live_instance_epoch(plugin) == Some(epoch)
            });
        }
    }
}

/// Channel waiting ends at first dequeue, not at later preparation or guest execution.
pub(super) struct WorkReceiver(mailbox::Receiver);
impl WorkReceiver {
    /// Keep the actor's existing receive deadline and disconnect behavior.
    pub(super) fn recv_timeout(&self, timeout: Duration) -> Result<Work, mpsc::RecvTimeoutError> {
        self.0.recv_timeout(timeout).map(received)
    }
}
/// Coalescing uses the same interface for real envelopes and existing raw test messages.
pub(super) trait WorkInbox {
    fn try_work(&self) -> Result<Work, mpsc::TryRecvError>;
}
impl WorkInbox for WorkReceiver {
    fn try_work(&self) -> Result<Work, mpsc::TryRecvError> {
        self.0.try_recv().map(received)
    }
}
impl WorkInbox for mpsc::Receiver<Work> {
    fn try_work(&self) -> Result<Work, mpsc::TryRecvError> {
        self.try_recv()
    }
}

/// Allocate one mailbox with its original scope; Owner hashes that scope before retaining metadata.
pub(super) fn channel(
    scope: &str,
    sink: plugin_runtime::RuntimeObservations,
) -> (WorkSender, WorkReceiver) {
    let (sender, receiver) = mailbox::channel();
    (
        WorkSender::Timed {
            sender,
            observations: Arc::new(Mutex::new(sink)),
            scope: scope.into(),
        },
        WorkReceiver(receiver),
    )
}
/// A successful dequeue is a queue receipt, never evidence that its operation completed.
fn received(envelope: Envelope) -> Work {
    if let Some(observation) = envelope.observation {
        observation.finish(Outcome::Completed);
    }
    envelope.work
}

/// A stalled real actor rejects excess work immediately instead of allocating or blocking its UI.
#[test]
fn native_mailbox_bounds_stalled_ingress_without_blocking_the_sender() {
    let (sender, receiver) = channel("bounded-native", Default::default());
    for _ in 0..256 {
        sender.send(Work::Disable("fixture".into())).unwrap();
    }
    assert!(
        sender.send(Work::Disable("excess".into())).is_err(),
        "a full mailbox must reject its producer"
    );
    for _ in 0..256 {
        assert!(
            matches!(receiver.recv_timeout(Duration::ZERO).unwrap(),Work::Disable(id) if id=="fixture")
        );
    }
}

/// Native process input owns bytes too; it cannot bypass the mailbox's aggregate memory quota.
#[test]
fn native_mailbox_byte_budget_includes_owned_execution_input() {
    let (sender, _receiver) = channel("bounded-native-bytes", Default::default());
    assert!(
        sender
            .send(Work::ExecutionInput {
                session: 7,
                bytes: vec![0; 32 * 1024 * 1024 + 1]
            })
            .is_err()
    );
}

/// Form drafts own their text until the actor replies and share native input memory limits.
#[test]
fn native_mailbox_byte_budget_includes_form_drafts() {
    let (sender, _receiver) = channel("bounded-form-bytes", Default::default());
    let (reply, _receipt) = futures::channel::oneshot::channel();
    assert!(
        sender
            .send(Work::FormChange {
                plugin: "fixture".into(),
                epoch: 1,
                panel: "panel".into(),
                event: ui::UiEvent {
                    revision: 1,
                    node: "input".into(),
                    action: ui::Action::Change("x".repeat(32 * 1024 * 1024))
                },
                reply
            })
            .is_err()
    );
    assert!(sender.input_paused("fixture", 1, Some("panel")));
}

/// A real key release retains finite reserve even when ordinary input used its byte budget.
#[test]
fn native_mailbox_keeps_key_release_after_a_full_payload_prefix() {
    let (sender, receiver) = channel("native-release-reserve", Default::default());
    sender
        .send(Work::ExecutionInput {
            session: 7,
            bytes: vec![0; 32 * 1024 * 1024 - 256],
        })
        .unwrap();
    sender.send(native_key(ui::CanvasKeyPhase::Up, 2)).unwrap();
    assert!(matches!(
        receiver.recv_timeout(Duration::ZERO).unwrap(),
        Work::ExecutionInput { session: 7, .. }
    ));
    assert!(matches!(
        receiver.recv_timeout(Duration::ZERO).unwrap(),
        Work::Event(
            _,
            _,
            _,
            api::Notification::Ui(ui::UiEvent {
                action: ui::Action::Canvas(ui::CanvasEvent::Input {
                    input: ui::CanvasInput {
                        kind: ui::CanvasInputKind::Key {
                            phase: ui::CanvasKeyPhase::Up,
                            ..
                        },
                        ..
                    }
                }),
                ..
            })
        )
    ));
}

/// Construct genuine public Canvas key payloads without a fake runtime or a hidden delivery path.
#[cfg(test)]
fn native_key(phase: ui::CanvasKeyPhase, sequence: u64) -> Work {
    Work::Event(
        "fixture".into(),
        1,
        Some("panel".into()),
        api::Notification::Ui(ui::UiEvent {
            revision: 1,
            node: "drawing".into(),
            action: ui::Action::Canvas(ui::CanvasEvent::Input {
                input: ui::CanvasInput {
                    identity: 81,
                    presented_revision: 1,
                    target: "cell".into(),
                    epoch: 1,
                    sequence,
                    source: None,
                    file: None,
                    projection: None,
                    kind: ui::CanvasInputKind::Key {
                        phase,
                        key: "a".into(),
                        code: None,
                        repeat: false,
                        modifiers: Default::default(),
                    },
                },
            }),
        }),
    )
}

/// A new outer UI publication may replace an overlay owner while keeping inner target IDs.
#[test]
fn native_mailbox_move_coalescing_keeps_ui_publication_boundaries() {
    let (sender, receiver) = channel("native-owner-boundary", Default::default());
    sender.send(native_move(1, 1)).unwrap();
    sender.send(native_move(2, 2)).unwrap();
    assert_eq!(
        native_sequence(receiver.recv_timeout(Duration::ZERO).unwrap()),
        1
    );
    assert_eq!(
        native_sequence(receiver.recv_timeout(Duration::ZERO).unwrap()),
        2
    );
}

/// Build real pointer samples whose sequence and outer publication are independently observable.
#[cfg(test)]
fn native_move(sequence: u64, revision: u64) -> Work {
    let mut work = native_key(ui::CanvasKeyPhase::Down, sequence);
    if let Work::Event(_, _, _, api::Notification::Ui(event)) = &mut work {
        event.revision = revision;
        if let ui::Action::Canvas(ui::CanvasEvent::Input { input }) = &mut event.action {
            input.kind = ui::CanvasInputKind::Pointer(ui::CanvasPointerInput {
                pointer: 1,
                device: ui::CanvasPointerDevice::Mouse,
                phase: ui::CanvasPointerPhase::Move,
                x: sequence as f32,
                y: 0.,
                button: None,
                buttons: None,
                clicks: None,
                pressure: None,
                modifiers: Default::default(),
                cancel_reason: None,
            });
        }
    }
    work
}
/// Extract the public sequence delivered through the actor's normal receive boundary.
#[cfg(test)]
fn native_sequence(work: Work) -> u64 {
    let Work::Event(
        _,
        _,
        _,
        api::Notification::Ui(ui::UiEvent {
            action: ui::Action::Canvas(ui::CanvasEvent::Input { input }),
            ..
        }),
    ) = work
    else {
        panic!("expected native input")
    };
    input.sequence
}

/// FIFO cancellation and actor acknowledgement prevent prefetch from reopening an old gesture.
#[test]
fn native_mailbox_resume_waits_for_actor_acknowledgement() {
    let (sender, receiver) = channel("native-resume-order", Default::default());
    for _ in 0..256 {
        sender.send(Work::Disable("prefix".into())).unwrap();
    }
    assert!(
        sender
            .send(native_key(ui::CanvasKeyPhase::Down, 1))
            .is_err()
    );
    assert!(sender.input_paused("fixture", 1, Some("panel")));
    sender.send(native_key(ui::CanvasKeyPhase::Up, 2)).unwrap();
    assert!(sender.resume_input("fixture", 1, Some("panel")));
    for _ in 0..256 {
        assert!(matches!(
            receiver.recv_timeout(Duration::ZERO).unwrap(),
            Work::Disable(_)
        ));
    }
    assert!(matches!(
        receiver.recv_timeout(Duration::ZERO).unwrap(),
        Work::InputBackpressure { resume: false, .. }
    ));
    assert_eq!(
        native_sequence(receiver.recv_timeout(Duration::ZERO).unwrap()),
        2
    );
    assert!(matches!(
        receiver.recv_timeout(Duration::ZERO).unwrap(),
        Work::InputBackpressure { resume: true, .. }
    ));
    assert!(
        sender
            .send(native_key(ui::CanvasKeyPhase::Down, 3))
            .is_err()
    );
    sender.acknowledge_input_resume("fixture", 1, Some("panel"));
    sender
        .send(native_key(ui::CanvasKeyPhase::Down, 4))
        .unwrap();
    assert_eq!(
        native_sequence(receiver.recv_timeout(Duration::ZERO).unwrap()),
        4
    );
}

/// Worker teardown keeps one dedicated finite slot even when ordinary producers have stalled it.
#[test]
fn native_mailbox_preserves_shutdown_after_full_input() {
    let (sender, receiver) = channel("native-shutdown-reserve", Default::default());
    for _ in 0..256 {
        sender.send(Work::Disable("prefix".into())).unwrap();
    }
    assert!(sender.send(Work::Shutdown(None)).is_ok());
    for _ in 0..256 {
        assert!(matches!(
            receiver.recv_timeout(Duration::ZERO).unwrap(),
            Work::Disable(_)
        ));
    }
    assert!(matches!(
        receiver.recv_timeout(Duration::ZERO).unwrap(),
        Work::Shutdown(None)
    ));
}

/// Movement replacement never crosses real key barriers or modifier changes.
#[test]
fn native_mailbox_coalesces_only_adjacent_matching_samples() {
    let (sender, receiver) = channel("native-motion-order", Default::default());
    sender.send(native_move(1, 1)).unwrap();
    sender.send(native_move(2, 1)).unwrap();
    sender
        .send(native_key(ui::CanvasKeyPhase::Down, 3))
        .unwrap();
    sender.send(native_move(4, 1)).unwrap();
    let mut changed = native_move(5, 1);
    if let Work::Event(
        _,
        _,
        _,
        api::Notification::Ui(ui::UiEvent {
            action: ui::Action::Canvas(ui::CanvasEvent::Input { input }),
            ..
        }),
    ) = &mut changed
    {
        if let ui::CanvasInputKind::Pointer(pointer) = &mut input.kind {
            pointer.modifiers.shift = true;
        }
    }
    sender.send(changed).unwrap();
    for expected in [2, 3, 4, 5] {
        assert_eq!(
            native_sequence(receiver.recv_timeout(Duration::ZERO).unwrap()),
            expected
        );
    }
    assert!(receiver.try_work().is_err());
}

/// Two-axis absolute offsets replace only samples from the same retained content epoch.
#[test]
fn native_mailbox_coalesces_two_axis_offsets_without_crossing_epochs() {
    let (sender, receiver) = channel("native-scroll-order", Default::default());
    let scroll = |epoch, x| {
        Work::Event(
            "fixture".into(),
            1,
            Some("panel".into()),
            api::Notification::Ui(ui::UiEvent {
                revision: 1,
                node: "drawing".into(),
                action: ui::Action::Canvas(ui::CanvasEvent::Scroll2D {
                    identity: 81,
                    source: None,
                    file: None,
                    epoch,
                    offset: ui::SceneOffset { x, y: 4. },
                }),
            }),
        )
    };
    sender.send(scroll(1, 1.)).unwrap();
    sender.send(scroll(1, 2.)).unwrap();
    sender.send(scroll(2, 3.)).unwrap();
    for expected in [(1, 2.), (2, 3.)] {
        let Work::Event(
            _,
            _,
            _,
            api::Notification::Ui(ui::UiEvent {
                action: ui::Action::Canvas(ui::CanvasEvent::Scroll2D { epoch, offset, .. }),
                ..
            }),
        ) = receiver.recv_timeout(Duration::ZERO).unwrap()
        else {
            panic!("expected two-axis offset")
        };
        assert_eq!((epoch, offset.x), expected);
    }
    assert!(receiver.try_work().is_err());
}

/// Deliberate mailbox delay is charged to queue waiting, without running a guest callback.
#[test]
fn developer_tooling_mailbox_wait_has_its_own_receipt() {
    let sink = plugin_runtime::RuntimeObservations::default();
    sink.start_capture();
    let (sender, receiver) = channel("private-workspace-marker", sink.clone());
    sender.send(Work::Disable("fixture".into())).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    assert!(matches!(
        receiver.recv_timeout(Duration::from_secs(1)).unwrap(),
        Work::Disable(_)
    ));
    let snapshot = sink.snapshot(None);
    assert_eq!(snapshot.records.len(), 1);
    assert_eq!(snapshot.records[0].stage, Stage::Queue);
    assert!(snapshot.records[0].elapsed_us >= 20_000);
    assert!(snapshot.unobserved.contains(&Stage::Guest));
    assert!(
        !serde_json::to_string(&snapshot)
            .unwrap()
            .contains("private-workspace-marker")
    );
}
