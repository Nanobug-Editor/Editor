//! Scheduling decisions retain ordinary waiters, shared flights and the bounded worker budget.
use super::*;

/// Keep the scheduling boundary independent of WASM compilation; public package tests separately
/// exercise real preparation and final admission while these owned jobs remain outstanding.
fn flight(state: &mut State, id: &str, reason: DemandReason, running: bool) -> Availability {
    let token = Availability {
        manager: state.identity,
        plugin: id.into(),
        scope: "workspace".into(),
        generation: 1,
        configuration: 1,
    };
    state.flights.insert(
        token.clone(),
        Flight {
            waiters: Arc::new(Mutex::new(vec![Waiter {
                completion: crate::Completion::new(120_000),
                reason,
                language_workspace: None,
            }])),
            preparation: None,
            running,
            lease_count: Arc::new(std::sync::atomic::AtomicUsize::new(1)),
        },
    );
    token
}

fn language() -> DemandReason {
    DemandReason::Language {
        provider: "provider/analysis".into(),
        role: LanguageRole::Lsp,
    }
}

/// Startup may restore many language documents before a dock becomes visible. The dock can
/// use one remaining slot, and taking it must still enforce the original four-job total budget.
#[test]
fn visible_demand_can_start_beside_three_slow_language_jobs() {
    let mut state = State::default();
    for id in ["language-a", "language-b", "language-c"] {
        flight(&mut state, id, language(), true);
    }
    flight(&mut state, "language-d", language(), false);
    assert!(state.next_preparation().is_none());
    let visible = flight(
        &mut state,
        "visible-z",
        DemandReason::View("panel".into()),
        false,
    );
    assert_eq!(state.next_preparation(), Some(visible.clone()));
    state.flights.get_mut(&visible).unwrap().running = true;
    assert!(state.next_preparation().is_none());
}

/// A nested service can make progress through the reserved slot, but queued explicit native
/// input takes precedence. Service preparation must never bypass the overall worker budget.
#[test]
fn reserved_capacity_allows_service_dependencies_after_interaction() {
    let mut state = State::default();
    for id in ["a", "b", "c"] {
        flight(&mut state, id, DemandReason::Background, true);
    }
    let service = flight(
        &mut state,
        "service",
        DemandReason::Service("analysis".into()),
        false,
    );
    assert_eq!(state.next_preparation(), Some(service.clone()));
    let visible = flight(
        &mut state,
        "view",
        DemandReason::Command("open".into()),
        false,
    );
    assert_eq!(state.next_preparation(), Some(visible.clone()));
    state.flights.remove(&visible);
    state.flights.get_mut(&service).unwrap().running = true;
    assert!(state.next_preparation().is_none());
}

/// A visible consumer joins an existing background flight instead of starting a duplicate.
/// Cancelling just that consumer restores the surviving purposes' priority and deadline.
#[test]
fn coalesced_interactive_demand_promotes_only_while_its_waiter_is_live() {
    let mut state = State::default();
    let background = flight(&mut state, "background-a", DemandReason::Background, false);
    let selected = flight(&mut state, "language-z", language(), false);
    assert_eq!(state.next_preparation(), Some(selected.clone()));
    let visible = crate::Completion::new(120_000);
    state.flights[&background]
        .waiters
        .lock()
        .unwrap()
        .push(Waiter {
            completion: visible.clone(),
            reason: DemandReason::View("panel".into()),
            language_workspace: None,
        });
    assert_eq!(state.next_preparation(), Some(background.clone()));
    visible
        .cancel(api::CancelMode::TryTerminate, api::ErrorCode::Cancelled)
        .unwrap();
    assert_eq!(state.next_preparation(), Some(selected));
    assert!(
        !state.flights[&background].waiters.lock().unwrap()[0]
            .completion
            .status()
            .is_terminal()
    );
}
