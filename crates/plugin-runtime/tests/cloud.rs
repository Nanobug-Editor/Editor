//! T25–T30 are observed through independently SDK-built packages and the production Manager.
#[path = "cloud/display.rs"]
mod display;
#[path = "support/cloud_http.rs"]
mod endpoint;
#[path = "support/cloud_packages.rs"]
mod packages;
#[path = "cloud/private_depth.rs"]
mod private_depth;
#[path = "cloud/secret_publication.rs"]
mod secret_publication;
#[path = "cloud/security.rs"]
mod security;
#[path = "cloud/temporary_ownership.rs"]
mod temporary_ownership;
#[path = "support/cloud_tls.rs"]
mod tls;
#[path = "cloud/updates.rs"]
mod updates;
use packages::*;
use plugin_runtime::{
    HostResources, HttpConfiguration, Manager,
    plugin_protocol::{
        api::{self, ErrorCode, Operation as Op, Value},
        http, secrets, storage, timers,
    },
};
use serde_json::{Value as Json, json};
use std::time::{Duration, Instant};

fn http_op(operation: http::Operation) -> Op {
    Op::Http { operation }
}
fn secret_op(operation: secrets::Operation) -> Op {
    Op::Secrets { operation }
}
fn private_op(operation: storage::Operation) -> Op {
    Op::Private { operation }
}
fn timer_op(operation: timers::Operation) -> Op {
    Op::Timer { operation }
}
fn read(manager: &mut Manager, id: &str, handle: &api::ResourceHandle) -> Option<http::Event> {
    let Value::Http(event) = probe(
        manager,
        id,
        http_op(http::Operation::Read {
            handle: handle.clone(),
        }),
    )
    .unwrap() else {
        panic!("Expected stream event")
    };
    event
}
fn finish(manager: &mut Manager, id: &str, handle: &api::ResourceHandle) -> Vec<http::Event> {
    let until = Instant::now() + Duration::from_secs(5);
    let mut events = vec![];
    loop {
        assert!(
            Instant::now() < until,
            "Stream failed to terminate: {events:?}"
        );
        if let Some(event) = read(manager, id, handle) {
            let terminal = event.is_terminal();
            events.push(event);
            if terminal {
                break;
            }
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    events
}
#[path = "cloud/ownership.rs"]
mod ownership;
#[path = "cloud/private.rs"]
mod private;
#[path = "cloud/queries.rs"]
mod queries;
