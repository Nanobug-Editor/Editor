//! Provider-owned language control is validated before it reaches the native editor dispatcher.
use super::*;
use api::{ErrorCode, Failure};

impl State {
    /// Check the control envelope, original scope and declared method without running a native tool.
    /// Returns a structured failure before allocating a request when any grant or schema is missing.
    pub(super) fn prepare_language_control(
        &self,
        request: &plugin_protocol::language_control::Request,
    ) -> Result<crate::LanguageControlMetadata, Failure> {
        if !request.valid() {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid or oversized language control request",
            ));
        }
        let provider = self
            .language_providers
            .get(&request.provider)
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::PermissionDenied,
                    "Language provider is not owned by this instance",
                )
            })?;
        let method = provider
            .control_methods
            .get(&request.method)
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::UnsupportedOperation,
                    "Language control method was not declared",
                )
            })?;
        if !method.permissions.is_subset(&self.permissions)
            || self
                .plugin_services
                .context
                .as_ref()
                .is_some_and(|context| !method.permissions.is_subset(&context.permissions))
        {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Language control requires the original caller's declared read authority",
            ));
        }
        // Related-source capture reads beyond the target document independently of the method's
        // argument schema. Both this owner and the sealed original call chain must authorize it;
        // declaring a weaker custom method cannot lend the provider's workspace grant to callers.
        if request.context_snapshot
            && (!self.permissions.contains("workspace.read")
                || self
                    .plugin_services
                    .context
                    .as_ref()
                    .is_some_and(|context| !context.permissions.contains("workspace.read")))
        {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Related language sources require the original caller's workspace.read authority",
            ));
        }
        method.parameters.accepts(&request.arguments)?;
        let principal = &self.plugin_services.principal;
        let original = self
            .plugin_services
            .context
            .as_ref()
            .map_or(principal, |context| &context.caller);
        let borrower = self
            .plugin_services
            .context
            .as_ref()
            .and_then(|context| {
                context
                    .ancestry
                    .len()
                    .checked_sub(2)
                    .and_then(|index| context.ancestry.get(index))
            })
            .cloned()
            .unwrap_or_else(|| principal.instance.clone());
        let mut imports = Vec::new();
        let mut collection_guards = request
            .service_sets
            .iter()
            .map(|set| self.service_set_guard(&set.contract, set.revision))
            .collect::<Result<Vec<_>, _>>()?;
        let mut authority = crate::EditorAuthority::new(
            self.plugin_services.alive.clone(),
            self.plugin_services.context.as_ref(),
        );
        for guard in &collection_guards {
            authority = authority.with_guard(guard.clone());
        }
        if !request.imports.is_empty() || request.publish_binding.is_some() {
            if !self.api.capabilities.contains_key("language.modules") {
                return Err(Failure::new(
                    ErrorCode::CapabilityUnavailable,
                    "language.modules was not negotiated",
                ));
            }
            if !method.permissions.contains("native.modules")
                || !self.permissions.contains("native.modules")
                || self
                    .plugin_services
                    .context
                    .as_ref()
                    .is_some_and(|context| !context.permissions.contains("native.modules"))
            {
                return Err(Failure::new(
                    ErrorCode::PermissionDenied,
                    "Native module loading requires each participant's explicit grant",
                ));
            }
            for import in &request.imports {
                let mut import_authority = authority.clone();
                let source = if let Some(handle) = &import.reference {
                    self.roots.resolve(handle)?;
                    let reference = self
                        .plugin_services
                        .references
                        .get(&handle.resource)
                        .ok_or_else(|| {
                            Failure::new(
                                ErrorCode::InvalidHandle,
                                "Module import requires an owned service reference",
                            )
                        })?;
                    let broker = self.plugin_services.broker.lock().unwrap();
                    broker.validate_reference(principal, reference)?;
                    // A usable reference does not outlive the composition that selected it. The
                    // guard follows both executable code and the asynchronous result/binding gate.
                    if let Some(guard) = &reference.collection {
                        import_authority = import_authority.with_guard(guard.clone());
                        collection_guards.push(guard.clone());
                    }
                    broker
                        .reference_provider(reference)?
                        .caller
                        .instance
                        .clone()
                } else {
                    // The immediate trusted hop owns exported code; the original consumer still owns
                    // contexts and permissions. This preserves fixture -> adapter -> engine composition.
                    borrower.clone()
                };
                imports.push(self.host_resources.modules.import(
                    &source,
                    &principal.scope,
                    &principal.instance,
                    &import.export,
                    import_authority,
                    &self.workspace,
                    &request.document,
                )?);
            }
        }
        Ok(crate::LanguageControlMetadata {
            collection_guards,
            provider: format!("{}/{}", principal.plugin, request.provider),
            owner: principal.instance.clone(),
            consumer: original.instance.clone(),
            borrower,
            scope: principal.scope.clone(),
            binding_document: request.publish_binding.clone(),
            bindings: self.host_resources.bindings.clone(),
            response: method.result.clone(),
            imports,
        })
    }
}
