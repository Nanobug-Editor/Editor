//! General cloud calls share negotiation, delegation and the instance's existing revocable resource slots.
use super::*;
use api::{ErrorCode, Failure, Value};
use resource_roots::RootKind;
use std::time::{Duration, Instant};

pub(super) struct Timer {
    handle: api::ResourceHandle,
    next: Instant,
    delay: Duration,
    repeat: bool,
    sequence: u64,
}
impl State {
    /// Network, credentials and scheduling never acquire authority from another plugin or preparation hook.
    fn cloud_authority(&self, capability: &str, permission: Option<&str>) -> Result<(), Failure> {
        if !self.api.capabilities.contains_key(capability) {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                format!("{capability} was not negotiated"),
            ));
        }
        if !self.active
            || self.roots.retired
            || self.migrating
            || self.language_hook
            || !self
                .plugin_services
                .alive
                .load(std::sync::atomic::Ordering::Acquire)
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Cloud operations require an active instance",
            ));
        }
        if permission.is_some_and(|permission| !self.permissions.contains(permission)) {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Cloud operation permission is not granted",
            ));
        }
        Ok(())
    }
    pub(super) fn http_request(
        &mut self,
        operation: plugin_protocol::http::Operation,
    ) -> Result<Value, Failure> {
        self.cloud_authority("network.http", Some("network.http"))?;
        match operation {
            plugin_protocol::http::Operation::Start { request } => {
                self.roots.ensure_capacity()?;
                // Delegated HTTP intersects exact target grants as well as the base network permission.
                let permissions = self
                    .plugin_services
                    .context
                    .as_ref()
                    .map(|context| {
                        self.permissions
                            .intersection(&context.permissions)
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_else(|| self.permissions.clone());
                let stream =
                    crate::http::start(request, permissions, self.host_resources.http.clone())?;
                let Value::Resource(handle) = self.roots.open(RootKind::Http)? else {
                    unreachable!()
                };
                self.http_streams.insert(handle.resource, stream);
                Ok(Value::Resource(handle))
            }
            plugin_protocol::http::Operation::Read { handle } => {
                if !matches!(self.roots.resolve(&handle)?, RootKind::Http) {
                    return Err(Failure::new(ErrorCode::InvalidHandle, "Not a HTTP stream"));
                }
                self.http_streams
                    .get_mut(&handle.resource)
                    .ok_or_else(|| Failure::new(ErrorCode::InvalidHandle, "HTTP stream released"))?
                    .read()
                    .map(Value::Http)
            }
        }
    }
    pub(super) fn secret_request(
        &mut self,
        operation: plugin_protocol::secrets::Operation,
    ) -> Result<Value, Failure> {
        self.cloud_authority("storage.secrets", Some("secrets"))?;
        // Even an authorized service caller cannot access the provider's private namespace or Token.
        if self
            .plugin_services
            .context
            .as_ref()
            .is_some_and(|context| {
                !context.direct_host_selection(&self.plugin_services.principal.instance)
            })
        {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "Secret access cannot be delegated",
            ));
        }
        let (plugin_protocol::secrets::Operation::Read { key }
        | plugin_protocol::secrets::Operation::Write { key, .. }
        | plugin_protocol::secrets::Operation::Delete { key }) = &operation;
        if key.is_empty() || key.len() > 128 || key.chars().any(char::is_control) {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid private secret key",
            ));
        }
        use sha2::{Digest, Sha256};
        let target = format!(
            "Nanobug/plugin/{}/{:x}/{:x}",
            self.host_resources.secret_namespace,
            Sha256::digest(self.plugin_services.principal.plugin.as_bytes()),
            Sha256::digest(key.as_bytes())
        );
        let store = platform_windows::credentials::CredentialStore::new(&target)
            .map_err(|message| Failure::new(ErrorCode::OperationFailed, message))?;
        let failed = |message| {
            Failure::new(
                if cfg!(windows) {
                    ErrorCode::OperationFailed
                } else {
                    ErrorCode::UnsupportedOperation
                },
                message,
            )
        };
        match operation {
            plugin_protocol::secrets::Operation::Read { .. } => {
                store.read().map(Value::Secret).map_err(failed)
            }
            plugin_protocol::secrets::Operation::Write { bytes, .. } => {
                if !(1..=2560).contains(&bytes.len()) {
                    return Err(Failure::new(
                        ErrorCode::InvalidRequest,
                        "Secret value must be 1 to 2560 bytes",
                    ));
                }
                if self.staged_writes.is_some() {
                    return Err(Failure::new(
                        ErrorCode::InvalidState,
                        "Secret updates require committed activation",
                    ));
                }
                store.write(&bytes).map(|_| Value::Unit).map_err(failed)
            }
            plugin_protocol::secrets::Operation::Delete { .. } => {
                if self.staged_writes.is_some() {
                    return Err(Failure::new(
                        ErrorCode::InvalidState,
                        "Secret deletion requires committed activation",
                    ));
                }
                store.delete().map(|_| Value::Unit).map_err(failed)
            }
        }
    }
    pub(super) fn timer_request(
        &mut self,
        operation: plugin_protocol::timers::Operation,
    ) -> Result<Value, Failure> {
        self.cloud_authority("lifecycle.timers", None)?;
        let (plugin_protocol::timers::Operation::Start { delay_ms, .. }
        | plugin_protocol::timers::Operation::Reset { delay_ms, .. }) = &operation;
        if !(10..=86400000).contains(delay_ms) {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Timer delay must be 10 ms to one day",
            ));
        }
        let delay = Duration::from_millis((*delay_ms).into());
        match operation {
            plugin_protocol::timers::Operation::Start { repeat, .. } => {
                if self.timers.len() >= 32 {
                    return Err(Failure::new(
                        ErrorCode::LimitExceeded,
                        "Timer quota exceeded",
                    ));
                }
                let Value::Resource(handle) = self.roots.open(RootKind::Timer)? else {
                    unreachable!()
                };
                self.timers.insert(
                    handle.resource,
                    Timer {
                        handle: handle.clone(),
                        next: Instant::now() + delay,
                        delay,
                        repeat,
                        sequence: 0,
                    },
                );
                Ok(Value::Resource(handle))
            }
            plugin_protocol::timers::Operation::Reset { handle, .. } => {
                self.roots.resolve(&handle)?;
                let timer = self
                    .timers
                    .get_mut(&handle.resource)
                    .ok_or_else(|| Failure::new(ErrorCode::InvalidHandle, "Timer was released"))?;
                timer.delay = delay;
                timer.next = Instant::now() + delay;
                Ok(Value::Unit)
            }
        }
    }
}
impl Instance {
    /// At most eight coalesced ticks per poll; callbacks retain the original service authority.
    pub(super) fn poll_timers(&mut self) -> anyhow::Result<bool> {
        let now = Instant::now();
        let mut ready = self
            .store
            .data()
            .timers
            .values()
            .filter(|timer| timer.next <= now)
            .collect::<Vec<_>>();
        // Oldest deadlines run first, so frequently repeated low-ID timers cannot starve later timers.
        ready.sort_by_key(|timer| timer.next);
        let due = ready
            .into_iter()
            .take(8)
            .map(|timer| timer.handle.clone())
            .collect::<Vec<_>>();
        for handle in &due {
            let state = self.store.data_mut();
            // A prior callback can release any of its timers in the captured due set.
            let Some(timer) = state.timers.get_mut(&handle.resource) else {
                continue;
            };
            timer.sequence = timer.sequence.saturating_add(1);
            let sequence = timer.sequence;
            let repeat = timer.repeat;
            timer.next = now + timer.delay;
            let context = state
                .plugin_services
                .resources
                .get(&handle.resource)
                .map(|(_, context)| context.clone());
            if !repeat {
                let _ = self
                    .store
                    .data_mut()
                    .resource_request(api::Operation::CloseResource {
                        handle: handle.clone(),
                    });
            }
            self.call_with_service_context(
                context,
                api::Input::Event {
                    panel: None,
                    event: api::Notification::Timer {
                        handle: handle.clone(),
                        sequence,
                    },
                },
            )?;
        }
        Ok(!due.is_empty())
    }
}
