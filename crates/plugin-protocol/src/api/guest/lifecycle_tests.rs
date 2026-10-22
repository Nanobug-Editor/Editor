//! Observable SDK lifecycle behavior through a private, failure-injecting host transport.
use super::*;

/// A completed but not yet delivered request cannot prevent the next user intent from starting.
#[test]
fn lifecycle_intent_replaces_completed_request_before_terminal_delivery() {
    let mut next = 0;
    with_host(
        move |operation| match operation {
            Operation::Editor { .. } => {
                next += 1;
                Ok(Value::Accepted(handle(next)))
            }
            Operation::CancelRequest { .. } => {
                Err(Failure::new(ErrorCode::InvalidState, "Already terminal"))
            }
            Operation::CloseResource { .. } => Ok(Value::Unit),
            _ => panic!("unexpected SDK operation"),
        },
        || {
            let mut intent = EditorIntent::default();
            assert_eq!(
                intent
                    .replace(
                        EditorOperation::ListDocuments,
                        1000,
                        CancelMode::StopWaiting
                    )
                    .unwrap(),
                1
            );
            assert_eq!(
                intent
                    .replace(
                        EditorOperation::ListDocuments,
                        1000,
                        CancelMode::StopWaiting
                    )
                    .unwrap(),
                2
            );
            assert!(intent.close().is_complete());
        },
    );
}
use std::{cell::RefCell, rc::Rc};

/// Per-test host adapters stay thread-local and never become a guest or host capability.
type TestHost = Box<dyn FnMut(Operation) -> Result<Value, Failure>>;
thread_local! {
    /// Each test owns its independently scoped transport; parallel tests cannot replace it.
    static HOST: RefCell<Option<TestHost>> = RefCell::new(None);
}

/// Intercept an SDK operation when a scoped test adapter is installed.
pub(super) fn intercept(operation: &Operation) -> Option<Result<Value, Failure>> {
    HOST.with(|host| {
        host.borrow_mut()
            .as_mut()
            .map(|call| call(operation.clone()))
    })
}

/// Restore the production transport even when a lifecycle assertion panics.
struct RestoreHost;
impl Drop for RestoreHost {
    fn drop(&mut self) {
        HOST.with(|host| *host.borrow_mut() = None);
    }
}

/// Run the public SDK against `call`; `test` observes resources and returned outcomes.
fn with_host(call: impl FnMut(Operation) -> Result<Value, Failure> + 'static, test: impl FnOnce()) {
    HOST.with(|host| *host.borrow_mut() = Some(Box::new(call)));
    let _restore = RestoreHost;
    test();
}

/// Distinct identities let tests reject late receipts independently of their numeric slots.
fn handle(resource: u64) -> ResourceHandle {
    ResourceHandle {
        instance: "guest-a".into(),
        scope: "workspace-a".into(),
        resource,
    }
}

/// Closing a resource group continues after failure and does not reclose fired one-shot timers.
#[test]
fn lifecycle_group_retries_only_failed_owned_resources() {
    let closed = Rc::new(RefCell::new(Vec::new()));
    let observed = closed.clone();
    let mut next = 0;
    let mut fail = true;
    with_host(
        move |operation| match operation {
            Operation::Timer { .. } | Operation::SubscribeDocumentEvents => {
                next += 1;
                Ok(Value::Resource(handle(next)))
            }
            Operation::CloseResource { handle } => {
                observed.borrow_mut().push(handle.resource);
                if handle.resource == 3 && fail {
                    fail = false;
                    Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "injected refusal",
                    ))
                } else {
                    Ok(Value::Unit)
                }
            }
            _ => panic!("unexpected SDK operation"),
        },
        || {
            let mut group = ResourceGroup::default();
            group.timer(100, true).unwrap();
            let once = group.timer(100, false).unwrap();
            group.documents().unwrap();
            group.observe(&Notification::Timer {
                handle: once,
                sequence: 1,
            });
            let first = group.close_all();
            assert_eq!(first.closed, 1);
            assert_eq!(first.pending.len(), 1);
            assert_eq!(first.pending[0].code, ErrorCode::PermissionDenied);
            assert_eq!(*closed.borrow(), [3, 1]);
            assert!(group.close_all().is_complete());
            assert!(group.close_all().is_complete());
            assert_eq!(*closed.borrow(), [3, 1, 3]);
        },
    );
}

