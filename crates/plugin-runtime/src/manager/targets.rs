//! Dynamic discovery/build requests pin provider instances and independent preparation resource roots.
use super::{
    Manager,
    host_services::{host_caller, host_method_call, start_failure},
};
use crate::{
    Availability, Completion,
    plugin_services::{Context, Reference},
};
use plugin_protocol::{api::RequestUpdate, service::Dependency, targets};
use serde_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// One host-owned wait. Cancelling a preparation revokes its own native work, never unrelated targets.
pub struct TargetRequest {
    completion: Completion<Value>,
    alive: Arc<AtomicBool>,
    native: Arc<std::sync::Mutex<crate::native_work::NativeWork>>,
    provider: String,
    reference: Reference,
    availability: Availability,
    scope: String,
}
impl TargetRequest {
    /// Retain immutable provenance after this bounded invocation releases its native root.
    pub fn origin(&self) -> TargetOrigin {
        TargetOrigin {
            provider: self.provider.clone(),
            instance: self.reference.owner().map(str::to_owned),
            availability: self.availability.clone(),
            scope: self.scope.clone(),
        }
    }
    /// Final receipts and deadlines release the preparation root; the final executable is not started.
    pub fn status(&self) -> RequestUpdate<Value> {
        let mut status = self.completion.status();
        let native = self.native.lock().unwrap();
        if let Some(message) = native.failure() {
            self.alive.store(false, Ordering::Release);
            self.completion.retire();
            return RequestUpdate::Completed {
                result: Err(plugin_protocol::api::Failure::new(
                    plugin_protocol::api::ErrorCode::OperationFailed,
                    message,
                )),
            };
        }
        if native.stopped() && native.drained() {
            self.alive.store(false, Ordering::Release);
            self.completion.retire();
            status = self.completion.status();
        } else if status.is_terminal() {
            self.alive.store(false, Ordering::Release);
            // Do not publish an artifact/control receipt while owned native work still needs cleanup.
            if !native.drained() {
                return RequestUpdate::Accepted;
            }
        }
        status
    }
    /// Caller identity remains valid only in the original workspace and provider incarnation.
    pub fn valid_for(&self, manager: &Manager) -> bool {
        self.origin().valid_for(manager)
    }
    /// Observe this invocation's bounded native history independently from another configuration.
    pub fn snapshot(&self) -> crate::PreparationSnapshot {
        let mut snapshot = self.native.lock().unwrap().snapshot();
        snapshot.provider = self.provider.clone();
        snapshot
    }
    /// Seal new effects, request supported normal exit, then force after the same bounded grace as Run.
    pub fn stop_with(&self, mode: plugin_protocol::process::ExitMode) {
        self.native.lock().unwrap().stop(mode);
        if self.native.lock().unwrap().drained() {
            self.alive.store(false, Ordering::Release);
            self.completion.retire();
        }
    }
    /// Immediate abandonment revokes all resources; explicit normal Stop uses stop_with instead.
    pub fn stop(&self) {
        self.alive.store(false, Ordering::Release);
        self.completion.retire();
    }
}
impl Drop for TargetRequest {
    /// Abandoned waits own cleanup; their external side effects are never claimed to roll back.
    fn drop(&mut self) {
        self.stop();
    }
}
impl Manager {
    /// Snapshot compatible static configuration contributors without starting an invocation.
    /// Restricted or closed workspaces have no valid origins; this does not grant permissions.
    pub fn configuration_origins(&mut self) -> Vec<TargetOrigin> {
        if !self.trusted || !self.workspace_open {
            return vec![];
        }
        self.configuration_providers()
            .into_iter()
            .filter_map(|provider| {
                let availability = self.availability(&provider).ok()?;
                let instance = self.instance_id(&provider).map(str::to_owned);
                Some(TargetOrigin {
                    provider,
                    instance,
                    availability,
                    scope: self.host_scope(),
                })
            })
            .collect()
    }
    /// Validate any embedded native document against the actual provider's negotiated public capabilities.
    /// This generic admission gate is reusable by host-owned surfaces and never grants additional access.
    pub fn validate_native_document(
        &self,
        provider: &str,
        document: &plugin_protocol::ui::Document,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.live.contains_key(provider),
            "Native UI provider is unavailable"
        );
        let entry = self
            .installed
            .get(provider)
            .ok_or_else(|| anyhow::anyhow!("Unknown native UI provider"))?;
        let negotiated = crate::capabilities::negotiate(&entry.manifest)?;
        document.validate().map_err(anyhow::Error::msg)?;
        for (name, version) in document.required_capabilities() {
            anyhow::ensure!(
                negotiated
                    .capabilities
                    .get(name)
                    .is_some_and(|actual| *actual >= version),
                "{name} was not negotiated for the native document"
            );
        }
        Ok(())
    }
    /// List compatible static contributors without selecting a default or starting project tools.
    pub fn target_providers(&self) -> Vec<String> {
        let contract = targets::declaration();
        let dependency = Dependency {
            version: ">=1.0,<2".parse().unwrap(),
            optional: false,
            methods: contract.methods,
        };
        self.published_entries()
            .into_iter()
            .filter(|entry| {
                entry.enabled
                    && entry.error.is_none()
                    && self.trusted
                    && self.workspace_open
                    && entry.manifest.scope == plugin_protocol::api::InstanceScope::Workspace
                    && entry
                        .manifest
                        .plugin_services
                        .provides
                        .get(targets::CONTRACT)
                        .is_some_and(|contract| dependency.matches(contract))
            })
            .map(|entry| entry.manifest.id)
            .collect()
    }
    /// Begin only a known method on an explicit contributor. Schema, grants and trust precede effects.
    pub fn begin_target_call(
        &mut self,
        provider: &str,
        method: &str,
        arguments: Value,
    ) -> anyhow::Result<TargetRequest> {
        anyhow::ensure!(
            self.trusted && self.workspace_open,
            "Restricted/closed workspace cannot prepare targets"
        );
        anyhow::ensure!(
            matches!(method, "discover" | "prepare"),
            "Unknown target method"
        );
        self.begin_configuration_service(
            provider,
            targets::CONTRACT,
            targets::declaration(),
            method,
            arguments,
        )
    }

    /// All compatible static plugins can supply command templates, forms and validation on real demand.
    pub fn configuration_providers(&self) -> Vec<String> {
        let contract = plugin_protocol::configurations::declaration();
        let dependency = Dependency {
            version: ">=1.0,<2".parse().unwrap(),
            optional: false,
            methods: contract.methods,
        };
        self.published_entries()
            .into_iter()
            .filter(|entry| {
                entry.enabled
                    && entry.error.is_none()
                    && self.trusted
                    && self.workspace_open
                    && entry.manifest.scope == plugin_protocol::api::InstanceScope::Workspace
                    && entry
                        .manifest
                        .plugin_services
                        .provides
                        .get(plugin_protocol::configurations::CONTRACT)
                        .is_some_and(|contract| dependency.matches(contract))
            })
            .map(|entry| entry.manifest.id)
            .collect()
    }

    /// Retain an original configuration call and bind its first instance once, with bounded resources.
    /// Permission or schema failures are returned before any guest callback or native process starts.
    pub fn begin_configuration_call(
        &mut self,
        provider: &str,
        method: &str,
        arguments: Value,
    ) -> anyhow::Result<TargetRequest> {
        self.begin_configuration_service(
            provider,
            plugin_protocol::configurations::CONTRACT,
            plugin_protocol::configurations::declaration(),
            method,
            arguments,
        )
    }

    /// Existing target and configuration consumers share instance pinning and native cleanup roots.
    fn begin_configuration_service(
        &mut self,
        provider: &str,
        name: &str,
        contract: plugin_protocol::service::Contract,
        method: &str,
        arguments: Value,
    ) -> anyhow::Result<TargetRequest> {
        anyhow::ensure!(
            self.trusted && self.workspace_open,
            "Restricted/closed workspace cannot configure targets"
        );
        anyhow::ensure!(
            contract.methods.contains_key(method),
            "Unknown configuration method"
        );
        let dependency = Dependency {
            version: ">=1.0,<2".parse().unwrap(),
            optional: false,
            methods: contract.methods,
        };
        let scope = self.host_scope();
        let caller = host_caller(&scope);
        self.refresh_services();
        let availability = self.availability(provider).map_err(start_failure)?;
        let reference = self
            .plugin_services
            .lock()
            .unwrap()
            .resolve_named(&caller, name, &dependency, provider)
            .map_err(start_failure)?;
        let alive = Arc::new(AtomicBool::new(true));
        let native = Arc::new(std::sync::Mutex::new(
            crate::native_work::NativeWork::default(),
        ));
        self.host_resources.preparations.register(&alive, &native);
        let origin = Context {
            native_waits: Vec::new(),
            menu: None,
            origin: crate::plugin_services::InvocationOrigin::Delegated,
            caller: caller.clone(),
            permissions: caller.permissions.clone(),
            ancestry: vec![],
            lifetimes: vec![self.host_alive.clone(), alive.clone()],
        };
        let mut completion = Completion::new(if method == "prepare" { 120_000 } else { 30_000 });
        completion.lifetimes = origin.lifetimes.clone();
        let owner = reference.provider.clone();
        let mut call = host_method_call(
            &caller,
            reference.clone(),
            method,
            arguments,
            &dependency,
            completion.clone(),
            self.host_alive.clone(),
        )
        .map_err(start_failure)?;
        call.context = origin
            .delegate(&owner, &call.signature)
            .map_err(start_failure)?;
        self.plugin_services
            .lock()
            .unwrap()
            .enqueue(call)
            .map_err(start_failure)?;
        Ok(TargetRequest {
            completion,
            alive,
            native,
            provider: provider.into(),
            reference,
            availability,
            scope,
        })
    }
}

