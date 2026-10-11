//! Configuration rejection and unpublished snapshots exercise the public temporary-owner lifecycle.
use plugin_protocol::{api, settings, storage, ui};
use serde_json::{Value, json};
use std::cell::RefCell;

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
#[derive(Default)]
struct State {
    reject_phase: String,
    policy: Value,
    receipt: String,
    revision: u64,
}
impl State {
    /// Only logical rejection policy crosses snapshots; temporary handles always belong to one live guest.
    fn dispatch(&mut self, input: api::Input) -> Result<api::Output, api::Failure> {
        match input {
            api::Input::Prepare { snapshot, .. } => {
                require_unpublished_temporary("prepare")?;
                self.reject_phase = snapshot
                    .and_then(|snapshot| serde_json::from_str::<Value>(&snapshot.data).ok())
                    .and_then(|state| state["reject_phase"].as_str().map(str::to_owned))
                    .unwrap_or_default();
                self.policy = api::guest::read_asset("snapshot-policy.json")
                    .ok()
                    .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                    .unwrap_or_default();
                if self.reject_phase == "prepare" {
                    return Err(rejected("prepare"));
                }
            }
            api::Input::Snapshot => {
                let outcome = if self.policy["open_temporary"].as_bool() == Some(true) {
                    // A candidate may already have flushed staged files; that must not grant directory ownership.
                    Some(match storage::open(storage::Area::Temporary) {
                        Ok(handle) => {
                            api::guest::request(api::Operation::WriteFile {
                                handle: handle.clone(),
                                path: "candidate.txt".into(),
                                bytes: b"unpublished temporary bytes".to_vec(),
                            })?;
                            api::guest::request(api::Operation::CloseResource { handle })?;
                            "allowed".into()
                        }
                        Err(error) => format!("{:?}", error.code),
                    })
                } else {
                    None
                };
                if self.policy["fail"].as_bool() == Some(true) {
                    return Err(api::Failure::new(
                        api::ErrorCode::OperationFailed,
                        format!(
                            "Candidate snapshot rejected; temporary={}",
                            outcome.unwrap()
                        ),
                    ));
                }
                return Ok(api::Output {
                    snapshot: Some(plugin_protocol::Snapshot {
                        schema: 1,
                        data: json!({"reject_phase":self.reject_phase,"temporary_outcome":outcome})
                            .to_string(),
                    }),
                    ..Default::default()
                });
            }
            api::Input::Event {
                event: api::Notification::Configuration { phase, values },
                ..
            } => {
                require_unpublished_temporary(match phase {
                    settings::Phase::Validate => "validate",
                    settings::Phase::Apply => "apply",
                })?;
                let reject = values["reject"].value.as_bool() == Some(true);
                match phase {
                    settings::Phase::Validate => {
                        let mut proposal = settings::Proposal::default();
                        if reject && self.reject_phase == "validate" {
                            proposal
                                .errors
                                .insert("reject".into(), "validate rejected".into());
                        }
                        return Ok(api::Output {
                            configuration: Some(proposal),
                            ..Default::default()
                        });
                    }
                    settings::Phase::Apply if reject && self.reject_phase == "apply" => {
                        return Err(rejected("apply"));
                    }
                    _ => {}
                }
            }
            api::Input::Event {
                event: api::Notification::Command { id, arguments, .. },
                ..
            } if id == "reject-at" => {
                self.reject_phase = arguments
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .unwrap_or_default();
            }
            api::Input::Event {
                event: api::Notification::Command { id, arguments, .. },
                ..
            } if id == "probe" => {
                let operation =
                    serde_json::from_value(arguments.unwrap_or_default()).map_err(|_| {
                        api::Failure::new(api::ErrorCode::InvalidRequest, "Invalid private probe")
                    })?;
                self.receipt = serde_json::to_string(&api::guest::request(operation)).unwrap();
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
/// Every preactivation phase checks the actual SDK reply; a missing grant cannot pass as lifecycle denial.
fn require_unpublished_temporary(phase: &str) -> Result<(), api::Failure> {
    match storage::open(storage::Area::Temporary) {
        Err(error) if error.code == api::ErrorCode::InvalidState => Ok(()),
        Err(error) => Err(error),
        Ok(handle) => {
            api::guest::request(api::Operation::CloseResource { handle })?;
            Err(api::Failure::new(
                api::ErrorCode::OperationFailed,
                format!("{phase} unexpectedly acquired temporary ownership"),
            ))
        }
    }
}
/// Guest failures remain typed so the public Manager must keep the previous configuration owner alive.
fn rejected(phase: &str) -> api::Failure {
    api::Failure::new(api::ErrorCode::OperationFailed, format!("{phase} rejected"))
}
struct Guest;
impl plugin_protocol::bindings::Guest for Guest {
    /// The independently built WASM returns the SDK completion envelope, without a host test shortcut.
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
