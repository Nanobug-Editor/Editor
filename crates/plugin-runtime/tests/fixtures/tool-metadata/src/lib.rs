//! An independent SDK consumer observes tool metadata through the readonly language preparation seam.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    language, process,
};
use serde_json::{Value, json};

/// The fixture owns no files or processes; the host supplies all declared tool identity and consent.
struct MetadataFixture;

impl Guest for MetadataFixture {
    /// Return observed metadata as inert initialization data, or propagate the original typed failure.
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            if let api::Input::Event {
                event: api::Notification::LanguageService(context),
                ..
            } = input
            {
                let policy: Value = context
                    .settings
                    .get("fixture.inspection")
                    .and_then(|setting| setting.value.as_str())
                    .map(serde_json::from_str)
                    .transpose()
                    .map_err(|error| {
                        api::Failure::new(api::ErrorCode::InvalidRequest, error.to_string())
                    })?
                    .unwrap_or_else(|| json!({}));
                let source = policy
                    .get("source")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(|error| {
                        api::Failure::new(api::ErrorCode::InvalidRequest, error.to_string())
                    })?
                    .unwrap_or(process::ToolSource::Service {
                        service: "inspected".into(),
                    });
                let files = policy
                    .get("files")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(|error| {
                        api::Failure::new(api::ErrorCode::InvalidRequest, error.to_string())
                    })?
                    .unwrap_or_else(|| vec!["metadata.txt".into()]);
                // Return typed host failures as inert data when testing the operation boundary,
                // rather than observing the outer language-preparation job's error wrapper.
                let metadata = match process::inspect_tool(source, 1, files) {
                    Ok(metadata) => serde_json::to_value(metadata).unwrap(),
                    Err(error) if policy["capture_failure"] == true => json!({"failure":error}),
                    Err(error) => return Err(error),
                };
                return Ok(api::Output {
                    language_service: Some(language::Proposal {
                        initialization_options: Some(metadata),
                        ..Default::default()
                    }),
                    ..Default::default()
                });
            }
            Ok(api::Output::default())
        })
    }
}
export!(MetadataFixture);
