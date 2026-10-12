//! Asynchronous native dependency acquisition precedes a declared plugin method, without blocking WASM.
use super::*;
use crate::{InstallControl, plugin_services::Call};
use std::sync::mpsc;

/// One accepted invocation retains its identity, cancellation gate and immutable dependency leases.
pub(super) struct Pending {
    call: Call,
    control: InstallControl,
    ready: mpsc::Receiver<anyhow::Result<Vec<crate::dependencies::Prepared>>>,
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.control.cancel();
    }
}
impl Manager {
    /// Consume only method metadata from the admitted provider package; callers cannot supply plans.
    pub(super) fn prepare_service_call(
        &mut self,
        call: Call,
    ) -> Result<Option<Call>, api::Failure> {
        let provider = self
            .live
            .values()
            .chain(self.parked.values().flat_map(|scope| scope.live.values()))
            .find(|instance| {
                instance.service_provider().is_some_and(|provider| {
                    provider.caller.instance == call.reference.provider.caller.instance
                })
            });
        let Some(preparation) = provider
            .map(|instance| instance.native_preparation(&call))
            .transpose()?
            .flatten()
        else {
            return Ok(Some(call));
        };
        let (assets, declared) = preparation;
        let plans = declared;
        let mut cached = Vec::new();
        for plan in &plans {
            let prepared = crate::dependencies::cached(&self.root, plan).ok();
            if prepared.is_none() {
                // Installer execution needs a separate concrete prompt; lazy acquisition cannot bypass it.
                if plan
                    .artifacts
                    .iter()
                    .any(|artifact| artifact.installer.is_some())
                {
                    return Err(api::Failure::new(
                        api::ErrorCode::PermissionDenied,
                        "Prepare native installer dependencies through plugin installation",
                    ));
                }
            }
            // Keep cache-hit leases until dispatch, and include every plan in this version's receipt.
            // Dropping hits here would let removing the previous owner's pin break offline reuse.
            cached.push(prepared);
        }
        if plans.is_empty() {
            return Ok(Some(call));
        }
        if self.service_preparations.len() >= 32 {
            return Err(api::Failure::new(
                api::ErrorCode::LimitExceeded,
                "Native preparation quota exceeded",
            ));
        }
        let root = self.root.clone();
        // Separate immutable plan sets retain pins from other methods of the same package version.
        let plan_digest = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&plans).map_err(|error| api::Failure::new(
                    api::ErrorCode::InvalidRequest,
                    error.to_string()
                ))?
            )
        );
        let receipt = root
            .join("dependency-receipts")
            .join(&call.reference.provider.caller.plugin)
            .join(format!(
                "{}-on-demand-{plan_digest}.json",
                assets.file_name().unwrap().to_string_lossy()
            ));
        let completion = call.completion.clone();
        let lifetime = call.completion.clone();
        let source = call.context.clone();
        let control = InstallControl::new(move |stage| {
            completion.progress(format!("Preparing native service: {stage:?}"))
        })
        .with_guard(move || {
            // Download/receipt publication observes the same original ancestry as activation, even
            // before the actor polls. Completed parents remain valid; cancellation/deadlines do not.
            !lifetime.status().is_terminal()
                && source
                    .lifetimes
                    .iter()
                    .all(|alive| alive.load(std::sync::atomic::Ordering::Acquire))
                && source
                    .native_waits
                    .iter()
                    .all(|wait| !matches!(wait.status(), api::RequestUpdate::Cancelled { .. }))
        });
        let worker_control = control.clone();
        let (send, ready) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("plugin-service-preparation".into())
            .spawn(move || {
                let result = (|| {
                    let _lock = crate::dependencies::lock(&root, &worker_control)?;
                    let mut prepared = Vec::new();
                    for (plan, hit) in plans.iter().zip(cached) {
                        prepared.push(match hit {
                            Some(hit) => hit,
                            None => {
                                crate::dependencies::prepare(&root, &assets, plan, &worker_control)?
                            }
                        });
                    }
                    // Retain immutable bytes after the process exits, under the package's cache receipt.
                    worker_control.check()?;
                    let retained: BTreeMap<_, _> = plans
                        .into_iter()
                        .enumerate()
                        .map(|(index, plan)| (format!("service:{index}"), plan))
                        .collect();
                    atomic_write(&receipt, &serde_json::to_vec(&retained)?)?;
                    Ok(prepared)
                })();
                let _ = send.send(result);
            })
            .map_err(|error| {
                api::Failure::new(api::ErrorCode::OperationFailed, error.to_string())
            })?;
        self.service_preparations.push(Pending {
            call,
            control,
            ready,
        });
        Ok(None)
    }

    /// Retired/cancelled invocations cancel I/O; completion never revives a stale provider incarnation.
    pub(super) fn poll_service_preparations(
        &mut self,
    ) -> Vec<(Call, Vec<crate::dependencies::Prepared>)> {
        let mut ready_calls = Vec::new();
        let pending = std::mem::take(&mut self.service_preparations);
        for item in pending {
            if item.call.completion.status().is_terminal() {
                continue;
            }
            let valid = self
                .plugin_services
                .lock()
                .unwrap()
                .validate_reference(&item.call.context.caller, &item.call.reference);
            if let Err(error) = valid {
                item.call.completion.finish(Err(error));
                continue;
            }
            match item.ready.try_recv() {
                Ok(Ok(leases)) => ready_calls.push((item.call.clone(), leases)),
                Ok(Err(error)) => item.call.completion.finish(Err(api::Failure::new(
                    api::ErrorCode::OperationFailed,
                    format!("Native service preparation failed: {error:#}"),
                ))),
                Err(mpsc::TryRecvError::Empty) => self.service_preparations.push(item),
                Err(mpsc::TryRecvError::Disconnected) => {
                    item.call.completion.finish(Err(api::Failure::new(
                        api::ErrorCode::OperationFailed,
                        "Native preparation worker stopped",
                    )))
                }
            }
        }
        ready_calls
    }
}