/// A replaced intent rejects late results, and host-retired terminal requests are not reclosed.
#[test]
fn lifecycle_intent_keeps_only_the_current_result() {
    let closed = Rc::new(RefCell::new(Vec::new()));
    let observed = closed.clone();
    let mut next = 0;
    with_host(
        move |operation| match operation {
            Operation::Editor { .. } => {
                next += 1;
                Ok(Value::Accepted(handle(next)))
            }
            Operation::CancelRequest { .. } => {
                Ok(Value::Cancellation(CancellationEffect::WaitingStopped))
            }
            Operation::CloseResource { handle } => {
                observed.borrow_mut().push(handle.resource);
                Ok(Value::Unit)
            }
            _ => panic!("unexpected SDK operation"),
        },
        || {
            let mut intent = EditorIntent::default();
            intent
                .replace(
                    EditorOperation::ListDocuments,
                    1000,
                    CancelMode::StopWaiting,
                )
                .unwrap();
            intent
                .replace(
                    EditorOperation::ListDocuments,
                    1000,
                    CancelMode::StopWaiting,
                )
                .unwrap();
            let completed = |id| Notification::Request {
                handle: handle(id),
                update: RequestUpdate::Completed {
                    result: Ok(EditorValue::Unit),
                },
            };
            assert!(intent.update(&completed(1)).is_none());
            assert!(intent.update(&completed(2)).is_some());
            assert!(intent.update(&completed(2)).is_none());
            assert!(intent.close().is_complete());
            assert_eq!(*closed.borrow(), [1]);
        },
    );
}

/// A failed timer allocation closes its new stream; a successful transfer releases both resources.
#[test]
fn lifecycle_http_owns_partial_initialization_and_terminal_cleanup() {
    use crate::http::{self, HttpTask};
    let closed = Rc::new(RefCell::new(Vec::new()));
    let observed = closed.clone();
    let mut next = 0;
    let mut refuse_timer = true;
    let mut events = std::collections::VecDeque::from([
        http::Event::Headers {
            status: 404,
            headers: vec![],
        },
        http::Event::Chunk {
            bytes: vec![0xe4, 0xbd],
        },
        http::Event::Finished { bytes: 2 },
    ]);
    with_host(
        move |operation| match operation {
            Operation::Http {
                operation: http::Operation::Start { .. },
            } => {
                next += 1;
                Ok(Value::Resource(handle(next)))
            }
            Operation::Timer { .. } if refuse_timer => {
                refuse_timer = false;
                Err(Failure::new(ErrorCode::LimitExceeded, "timer budget"))
            }
            Operation::Timer { .. } => {
                next += 1;
                Ok(Value::Resource(handle(next)))
            }
            Operation::Http {
                operation: http::Operation::Read { .. },
            } => Ok(Value::Http(events.pop_front())),
            Operation::CloseResource { handle } => {
                observed.borrow_mut().push(handle.resource);
                Ok(Value::Unit)
            }
            _ => panic!("unexpected SDK operation"),
        },
        || {
            let api = Negotiated {
                base: semver::Version::new(1, 0, 0),
                capabilities: [
                    ("network.http".into(), semver::Version::new(1, 0, 0)),
                    ("lifecycle.timers".into(), semver::Version::new(1, 0, 0)),
                ]
                .into(),
            };
            let failed = HttpTask::start(&api, http::Request::get("https://example.test"), 100)
                .err()
                .unwrap();
            assert_eq!(failed.cause.code, ErrorCode::LimitExceeded);
            assert!(failed.cleanup.is_complete());
            assert!(failed.recovery.is_none());
            let mut task =
                HttpTask::start(&api, http::Request::get("https://example.test"), 100).unwrap();
            assert!(
                task.on_event(&Notification::Timer {
                    handle: handle(99),
                    sequence: 1
                })
                .events
                .is_empty()
            );
            let batch = task.on_event(&Notification::Timer {
                handle: handle(3),
                sequence: 1,
            });
            assert!(matches!(
                batch.events.as_slice(),
                [
                    http::Event::Headers { status: 404, .. },
                    http::Event::Chunk { .. },
                    http::Event::Finished { bytes: 2 }
                ]
            ));
            assert!(batch.cleanup.is_complete());
            assert!(task.close().is_complete());
            assert_eq!(*closed.borrow(), [1, 3, 2]);
        },
    );
}

