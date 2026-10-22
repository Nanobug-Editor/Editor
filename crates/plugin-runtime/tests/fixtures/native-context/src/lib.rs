//! Native-context authority is exercised through a real independent SDK component and command.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    process, ui,
};

/// The fixture returns inert receipts and never starts the deliberately nonexistent native service.
struct NativeContextFixture;
impl Guest for NativeContextFixture {
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| match input {
            api::Input::Snapshot => Ok(api::Output {
                snapshot: Some(plugin_protocol::Snapshot {
                    schema: 1,
                    data: "{}".into(),
                }),
                ..Default::default()
            }),
            api::Input::Event {
                event: api::Notification::ServicePreparation(context),
                ..
            } => {
                let mode = context.arguments["binding"].as_str().unwrap_or("");
                if mode == "execute" {
                    api::guest::request(api::Operation::Process {
                        operation: process::Operation::Execute {
                            program: "deliberately-absent-native-context-fixture".into(),
                            args: vec![],
                            transport: process::Transport::Stdio,
                            cwd: None,
                            env: Default::default(),
                        },
                    })?;
                }
                if mode == "native-context" {
                    process::native_context("native")?;
                }
                let selection = match mode {
                    "foreign" => vec!["foreign".into()],
                    "duplicate" => vec!["native".into(), "native".into()],
                    _ => vec![],
                };
                let mut output = api::Output {
                    service_preparation: Some(selection),
                    ..Default::default()
                };
                if mode == "ui" {
                    output.views.push(api::View {
                        panel: "result".into(),
                        document: ui::Document::new(ui::Node::text(
                            "forbidden",
                            "Forbidden preparation view",
                        )),
                    });
                }
                Ok(output)
            }
            api::Input::Event {
                event: api::Notification::Service(plugin_protocol::service::Notification::Invoke(_)),
                ..
            } => Ok(api::Output {
                service_reply: Some(Ok(serde_json::json!({"program":"never-started.exe"}))),
                ..Default::default()
            }),
            api::Input::Event {
                event: api::Notification::Command { arguments, .. },
                ..
            } => {
                let result = process::native_context(
                    arguments
                        .as_ref()
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("native"),
                );
                Ok(api::Output {
                    views: vec![api::View {
                        panel: "result".into(),
                        document: ui::Document::new(ui::Node::text(
                            "result",
                            serde_json::to_string(&result).unwrap(),
                        )),
                    }],
                    ..Default::default()
                })
            }
            _ => Ok(Default::default()),
        })
    }
}
export!(NativeContextFixture);
