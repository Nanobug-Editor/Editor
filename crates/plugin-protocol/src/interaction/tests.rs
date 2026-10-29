//! Message publication has its own wire operation and keeps native notice bounds and completion distinct.

use super::*;

/// A published receipt is never confused with user dismissal, and malformed text is rejected before admission.
#[test]
fn publish_message_wire_bounds_and_completion_are_explicit() {
    let mut message = Operation::PublishMessage {
        title: "Git".into(),
        message: "中文错误".into(),
        severity: Severity::Error,
    };
    message.validate().unwrap();
    let wire = serde_json::to_value(&message).unwrap();
    assert_eq!(wire["kind"], "publish_message");
    assert_eq!(wire["severity"], "error");
    assert!(matches!(
        serde_json::from_value::<Operation>(wire).unwrap(),
        Operation::PublishMessage { .. }
    ));
    assert_eq!(serde_json::to_value(Value::Published).unwrap(), "Published");
    assert_eq!(serde_json::to_value(Value::Dismissed).unwrap(), "Dismissed");
    for (title, text) in [
        (String::new(), String::new()),
        ("x".repeat(257), String::new()),
        ("Git".into(), "文".repeat(1366)),
    ] {
        message = Operation::PublishMessage {
            title,
            message: text,
            severity: Severity::Warning,
        };
        assert_eq!(
            message.validate().unwrap_err().code,
            ErrorCode::InvalidRequest
        );
    }
}
