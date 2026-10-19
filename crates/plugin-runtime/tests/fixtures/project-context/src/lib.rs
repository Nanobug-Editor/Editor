//! An unfamiliar language chooses a project and approved startup candidate through the public SDK.
use plugin_protocol::{
    api,
    bindings::{Guest, export},
    language,
};
struct ProjectContexts;

impl Guest for ProjectContexts {
    /// Discovery is source-scoped and readonly; no process, host branch or mutable editor copy is needed.
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| match input {
            api::Input::Event {
                event: api::Notification::LanguageService(context),
                ..
            } => {
                let Some(document) = context.document else {
                    return Ok(api::Output {
                        language_service: Some(language::Proposal::default()),
                        ..Default::default()
                    });
                };
                let root = document.path.split('/').next().unwrap_or("first");
                Ok(api::Output {
                    language_service: Some(language::Proposal {
                        service: Some(if root == "second" { "second" } else { "first" }.into()),
                        project_root: Some(root.into()),
                        context: Some(language::ProjectContext {
                            key: root.into(),
                            generation: document.generation,
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
            }
            _ => Ok(Default::default()),
        })
    }
}
export!(ProjectContexts);
