//! Public configuration provider retaining arbitrary literal launch/preparation inputs for regression tests.
//! No executable is started here: validation returns the public Launch shape to the normal host boundary.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    configurations, service,
};
use serde_json::{Value, json};

struct LaunchFixture;
impl Guest for LaunchFixture {
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| match input {
            api::Input::Snapshot => Ok(api::Output {
                snapshot: Some(Default::default()),
                ..Default::default()
            }),
            api::Input::Event {
                event: api::Notification::Service(service::Notification::Invoke(call)),
                ..
            } => {
                let answer = match call.method.as_str() {
                    "catalog" => Ok(json!({"templates":[]})),
                    "validate" => validate(&call.arguments),
                    _ => Err(api::Failure::new(
                        api::ErrorCode::InvalidRequest,
                        "Unsupported fixture method",
                    )),
                };
                Ok(api::Output {
                    service_reply: Some(answer.map(|value| json!({"payload":value.to_string()}))),
                    ..Default::default()
                })
            }
            _ => Ok(Default::default()),
        })
    }
}
export!(LaunchFixture);

/// Decode a bounded public Launch; malformed values fail before any host execution is admitted.
fn validate(arguments: &Value) -> Result<Value, api::Failure> {
    let launch: configurations::Launch = serde_json::from_str(
        arguments["values"].as_str().unwrap_or_default(),
    )
    .map_err(|error| api::Failure::new(api::ErrorCode::InvalidRequest, error.to_string()))?;
    serde_json::to_value(configurations::Validation {
        valid: true,
        message: String::new(),
        launch: Some(launch),
    })
    .map_err(|error| api::Failure::new(api::ErrorCode::InvalidRequest, error.to_string()))
}
