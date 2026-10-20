//! A new independent consumer observes versioned public wire operations in its ordinary native view.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    language_context, ui,
};
use serde_json::Value;

struct ContextSelection;
impl Guest for ContextSelection {
    /// The frozen 0.1.0 package supplied the pre-extension wire RED. This current consumer uses
    /// the exported SDK type, without manufacturing a native owner or a private testing entry point.
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            if matches!(input, api::Input::Snapshot) {
                return Ok(api::Output {
                    snapshot: Some(plugin_protocol::Snapshot {
                        schema: 1,
                        data: "{}".into(),
                    }),
                    ..Default::default()
                });
            }
            if let api::Input::Event {
                event: api::Notification::Command { arguments, .. },
                ..
            } = input
            {
                let operation: language_context::Operation =
                    serde_json::from_value(arguments.unwrap_or(Value::Null)).map_err(|error| {
                        api::Failure::new(api::ErrorCode::InvalidRequest, error.to_string())
                    })?;
                let response = api::guest::request(api::Operation::LanguageContexts { operation });
                let response = serde_json::to_value(response).map_err(|error| {
                    api::Failure::new(api::ErrorCode::OperationFailed, error.to_string())
                })?;
                return Ok(api::Output {
                    views: vec![api::View {
                        panel: "status".into(),
                        document: ui::Document::new(ui::Node::text("result", response.to_string())),
                    }],
                    ..Default::default()
                });
            }
            Ok(Default::default())
        })
    }
}
export!(ContextSelection);
