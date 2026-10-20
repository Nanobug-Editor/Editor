//! Native demand transport retains original intent while the Manager owns shared preparation.
//! No second preparation pool, guest instance or document state lives in the application actor.

use super::*;
use crate::extensions::command_intents::CommandGuard;
use futures::channel::oneshot;
use plugin_runtime::{
    ActivationReceipt, Availability, DemandReason, DemandRequest, RequestCancellation,
};
use rust_i18n::t;
use std::sync::atomic::{AtomicBool, Ordering};

/// A command's original static identity is separate from the eventual live callback incarnation.
#[derive(Clone)]
pub(in crate::extensions) struct CommandOrigin {
    pub availability: Availability,
    pub guard: Option<CommandGuard>,
}

/// The native consumer owns this wait. Hiding, replacing or disposing it cancels only this waiter.
pub(crate) struct DemandWait {
    receiver: oneshot::Receiver<Result<ActivationReceipt, api::Failure>>,
    cancellation: RequestCancellation,
    /// Original reservation evidence survives a late result after its cached error was withdrawn.
    published_failure: Arc<AtomicBool>,
}

impl DemandWait {
    /// True only when the exact original reservation published an Err and returned Completed Err.
    /// Admission, receipt validation, cancellation and timeout cannot authorize an automatic retry.
    pub(crate) fn preparation_error_was_published(&self) -> bool {
        self.published_failure.load(Ordering::Acquire)
    }

    /// Use the same token for the surrounding native request's Escape and document cancellation.
    pub(crate) fn cancellation(&self) -> RequestCancellation {
        self.cancellation.clone()
    }

    /// Wait asynchronously without blocking GPUI or creating a second worker pool.
    pub(crate) async fn wait(&mut self) -> Result<ActivationReceipt, api::Failure> {
        (&mut self.receiver)
            .await
            .unwrap_or_else(|_| Err(unavailable()))
    }

    /// Panel/document reconciliation polls this bounded native receipt without running guest code.
    pub(crate) fn try_take(&mut self) -> Option<Result<ActivationReceipt, api::Failure>> {
        match self.receiver.try_recv() {
            Ok(value) => value,
            Err(_) => Some(Err(unavailable())),
        }
    }
}

impl Drop for DemandWait {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

/// A stopped actor is a terminal result, never a reason to sample a new target and retry.
fn unavailable() -> api::Failure {
    api::Failure::new(
        api::ErrorCode::InvalidState,
        t!("plugins.worker_unavailable").to_string(),
    )
}

/// Native failures reach the status bar; retain the typed reason while translating its explanation.
fn cancelled(reason: api::ErrorCode) -> api::Failure {
    let message = if reason == api::ErrorCode::TimedOut {
        t!("plugins.demand_timeout")
    } else {
        t!("plugins.demand_cancelled")
    };
    api::Failure::new(reason, message.to_string())
}

impl Worker {
    /// Queue the already captured availability and native guard for one actual selected entry.
    /// Trust and static publication are checked before enqueue; Manager rechecks at final admission.
    pub(crate) fn demand(
        &self,
        availability: Availability,
        reason: DemandReason,
        guard: Option<CommandGuard>,
    ) -> Result<DemandWait, api::Failure> {
        if !self.trusted.load(std::sync::atomic::Ordering::Acquire) {
            return Err(api::Failure::new(
                api::ErrorCode::PermissionDenied,
                t!("plugins.restricted").to_string(),
            ));
        }
        let (reply, receiver) = oneshot::channel();
        let cancellation = RequestCancellation::default();
        let published_failure = Arc::new(AtomicBool::new(false));
        self.tx
            .send(Work::Demand {
                plugin: availability.plugin().to_owned(),
                availability,
                reason,
                guard,
                cancellation: cancellation.clone(),
                published_failure: published_failure.clone(),
                reply,
            })
            .map_err(|_| unavailable())?;
        Ok(DemandWait {
            receiver,
            cancellation,
            published_failure,
        })
    }
}

/// Commands and receipt consumers share one bounded actor queue; preparation belongs to Manager.
#[derive(Default)]
pub(in crate::extensions) struct Demands {
    pending: Vec<Pending>,
}

struct Pending {
    availability: Availability,
    request: DemandRequest,
    operation: Operation,
}

enum Operation {
    Reply {
        guard: Option<CommandGuard>,
        cancellation: RequestCancellation,
        published_failure: Arc<AtomicBool>,
        reply: oneshot::Sender<Result<ActivationReceipt, api::Failure>>,
    },
    Command {
        plugin: String,
        command: String,
        arguments: serde_json::Value,
        context: Option<commands::Context>,
        expected_epoch: u64,
        guard: Option<CommandGuard>,
    },
}

impl Operation {
    /// Native revocation is monotonic, including a selection or document ABA during preparation.
    fn admission(&self) -> Result<(), api::Failure> {
        let guard = match self {
            Self::Reply {
                guard,
                cancellation,
                reply,
                ..
            } => {
                if cancellation.is_cancelled() || reply.is_canceled() {
                    return Err(cancelled(api::ErrorCode::Cancelled));
                }
                guard
            }
            Self::Command { guard, .. } => guard,
        };
        guard.as_ref().map_or(Ok(()), CommandGuard::admit)
    }

