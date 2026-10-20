//! Registered context operations keep all scope and provider checks inside the ordinary SDK gate.
use super::*;
use api::{ErrorCode, Failure, Value};
use plugin_protocol::language_context::Operation;

impl State {
    /// Registration changes metadata only and cannot choose an undeclared executable or provider.
    pub(super) fn language_context_request(
        &mut self,
        operation: Operation,
    ) -> Result<Value, Failure> {
        if self
            .api
            .capabilities
            .get("language.lsp")
            .is_none_or(|version| *version < semver::Version::new(1, 6, 0))
        {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Registered contexts require language.lsp 1.6",
            ));
        }
        if !self.active
            || !self.activation_committed
            || self.roots.retired
            || self.roots.application
            || !self.permissions.contains("editor.read")
            || !self.permissions.contains("workspace.read")
        {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Project contexts require an active authorized workspace",
            ));
        }
        let principal = &self.plugin_services.principal;
        match operation {
            Operation::Register { contexts } => {
                if contexts
                    .iter()
                    .any(|context| !self.language_providers.contains_key(&context.provider))
                {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Project context provider is not owned by this instance",
                    ));
                }
                // Persistent domain metadata is owned by this provider, not by its first consumer.
                // Each later control still intersects its own complete original caller authority.
                let authority =
                    crate::EditorAuthority::new(self.plugin_services.alive.clone(), None);
                self.host_resources
                    .contexts
                    .register(
                        &principal.instance,
                        &principal.scope,
                        &self.workspace,
                        contexts,
                        authority,
                    )
                    .map(Value::LanguageContexts)
            }
            Operation::Describe { context } => self
                .host_resources
                .contexts
                .resolve(&context, &principal.instance, &principal.scope)
                .map(|context| Value::LanguageContexts(vec![context.reference])),
            Operation::Control {
                context,
                request,
                timeout_ms,
            } => {
                let selected = self.host_resources.contexts.resolve(
                    &context,
                    &principal.instance,
                    &principal.scope,
                )?;
                if selected.reference.descriptor.provider != request.provider {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Context belongs to another provider",
                    ));
                }
                self.editor_request_with_context(
                    api::EditorOperation::LanguageRequest { request },
                    timeout_ms,
                    Some(selected),
                )
            }
            Operation::Refresh {
                context,
                provider,
                document,
            } => {
                let declaration = self.language_providers.get(&provider).ok_or_else(|| {
                    Failure::new(ErrorCode::PermissionDenied, "Refresh provider is not owned")
                })?;
                if !declaration.primary
                    || declaration.internal
                    || !declaration
                        .delegate
                        .as_ref()
                        .is_some_and(|delegate| delegate.interactive)
                {
                    return Err(Failure::new(
                        ErrorCode::InvalidRequest,
                        "Refresh requires the owned interactive primary provider",
                    ));
                }
                let selected = self.host_resources.contexts.resolve(
                    &context,
                    &principal.instance,
                    &principal.scope,
                )?;
                api::ResourceIdentity::Local {
                    path: document.path.clone(),
                }
                .validate()?;
                self.editor_request_with_context(
                    api::EditorOperation::RefreshLanguageContext { provider, document },
                    30_000,
                    Some(selected),
                )
            }
        }
    }
}
