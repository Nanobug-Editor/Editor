//! Async preparation borrows a sealed transport; source providers never own its process lifetime.
use super::*;
use crate::{ActivationReceipt, Availability, Completion, DemandReason};
use plugin_protocol::{
    api::{ErrorCode, Failure, RequestUpdate},
    language::{Delegate, DocumentContext},
};
use serde_json::Value;

/// One bounded service invocation retains independent native waits until exact-intent admission.
/// Dropping the cache slot cancels its call and completes abandoned waits without retiring the lender.
pub(in crate::manager) struct Pending {
    token: Availability,
    owner: String,
    document: DocumentContext,
    call: Completion<Value>,
    waiters: Vec<(Completion<ActivationReceipt>, ActivationReceipt)>,
}
impl Drop for Pending {
    fn drop(&mut self) {
        let _ = self.call.cancel(
            plugin_protocol::api::CancelMode::TryTerminate,
            ErrorCode::Cancelled,
        );
        for (waiter, _) in &self.waiters {
            waiter.finish(Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Language delegation was retired",
            )));
        }
    }
}
impl Pending {
    /// Retain the activation actor's global native wait budget after preparation transfers ownership.
    pub(in crate::manager) fn waiter_count(&self) -> usize {
        self.waiters
            .iter()
            .filter(|(waiter, _)| !waiter.status().is_terminal())
            .count()
    }
    /// A newer document intent withdraws every old native wait before another broker tick can run.
    pub(super) fn supersede(&self, next: &DocumentContext) {
        if self.document.path == next.path && self.document.generation < next.generation {
            for (waiter, _) in &self.waiters {
                let _ = waiter.cancel(
                    plugin_protocol::api::CancelMode::TryTerminate,
                    ErrorCode::InvalidHandle,
                );
            }
            let _ = self.call.cancel(
                plugin_protocol::api::CancelMode::TryTerminate,
                ErrorCode::InvalidHandle,
            );
        }
    }
}

impl Manager {
    /// Deferred language waits share the same bounded reservation pool as activation flights.
    pub(in crate::manager) fn language_delegation_waiter_count(&self) -> usize {
        self.language_services
            .values()
            .filter_map(|slot| slot.delegation.as_ref())
            .map(Pending::waiter_count)
            .sum::<usize>()
            + self
                .parked
                .values()
                .filter_map(|project| project.resources.as_ref())
                .map(|resources| resources.language_waiter_count())
                .sum::<usize>()
    }
    /// Invoke only this selected package's already activated owner and its fixed public contract.
    /// No plugin callback or process launch runs inline; ordinary broker polling drives the request.
    pub(in crate::manager) fn begin_language_delegation(
        &mut self,
        provider: String,
        document: DocumentContext,
        fingerprint: Fingerprint,
        delegate: Delegate,
    ) {
        let result = (|| -> Result<Pending, Failure> {
            let plugin = provider.split('/').next().unwrap();
            let token = self.availability(plugin)?;
            let owner = self
                .live
                .get(plugin)
                .and_then(Instance::instance_id)
                .ok_or_else(|| {
                    Failure::new(
                        ErrorCode::InvalidHandle,
                        "Delegated provider has no live component",
                    )
                })?
                .to_owned();
            let method = plugin_protocol::language_control::binding_prepare_method();
            let contract =
                &self.installed[plugin].manifest.plugin_services.provides[&delegate.contract];
            let dependency = plugin_protocol::service::Dependency {
                collection: false,
                version: semver::VersionReq::parse(&format!("={}", contract.version)).unwrap(),
                optional: false,
                methods: BTreeMap::from([(delegate.method.clone(), method.clone())]),
            };
            let mut caller = super::super::host_services::host_caller(token.scope());
            // Native source intent authorizes this fixed read/module contract only. The callee and
            // every subsequent hop must independently retain the same installed permission grants.
            caller.permissions = method.permissions;
            self.refresh_services();
            let reference = self.plugin_services.lock().unwrap().resolve_pinned(
                &caller,
                &delegate.contract,
                &dependency,
                &owner,
            )?;
            let completion = Completion::new(120_000);
            let call = super::super::host_services::host_method_call(
                &caller,
                reference,
                &delegate.method,
                serde_json::json!({"document":document}),
                &dependency,
                completion.clone(),
                self.host_alive.clone(),
            )?;
            self.plugin_services.lock().unwrap().enqueue(call)?;
            Ok(Pending {
                token,
                owner,
                document: document.clone(),
                call: completion,
                waiters: Vec::new(),
            })
        })();
        match result {
            Ok(pending) => {
                let key = format!("{provider}!{}", serde_json::to_string(&document).unwrap());
                if let Some(slot) = self.language_services.get_mut(&key) {
                    slot.delegation = Some(pending);
                }
            }
            Err(error) => self.publish_document_language_lease(
                provider,
                document,
                None,
                fingerprint,
                Err(error.to_string()),
                false,
            ),
        }
    }