    /// Return the original caller's result; commands use the normal owned operation publication.
    fn finish(
        self,
        result: Result<ActivationReceipt, api::Failure>,
        manager: &mut Manager,
        output: &Arc<Mutex<Published>>,
        trusted: bool,
    ) {
        match self {
            Self::Reply { reply, .. } => {
                let _ = reply.send(result);
            }
            Self::Command {
                plugin,
                command,
                arguments,
                context,
                expected_epoch,
                guard,
            } => {
                let result = result.map_err(anyhow::Error::from).and_then(|_| {
                    // Release the publication mutex before entering guest code. The original
                    // context/arguments and native guard travel across demand without resampling.
                    output.lock().unwrap().admit_command(
                        manager,
                        trusted,
                        &plugin,
                        &command,
                        expected_epoch,
                    )?;
                    if let Some(guard) = guard {
                        guard.admit()?;
                    }
                    manager.invoke_command_with_context(&plugin, &command, arguments, context)
                });
                if let Err(error) = result {
                    let mut state = output.lock().unwrap();
                    state.logs.append(
                        &plugin,
                        plugin_runtime::LogLevel::Error,
                        "host.operation",
                        format!("{error:#}"),
                    );
                    state.status = Some(OperationStatus {
                        plugin: Some(plugin),
                        message: format!("{error:#}"),
                    });
                }
            }
        }
    }
}

impl Demands {
    /// Actor cleanup and public-package harnesses may wait for this finite set of original callers.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Capture work without waiting. Static commands remain visible while this request prepares.
    pub fn dispatch(
        &mut self,
        work: Work,
        manager: &mut Manager,
        output: &Arc<Mutex<Published>>,
        trusted: bool,
    ) -> Option<Work> {
        let (availability, reason, operation) = match work {
            Work::Demand {
                availability,
                reason,
                guard,
                cancellation,
                published_failure,
                reply,
                ..
            } => (
                availability,
                reason,
                Operation::Reply {
                    guard,
                    cancellation,
                    published_failure,
                    reply,
                },
            ),
            Work::Invoke {
                plugin,
                command,
                arguments,
                context,
                expected_epoch,
                origin,
            } => {
                let Some(origin) = origin else {
                    let mut state = output.lock().unwrap();
                    let error = api::Failure::new(
                        api::ErrorCode::StaleRevision,
                        t!("plugins.command_owner_changed").to_string(),
                    );
                    state.logs.append(
                        &plugin,
                        plugin_runtime::LogLevel::Error,
                        "host.operation",
                        error.to_string(),
                    );
                    state.status = Some(OperationStatus {
                        plugin: Some(plugin),
                        message: error.to_string(),
                    });
                    return None;
                };
                (
                    origin.availability,
                    DemandReason::Command(command.clone()),
                    Operation::Command {
                        plugin,
                        command,
                        arguments,
                        context,
                        expected_epoch,
                        guard: origin.guard,
                    },
                )
            }
            other => return Some(other),
        };
        let admission = operation.admission().and_then(|()| {
            if !trusted {
                return Err(api::Failure::new(
                    api::ErrorCode::PermissionDenied,
                    t!("plugins.restricted").to_string(),
                ));
            }
            if self.pending.len() >= 64 {
                return Err(api::Failure::new(
                    api::ErrorCode::LimitExceeded,
                    t!("plugins.demand_queue_full").to_string(),
                ));
            }
            manager.begin_demand(&availability, reason, 60_000)
        });
        match admission {
            Ok(request) => {
                self.pending.push(Pending {
                    availability,
                    request,
                    operation,
                });
            }
            Err(error) => operation.finish(Err(error), manager, output, trusted),
        }
        None
    }

    /// Commit receipt admission after Manager.poll, preserving cancellation and exact first birth.
    pub fn poll(&mut self, manager: &mut Manager, output: &Arc<Mutex<Published>>, trusted: bool) {
        let mut waiting = Vec::new();
        for pending in self.pending.drain(..) {
            let result = match pending.operation.admission() {
                Err(error) => {
                    let _ = pending.request.cancel();
                    Some(Err(error))
                }
                Ok(()) if !trusted => {
                    let _ = pending.request.cancel();
                    Some(Err(api::Failure::new(
                        api::ErrorCode::PermissionDenied,
                        t!("plugins.restricted").to_string(),
                    )))
                }
                Ok(()) => match pending.request.status() {
                    api::RequestUpdate::Completed { result } => {
                        if result.is_err()
                            && pending.request.has_published_language_error()
                            && let Operation::Reply {
                                published_failure, ..
                            } = &pending.operation
                        {
                            // Read the original reservation, not the current cache: its Err may
                            // already be withdrawn. A new same-key reservation cannot set this bit.
                            published_failure.store(true, Ordering::Release);
                        }
                        Some(result.and_then(|receipt| {
                            manager.admit_receipt(&pending.availability, &receipt)?;
                            Ok(receipt)
                        }))
                    }
                    api::RequestUpdate::Cancelled { reason, .. } => Some(Err(cancelled(reason))),
                    _ => None,
                },
            };
            if let Some(result) = result {
                pending.operation.finish(result, manager, output, trusted);
            } else {
                waiting.push(pending);
            }
        }
        self.pending = waiting;
    }
}
