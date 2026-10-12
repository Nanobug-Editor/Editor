//! The key-value package supplies its own analysis configuration over the public language hook.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    language,
};
use serde_json::json;
struct KeyValue;
impl Guest for KeyValue {
    /// The declared verified server remains fixed; this hook supplies only language-domain options.
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| match input {
            api::Input::Event {
                event: api::Notification::LanguageService(_),
                ..
            } => Ok(api::Output {
                language_service: Some(language::Proposal {
                    initialization_options: Some(json!({"keyValue":{"defaultPort":8080}})),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            api::Input::Snapshot => Ok(api::Output {
                snapshot: Some(Default::default()),
                ..Default::default()
            }),
            _ => Ok(Default::default()),
        })
    }
}
export!(KeyValue);