    /// Join a running delegated preparation instead of scheduling another component preparation.
    /// Each caller retains its original completion, cancellation and absolute deadline.
    pub(in crate::manager) fn join_language_delegation(
        &mut self,
        reason: &DemandReason,
        completion: Completion<ActivationReceipt>,
        receipt: ActivationReceipt,
    ) -> bool {
        let DemandReason::LanguageDocument {
            provider, document, ..
        } = reason
        else {
            return false;
        };
        let key = format!("{provider}!{}", serde_json::to_string(document).unwrap());
        let Some(pending) = self
            .language_services
            .get_mut(&key)
            .and_then(|slot| slot.delegation.as_mut())
        else {
            return false;
        };
        pending.waiters.push((completion, receipt));
        true
    }

    /// Advance delegated calls in every live workspace without borrowing the current UI projection.
    pub(in crate::manager) fn poll_delegated_languages(&mut self) {
        let scopes = std::iter::once(self.host_scope())
            .chain(self.parked.keys().cloned())
            .collect::<Vec<_>>();
        for scope in scopes {
            let _ = self.in_project(&scope, |manager| {
                manager.poll_project_delegations();
                manager.collect_unused_language_contexts();
            });
        }
    }

    /// Consume one-use authority only after the selected owner, scope and source intent still match.
    fn poll_project_delegations(&mut self) {
        let keys = self
            .language_services
            .iter()
            .filter(|(_, slot)| slot.delegation.is_some())
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for key in keys {
            let mut pending = self
                .language_services
                .get_mut(&key)
                .unwrap()
                .delegation
                .take()
                .unwrap();
            pending
                .waiters
                .retain(|(waiter, _)| !waiter.status().is_terminal());
            if pending.waiters.is_empty() {
                self.language_services.remove(&key);
                continue;
            }
            let valid = self
                .availability_in_scope(pending.token.plugin(), pending.token.scope())
                .is_ok_and(|current| current == pending.token)
                && self.document_intent_generation(
                    contexts::provider_key(&key),
                    &pending.document.path,
                ) == Some(pending.document.generation)
                && self
                    .live
                    .get(pending.token.plugin())
                    .and_then(Instance::instance_id)
                    == Some(pending.owner.as_str());
            let value = if !valid {
                Some(Err(Failure::new(
                    ErrorCode::InvalidHandle,
                    "Delegated owner changed",
                )))
            } else {
                match pending.call.status() {
                    RequestUpdate::Completed { result } => Some(result),
                    RequestUpdate::Cancelled { .. } => Some(Err(Failure::new(
                        ErrorCode::Cancelled,
                        "Delegated preparation was cancelled",
                    ))),
                    _ => None,
                }
            };
            let Some(result) = value else {
                self.language_services.get_mut(&key).unwrap().delegation = Some(pending);
                continue;
            };
            let result = result.and_then(|value| {
                let token = value
                    .get("binding")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        Failure::new(
                            ErrorCode::InvalidRequest,
                            "Delegated preparation returned no binding",
                        )
                    })?;
                self.host_resources.bindings.take(
                    token,
                    &pending.owner,
                    pending.token.scope(),
                    &pending.document,
                )
            });
            let fingerprint = self.language_services[&key].fingerprint.clone();
            let provider = contexts::provider_key(&key).to_owned();
            let project = result.as_ref().ok().and_then(|plan| {
                self.language_services.values().find_map(|slot| {
                    slot.service
                        .as_ref()
                        .ok()
                        .filter(|owned| Arc::ptr_eq(owned, plan))?;
                    slot.context.as_ref()?.project.clone()
                })
            });
            let completion = result.as_ref().map(|_| ()).map_err(Clone::clone);
            self.publish_document_language_lease(
                provider,
                pending.document.clone(),
                project,
                fingerprint,
                result.map_err(|error| error.to_string()),
                false,
            );
            for (waiter, receipt) in std::mem::take(&mut pending.waiters) {
                waiter.finish(completion.clone().map(|_| receipt));
            }
        }
    }
}
