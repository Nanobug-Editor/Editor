//! Bounded preparation keeps one slot available to native interaction during startup bursts.
use super::*;

const MAX_PREPARATIONS: usize = 4;

#[cfg(test)]
mod tests;

/// A shared first-birth job inherits its most urgent current consumer without changing any
/// waiter's original deadline, resource owner or cancellation. Names never establish urgency.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Priority {
    Interaction,
    Service,
    Language,
    Background,
}

impl Flight {
    fn priority(&self) -> Priority {
        self.waiters
            .lock()
            .unwrap()
            .iter()
            .filter(|waiter| !waiter.completion.status().is_terminal())
            .map(|waiter| match waiter.reason {
                DemandReason::View(_) | DemandReason::Command(_) => Priority::Interaction,
                DemandReason::Service(_) => Priority::Service,
                DemandReason::Language { .. } | DemandReason::LanguageDocument { .. } => {
                    Priority::Language
                }
                DemandReason::Background => Priority::Background,
            })
            .min()
            .unwrap_or(Priority::Background)
    }
}

impl State {
    /// Three independent startup/language jobs may run concurrently. A shown view or command
    /// can use the remaining slot; a dependent service may also borrow it so a caller is not
    /// stalled behind a capacity reservation for work that can require that very service.
    /// Running jobs keep their lease and are never cancelled or duplicated by prioritization.
    pub(super) fn next_preparation(&self) -> Option<Availability> {
        let running = self
            .flights
            .values()
            .filter(|flight| flight.running)
            .count();
        if running >= MAX_PREPARATIONS {
            return None;
        }
        self.flights
            .iter()
            .filter(|(_, flight)| !flight.running)
            .map(|(token, flight)| (token, flight.priority()))
            .filter(|(_, priority)| {
                running < MAX_PREPARATIONS - 1 || *priority <= Priority::Service
            })
            .min_by_key(|(_, priority)| *priority)
            .map(|(token, _)| token.clone())
    }
}
