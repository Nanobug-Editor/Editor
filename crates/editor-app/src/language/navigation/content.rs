//! Declared non-file definitions use the owned transport and return inert readonly UTF-8 content.
use super::*;
use plugin_runtime::plugin_protocol::{
    api::{ErrorCode, Failure},
    language_content::Content,
};

impl LanguageServer {
    /// Fetch one declared URI for a still-live source. Cancellation and provider/connection
    /// retirement reject late results; presentation additionally rechecks the native revision.
    pub(crate) fn content_for(
        &self,
        document: DocumentLease,
        uri: String,
    ) -> anyhow::Result<String> {
        let declaration = self
            .service
            .provider
            .content
            .as_ref()
            .filter(|content| content.accepts(&uri))
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::UnsupportedOperation,
                    "Undeclared language content URI",
                )
            })?;
        ensure!(
            self.service.capability("language.content").is_some(),
            "Language content was not negotiated"
        );
        let cancellation = RequestCancellation::default();
        let mut connection = self.feature_connection(&document, &cancellation)?;
        let current = connection
            .as_mut()
            .context("LSP provider has been retired")?;
        current.bind_document(&document)?;
        let response =
            current.request_cancellable(&declaration.method, json!({"uri":uri}), &cancellation);
        self.connection_failed(&declaration.method, &response, &mut connection);
        self.check_feature_lifetime(&document, &cancellation)?;
        let content: Content =
            serde_json::from_value(response?).context("Decode readonly language content")?;
        ensure!(
            content.text.len() <= 1024 * 1024,
            "Readonly language content exceeds 1 MiB"
        );
        Ok(content.text)
    }
}
