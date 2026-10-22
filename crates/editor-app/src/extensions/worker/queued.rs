//! Timed mailboxes preserve Work delivery, resize coalescing and immediate cancellation paths.
use super::*;
use plugin_runtime::observability::{Observation, Outcome, Owner, Stage};

/// The admission clock travels with the actual message, never in a shadow keyed queue.
pub(in crate::extensions) struct Envelope {
    work: Work,
    observation: Option<Observation>,
}

/// Real actors time envelope delivery; recorded UI transports retain their original inert messages.
#[derive(Clone)]
pub(in crate::extensions) enum WorkSender {
    Timed {
        sender: mpsc::Sender<Envelope>,
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
                        let Envelope { work, observation } = error.0;
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
}

/// Channel waiting ends at first dequeue, not at later preparation or guest execution.
pub(super) struct WorkReceiver(mpsc::Receiver<Envelope>);
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
    let (sender, receiver) = mpsc::channel();
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
