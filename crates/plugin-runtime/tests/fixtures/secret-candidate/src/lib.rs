//! Snapshot mutation attempts distinguish prepared private-data writes from published credential authority.
use plugin_protocol::{api, secrets, ui};
use std::cell::RefCell;

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
#[derive(Default)]
struct State {
    policy: serde_json::Value,
    receipt: String,
    revision: u64,
}
impl State {
    /// Only the fixture's optional immutable policy chooses a mutation; the host still decides authority.
    fn dispatch(&mut self, input: api::Input) -> Result<api::Output, api::Failure> {
        match input {
            api::Input::Prepare { .. } => {
                self.policy = api::guest::read_asset("snapshot-policy.json")
                    .ok()
                    .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                    .unwrap_or_default();
            }
            api::Input::Snapshot => {
                let attempt = match self.policy["mutation"].as_str() {
                    Some("write") => Some(secrets::write(
                        "api-token",
                        b"fake-unpublished-replacement".to_vec(),
                    )),
                    Some("delete") => Some(secrets::delete("api-token")),
                    _ => None,
                };
                let outcome = attempt.map(|result| match result {
                    Ok(()) => "allowed".into(),
                    Err(error) => format!("{:?}", error.code),
                });
                if self.policy["fail"].as_bool() == Some(true) {
                    return Err(api::Failure::new(
                        api::ErrorCode::OperationFailed,
                        format!(
                            "Candidate snapshot rejected; mutation={}",
                            outcome.as_deref().unwrap_or("none")
                        ),
                    ));
                }
                return Ok(api::Output {
                    snapshot: Some(plugin_protocol::Snapshot {
                        schema: 1,
                        data: serde_json::json!({"mutation_outcome":outcome}).to_string(),
                    }),
                    ..Default::default()
                });
            }
            api::Input::Event {
                event: api::Notification::Command { id, arguments, .. },
                ..
            } if id == "probe" => {
                let operation: api::Operation =
                    serde_json::from_value(arguments.unwrap_or_default()).map_err(|_| {
                        api::Failure::new(api::ErrorCode::InvalidRequest, "Invalid probe")
                    })?;
                let result = api::guest::request(operation).map(|value| match value {
                    api::Value::Secret(value) => api::Value::Bytes(if value.is_some() {
                        b"present".to_vec()
                    } else {
                        b"missing".to_vec()
                    }),
                    other => other,
                });
                self.receipt = serde_json::to_string(&result).unwrap();
            }
            _ => {}
        }
        self.revision += 1;
        Ok(api::Output {
            views: vec![api::View {
                panel: "cloud".into(),
                document: ui::Document::new(ui::Node::new(
                    "receipt",
                    ui::Kind::Text {
                        text: self.receipt.clone(),
                    },
                ))
                .revision(self.revision),
            }],
            ..Default::default()
        })
    }
}
struct Guest;
impl plugin_protocol::bindings::Guest for Guest {
    /// Typed completion failures remain visible to the production installation transaction.
    fn dispatch(payload: String) -> Result<String, String> {
        let invocation: api::Invocation =
            serde_json::from_str(&payload).map_err(|error| error.to_string())?;
        let result = STATE.with(|state| state.borrow_mut().dispatch(invocation.message));
        serde_json::to_string(&api::Completion {
            id: invocation.id,
            result,
        })
        .map_err(|error| error.to_string())
    }
}
plugin_protocol::bindings::export!(Guest with_types_in plugin_protocol::bindings);
