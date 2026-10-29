//! The ordinary editor admission boundary rejects newer message semantics before allocating a request.

use super::*;
use plugin_protocol::interaction::{Operation, Severity};

/// Publishing is available only after negotiating 1.1; Notify keeps its existing 1.0 contract.
#[test]
fn publish_message_requires_interaction_1_1_without_changing_notify() {
    let operation = Operation::PublishMessage {
        title: "Context".into(),
        message: "Message".into(),
        severity: Severity::Error,
    };
    let mut api = api::Negotiated {
        base: semver::Version::new(1, 0, 0),
        capabilities: Default::default(),
    };
    assert_eq!(
        validate_interaction_request(&operation, &api)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnavailable
    );
    api.capabilities
        .insert("ui.interaction".into(), semver::Version::new(1, 0, 0));
    assert_eq!(
        validate_interaction_request(&operation, &api)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnavailable
    );
    validate_interaction_request(
        &Operation::Notify {
            title: "Context".into(),
            message: "Message".into(),
            severity: Severity::Error,
        },
        &api,
    )
    .unwrap();
    api.capabilities
        .insert("ui.interaction".into(), semver::Version::new(1, 1, 0));
    validate_interaction_request(&operation, &api).unwrap();
}
