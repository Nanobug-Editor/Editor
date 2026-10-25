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
        let borrowed = matches!(&operation, Operation::RegisterBorrowed { .. });
        if borrowed
            && self
                .api
                .capabilities
                .get("language.lsp")
                .is_none_or(|version| *version < semver::Version::new(1, 8, 0))
        {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Borrowed contexts require language.lsp 1.8",
            ));
        }
        let principal = &self.plugin_services.principal;
        match operation {
            Operation::Check { context } => {
                if self
                    .api
                    .capabilities
                    .get("language.lsp")
                    .is_none_or(|version| *version < semver::Version::new(1, 9, 0))
                {
                    return Err(Failure::new(
                        ErrorCode::CapabilityUnavailable,
                        "Context lifetime checks require language.lsp 1.9",
                    ));
                }
                self.host_resources.contexts.check(
                    &context,
                    &principal.instance,
                    &principal.scope,
                )?;
                Ok(Value::Unit)
            }
            Operation::Register { contexts } | Operation::RegisterBorrowed { contexts } => {
                if self
                    .api
                    .capabilities
                    .get("language.lsp")
                    .is_none_or(|version| *version < semver::Version::new(1, 9, 0))
                    && contexts.iter().any(|context| {
                        serde_json::to_vec(&context.data).map_or(true, |bytes| bytes.len() > 4096)
                    })
                {
                    return Err(Failure::new(
                        ErrorCode::CapabilityUnavailable,
                        "Context metadata above 4 KiB requires language.lsp 1.9",
                    ));
                }
                if contexts
                    .iter()
                    .any(|context| !self.language_providers.contains_key(&context.provider))
                {
                    return Err(Failure::new(
                        ErrorCode::PermissionDenied,
                        "Project context provider is not owned by this instance",
                    ));
                }
                // Ordinary registration remains provider-owned. Borrowed metadata additionally
                // retains the runtime-sealed caller chain, independently from this request's wait.
                let mut persistent = if borrowed {
                    self.plugin_services.context.clone()
                } else {
                    None
                };
                if let Some(context) = &mut persistent {
                    context.lifetimes = self
                        .host_resources
                        .preparations
                        .persistent_lifetimes(&context.lifetimes);
                }
                let authority = crate::EditorAuthority::new(
                    self.plugin_services.alive.clone(),
                    persistent.as_ref(),
                );
                let authority = if borrowed {
                    let mut participants = self
                        .plugin_services
                        .context
                        .as_ref()
                        .map_or_else(Vec::new, |context| context.ancestry.clone());
                    participants.push(principal.instance.clone());
                    self.host_resources
                        .contexts
                        .borrow_discovery(authority, &participants)?
                } else {
                    authority
                };
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
            Operation::Invalidate => {
                if self
                    .api
                    .capabilities
                    .get("language.lsp")
                    .is_none_or(|version| *version < semver::Version::new(1, 9, 0))
                {
                    return Err(Failure::new(
                        ErrorCode::CapabilityUnavailable,
                        "Language invalidation requires language.lsp 1.9",
                    ));
                }
                self.host_resources
                    .contexts
                    .invalidate(&principal.instance)?;
                Ok(Value::Unit)
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