/// Cleanup failure during startup returns a recovery owner instead of dropping its only stream handle.
#[test]
fn lifecycle_http_start_failure_preserves_retryable_cleanup_owner() {
    use crate::http::{self, HttpTask};
    let mut denied = true;
    with_host(
        move |operation| match operation {
            Operation::Http {
                operation: http::Operation::Start { .. },
            } => Ok(Value::Resource(handle(1))),
            Operation::Timer { .. } => Err(Failure::new(ErrorCode::LimitExceeded, "no timer")),
            Operation::CloseResource { .. } if denied => {
                denied = false;
                Err(Failure::new(ErrorCode::PermissionDenied, "close refused"))
            }
            Operation::CloseResource { .. } => Ok(Value::Unit),
            _ => panic!("unexpected SDK operation"),
        },
        || {
            let api = Negotiated {
                base: semver::Version::new(1, 0, 0),
                capabilities: [
                    ("network.http".into(), semver::Version::new(1, 0, 0)),
                    ("lifecycle.timers".into(), semver::Version::new(1, 0, 0)),
                ]
                .into(),
            };
            let failed = HttpTask::start(&api, http::Request::get("https://example.test"), 100)
                .err()
                .unwrap();
            assert_eq!(failed.cause.code, ErrorCode::LimitExceeded);
            assert_eq!(failed.cleanup.pending[0].code, ErrorCode::PermissionDenied);
            let mut recovery = failed
                .recovery
                .expect("unreleased stream must have an owner");
            assert!(recovery.close().is_complete());
            assert!(recovery.close().is_complete());
        },
    );
}

/// Busy streams yield back to the host after eight events instead of draining indefinitely.
#[test]
fn lifecycle_http_yields_between_bounded_batches_and_never_calls_host_on_drop() {
    use crate::http::{self, HttpTask};
    let requests = Rc::new(RefCell::new(0));
    let observed = requests.clone();
    with_host(
        move |operation| {
            *observed.borrow_mut() += 1;
            match operation {
                Operation::Http {
                    operation: http::Operation::Start { .. },
                } => Ok(Value::Resource(handle(1))),
                Operation::Timer { .. } => Ok(Value::Resource(handle(2))),
                Operation::Http {
                    operation: http::Operation::Read { .. },
                } => Ok(Value::Http(Some(http::Event::Chunk { bytes: vec![0xe4] }))),
                Operation::CloseResource { .. } => Ok(Value::Unit),
                _ => panic!("unexpected SDK operation"),
            }
        },
        || {
            let api = Negotiated {
                base: semver::Version::new(1, 0, 0),
                capabilities: [
                    ("network.http".into(), semver::Version::new(1, 0, 0)),
                    ("lifecycle.timers".into(), semver::Version::new(1, 0, 0)),
                ]
                .into(),
            };
            let mut task =
                HttpTask::start(&api, http::Request::get("https://example.test"), 100).unwrap();
            let notification = Notification::Timer {
                handle: handle(2),
                sequence: 1,
            };
            assert_eq!(task.on_event(&notification).events.len(), 8);
            let before_drop = *requests.borrow();
            drop(task);
            assert_eq!(
                *requests.borrow(),
                before_drop,
                "Drop cannot perform imported calls or block"
            );
        },
    );
}
