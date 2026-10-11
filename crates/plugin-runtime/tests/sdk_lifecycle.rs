//! Actual old/new SDK packages exercise configuration and recovery through public demand receipts.
#[path = "support/lifecycle_http.rs"]
mod endpoint;
#[path = "support/cloud_packages.rs"]
mod legacy;
#[path = "support/lifecycle_packages.rs"]
mod packages;
#[path = "support/lifecycle_scenarios.rs"]
mod scenarios;

use plugin_runtime::{
    DemandReason, Manager, Package,
    plugin_protocol::{
        api, http, secrets,
        settings::{Scope, Source},
    },
};
use serde_json::{Value as Json, json};
use std::time::{Duration, Instant};

/// Warm commands retain their owner; cold commands must first receive and admit their original demand token.
fn demand(manager: &mut Manager, id: &str, command: &str) {
    if manager.instance_id(id).is_some() {
        return;
    }
    let token = manager.availability(id).unwrap();
    // A real cold WASM compile uses the public activation ceiling; HTTP deadlines below stay unchanged.
    let request = manager
        .begin_demand(&token, DemandReason::Command(command.into()), 120000)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(130);
    loop {
        manager.poll();
        match request.status() {
            api::RequestUpdate::Completed {
                result: Ok(receipt),
            } => {
                manager.admit_receipt(&token, &receipt).unwrap();
                return;
            }
            api::RequestUpdate::Completed { result: Err(error) } => {
                panic!("Demand failed: {:?}", error.code)
            }
            api::RequestUpdate::Cancelled { reason, .. } => panic!("Demand cancelled: {reason:?}"),
            _ => {}
        }
        assert!(
            Instant::now() < deadline,
            "Public activation receipt did not terminate"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Drive the production actor and timer clock, reporting only public output and resource counters on failure.
/// `stage` identifies the scenario step; `server` supplies a count without exposing captured credentials.
#[track_caller]
fn pump_until(
    manager: &mut Manager,
    id: &str,
    stage: &str,
    server: &endpoint::Server,
    condition: impl Fn(&Manager) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition(manager) {
        assert!(
            Instant::now() < deadline,
            "SDK lifecycle wait exceeded 10 s: plugin={id}, stage={stage}, text={:?}, instance={:?}, resources={}, server_requests={}",
            legacy::text(manager, id),
            manager.instance_id(id),
            manager.resource_count(),
            server.count(),
        );
        manager.poll();
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A bounded observation interval detects an old timer firing before its newly configured deadline.
fn pump_for(manager: &mut Manager, duration: Duration) {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        manager.poll();
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Read opaque managed data through the old guest's public command after admission/recovery.
fn private_text(manager: &mut Manager) -> String {
    demand(manager, "capability-example", "scope-read");
    manager
        .invoke_command("capability-example", "scope-read", Json::Null)
        .unwrap();
    let plugin_runtime::plugin_protocol::ui::Kind::Text { text } =
        &manager.live["capability-example"]
            .views
            .values()
            .next()
            .unwrap()
            .root
            .kind
    else {
        panic!("Expected SDK-published private-data result");
    };
    text.clone()
}
