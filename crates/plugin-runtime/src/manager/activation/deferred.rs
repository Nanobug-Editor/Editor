//! Renewed original purposes wait for abandoned workers without reviving their publication authority.
use super::*;

impl State {
    /// Count pending reservations across running and deferred work against one global native limit.
    pub(super) fn waiter_count(&self) -> usize {
        self.flights
            .values()
            .map(|flight| flight.waiters.lock().unwrap().len())
            .sum::<usize>()
            + self.deferred.values().map(Vec::len).sum::<usize>()
    }

    /// Lifecycle cancellation reaches queued purposes as well as a detached worker's original waits.
    pub(super) fn for_each_waiter(&self, mut operation: impl FnMut(&Availability, &Waiter)) {
        for (token, flight) in &self.flights {
            for waiter in flight.waiters.lock().unwrap().iter() {
                operation(token, waiter);
            }
        }
        for (token, waiters) in &self.deferred {
            for waiter in waiters {
                operation(token, waiter);
            }
        }
    }

    /// Deferred waits keep their own completion gates, source seals and absolute deadlines.
    pub(super) fn prune_deferred(&mut self) {
        self.deferred.retain(|_, waiters| {
            waiters.retain(|waiter| !waiter.completion.status().is_terminal());
            !waiters.is_empty()
        });
    }
}

impl Manager {
    /// Start fresh preparation only after the previous worker/result has relinquished its slot.
    /// Admission rechecks the original token; neither queuing nor promotion extends its deadline.
    pub(super) fn resume_deferred_demands(&mut self) {
        let ready = self
            .activation
            .deferred
            .keys()
            .filter(|token| !self.activation.flights.contains_key(*token))
            .cloned()
            .collect::<Vec<_>>();
        for token in ready {
            let waiters = self.activation.deferred.remove(&token).unwrap();
            let current = self.availability_in_scope(token.plugin(), token.scope());
            for waiter in waiters {
                if waiter.completion.status().is_terminal() {
                    continue;
                }
                if current.as_ref().ok() != Some(&token) {
                    waiter.completion.finish(Err(api::Failure::new(
                        api::ErrorCode::InvalidHandle,
                        "Deferred activation intent was retired",
                    )));
                    continue;
                }
                let completion = waiter.completion.clone();
                if let Err(error) = self.join_demand_waiter(&token, waiter) {
                    completion.finish(Err(error));
                }
            }
        }
    }
}