/// Read-only provenance of a public service receipt. The runtime alone creates these values.
/// Consumers recheck provenance before accepting delayed results; ordinary permission checks still apply.
#[derive(Clone, Debug)]
pub struct TargetOrigin {
    provider: String,
    instance: Option<String>,
    availability: Availability,
    scope: String,
}
impl PartialEq for TargetOrigin {
    /// Static provenance survives one first birth; retirement/configuration changes advance availability.
    /// A completed receipt still keeps its exact owner for valid_for, independently of discovery equality.
    fn eq(&self, other: &Self) -> bool {
        self.provider == other.provider
            && self.scope == other.scope
            && self.availability == other.availability
    }
}
impl Eq for TargetOrigin {}
impl TargetOrigin {
    /// Whether the original trusted workspace and live provider incarnation still exist.
    pub fn valid_for(&self, manager: &Manager) -> bool {
        manager.trusted
            && manager.workspace_open
            && manager.host_scope() == self.scope
            && manager.availability_current(&self.availability)
            && self.instance.as_ref().is_none_or(|instance| {
                manager.instance_id(&self.provider) == Some(instance.as_str())
            })
    }
    /// Identify presentation ownership without exposing a constructible caller reference.
    pub fn provider(&self) -> &str {
        &self.provider
    }
}
