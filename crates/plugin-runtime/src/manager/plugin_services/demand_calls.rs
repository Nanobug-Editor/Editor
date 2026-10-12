//! Service activation waits retain their original reference, values, scope and absolute deadline.
use super::*;
use crate::plugin_services::Waiting;

impl Manager {
    /// A ready exact owner runs now; a cold publication joins the Manager's shared bounded demand queue.
    /// This function never calls a guest or starts preparation while holding the broker mutex.
    pub(super) fn admit_service_demand(
        &mut self,
        call: Call,
    ) -> Result<Option<Call>, (Call, api::Failure)> {
        if call.reference.provider.availability.is_none() || call.reference.provider.running() {
            return Ok(Some(call));
        }
        // First enqueue captured this call, not its possibly older open reference. Never borrow
        // a fresh configuration here after the original request was accepted by the guest/host.
        let Some(token) = call.availability.get().and_then(Clone::clone) else {
            return Err((
                call,
                api::Failure::new(
                    api::ErrorCode::InvalidHandle,
                    "Service call has no original activation intent",
                ),
            ));
        };
        // Activation has its own 120 s upper bound; a longer service deadline never resets either clock.
        let timeout = call.completion.remaining_timeout_ms().min(120_000);
        if timeout == 0 || call.completion.status().is_terminal() {
            return Ok(None);
        }
        let demand = match self.begin_service_demand(
            &token,
            &call.reference.contract,
            &call.completion,
            &call.context.lifetimes,
            &call.context.native_waits,
        ) {
            Ok(demand) => demand,
            Err(failure) => return Err((call, failure)),
        };
        let waiting = Waiting {
            call,
            token,
            demand,
        };
        match waiting.demand.status() {
            api::RequestUpdate::Completed { result } => {
                self.complete_service_demand(waiting, result).map(Some)
            }
            api::RequestUpdate::Cancelled { reason, .. } => Err((
                waiting.call,
                api::Failure::new(reason, "Service activation wait ended"),
            )),
            _ => {
                let completion = waiting.call.completion.clone();
                if let Err(failure) = self.plugin_services.lock().unwrap().wait(waiting) {
                    completion.finish(Err(failure));
                }
                Ok(None)
            }
        }
    }

    /// Observe bounded waits fairly and requeue admitted calls; at most 32 callbacks execute per tick.
    pub(super) fn poll_service_demands(&mut self) {
        let waits = self.plugin_services.lock().unwrap().take_waiting();
        for waiting in waits {
            if waiting.call.completion.status().is_terminal() {
                continue;
            }
            let valid = self
                .availability_in_scope(waiting.token.plugin(), waiting.token.scope())
                .is_ok_and(|token| token == waiting.token);
            if !valid {
                waiting.call.completion.finish(Err(api::Failure::new(
                    api::ErrorCode::InvalidHandle,
                    "Original service activation intent changed",
                )));
                continue;
            }
            match waiting.demand.status() {
                api::RequestUpdate::Completed { result } => match self
                    .complete_service_demand(waiting, result)
                {
                    Ok(call) => {
                        let completion = call.completion.clone();
                        if let Err(failure) = self.plugin_services.lock().unwrap().enqueue(call) {
                            completion.finish(Err(failure));
                        }
                    }
                    Err((call, failure)) => call.completion.finish(Err(failure)),
                },
                api::RequestUpdate::Cancelled { reason, .. } => waiting.call.completion.finish(
                    Err(api::Failure::new(reason, "Service activation wait ended")),
                ),
                _ => {
                    let completion = waiting.call.completion.clone();
                    if let Err(failure) = self.plugin_services.lock().unwrap().wait(waiting) {
                        completion.finish(Err(failure));
                    }
                }
            }
        }
    }

    /// Final admission consumes only the captured receipt and binds the same publication once.
    /// Birth replaces the descriptive ancestry hop; it neither adds a hop nor grants extra authority.
    fn complete_service_demand(
        &mut self,
        waiting: Waiting,
        result: Result<ActivationReceipt, api::Failure>,
    ) -> Result<Call, (Call, api::Failure)> {
        let Waiting {
            mut call,
            token,
            demand: _demand,
        } = waiting;
        let admitted = result.and_then(|receipt| {
            self.admit_receipt(&token, &receipt)?;
            let owner = receipt.owner().ok_or_else(|| {
                api::Failure::new(
                    api::ErrorCode::CapabilityUnavailable,
                    "Service has no executable component",
                )
            })?;
            self.refresh_services();
            let broker = self.plugin_services.lock().unwrap();
            broker.validate_reference(&call.context.caller, &call.reference)?;
            if call.completion.status().is_terminal() || !broker.context_alive(&call.context) {
                return Err(api::Failure::new(
                    api::ErrorCode::InvalidHandle,
                    "Original service request ended",
                ));
            }
            broker.bind_reference(&call.reference, owner)
        });
        let reference = match admitted {
            Ok(reference) => reference,
            Err(failure) => return Err((call, failure)),
        };
        if let Some(last) = call.context.ancestry.last_mut()
            && *last == call.reference.provider.caller.instance
        {
            *last = reference.provider.caller.instance.clone();
        }
        if let Some(alive) = &reference.provider.owner_alive {
            call.context.lifetimes.push(alive.clone());
        }
        call.reference = reference;
        Ok(call)
    }
}
