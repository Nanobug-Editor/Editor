//! Public cloud request validation rejects credential-bearing URLs and header injection before IO.
use plugin_protocol::http;

/// A cloud query may carry an authorization header but cannot smuggle another HTTP message.
#[test]
fn cloud_requests_reject_unsafe_targets_and_headers() {
    let mut request = http::Request::get("https://api.example.test/v1/query");
    request.headers.push(http::Header {
        name: "Authorization".into(),
        value: "Bearer fake-test-token".into(),
    });
    assert!(request.validate().is_ok());
    request.headers.push(http::Header {
        name: "X-Query".into(),
        value: "query\r\nHost: other.test".into(),
    });
    assert!(request.validate().is_err());
    request.headers.pop();
    request.headers.push(http::Header {
        name: "X-Query".into(),
        value: "中文".into(),
    });
    assert!(request.validate().is_err());
    request.headers.pop();
    request.url = "https://user:secret@api.example.test/v1/query".into();
    assert!(request.validate().is_err());
    request.url = "file:///C:/private.txt".into();
    assert!(request.validate().is_err());
}

/// Arbitrary Token-bearing values stay out of implicit Debug output, including response cookies and bytes.
#[test]
fn implicit_cloud_diagnostics_redact_sensitive_payloads() {
    use plugin_protocol::{api, secrets};
    let token = "fake-diagnostic-token";
    let byte_representation = format!("{:?}", token.as_bytes());
    let mut request = http::Request::get(format!("https://api.example.test/?token={token}"));
    request.headers.push(http::Header {
        name: "Authorization".into(),
        value: token.into(),
    });
    request.body = token.as_bytes().to_vec();
    let header = http::Header {
        name: "Set-Cookie".into(),
        value: token.into(),
    };
    for diagnostic in [
        format!("{request:?}"),
        format!("{header:?}"),
        format!(
            "{:?}",
            http::Event::Headers {
                status: 200,
                headers: vec![header]
            }
        ),
        format!(
            "{:?}",
            http::Event::Chunk {
                bytes: token.as_bytes().to_vec()
            }
        ),
        format!(
            "{:?}",
            secrets::Operation::Write {
                key: "api-token".into(),
                bytes: token.as_bytes().to_vec()
            }
        ),
        format!("{:?}", api::Value::Secret(Some(token.as_bytes().to_vec()))),
        format!("{:?}", api::Value::Bytes(token.as_bytes().to_vec())),
    ] {
        assert!(!diagnostic.contains(token), "{diagnostic}");
        assert!(!diagnostic.contains(&byte_representation), "{diagnostic}");
    }
}
