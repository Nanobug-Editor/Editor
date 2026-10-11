//! Opt-in unopened file observations retain the original instance/root and bounded recovery semantics.
use super::*;
use api::{ErrorCode, Failure, Notification, Value};
use plugin_protocol::workspace::{FileEvent, FileTarget};
use resource_roots::RootKind;

impl State {
    pub(super) fn subscribe_workspace_files(
        &mut self,
        root: api::ResourceHandle,
    ) -> Result<Value, Failure> {
        if self
            .api
            .capabilities
            .get("workspace.files")
            .is_none_or(|version| *version < semver::Version::new(1, 2, 0))
        {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Workspace observations require workspace.files 1.2",
            ));
        }
        if !matches!(self.roots.resolve(&root)?, RootKind::Workspace) {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Owned workspace root required",
            ));
        }
        // Quota exhaustion is recoverable by releasing a subscription; it is not a stale root.
        if self.file_subscriptions.len() >= 8 {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Workspace observation quota is eight",
            ));
        }
        let file = self.authorize_file(
            &FileTarget {
                root,
                path: String::new(),
            },
            false,
            false,
        )?;
        let Value::Resource(handle) = self.roots.open(RootKind::FileSubscription)? else {
            unreachable!()
        };
        self.file_subscriptions.insert(
            handle.resource,
            crate::workspace_events::Subscription {
                handle: handle.clone(),
                root: file,
                events: Default::default(),
            },
        );
        Ok(Value::Resource(handle))
    }
}

impl Instance {
    /// Only the selected trusted workspace's shared native source publishes these observations.
    pub(crate) fn workspace_file_event(&mut self, event: FileEvent) {
        for subscription in self.store.data_mut().file_subscriptions.values_mut() {
            subscription.events.push(event.clone());
        }
    }

    pub(crate) fn workspace_events_failed(&mut self, error: Failure) {
        for subscription in self.store.data_mut().file_subscriptions.values_mut() {
            subscription.events.fail(error.clone());
        }
    }

    /// Each callback rechecks its resource; self-release or root replacement stops queued siblings.
    pub(super) fn poll_workspace_events(&mut self) -> anyhow::Result<()> {
        let deliveries = self
            .store
            .data_mut()
            .file_subscriptions
            .values_mut()
            .map(|subscription| {
                if let Err(error) = subscription.root.guard() {
                    subscription.events.fail(error);
                }
                (
                    subscription.handle.clone(),
                    subscription.events.take_batch(8),
                )
            })
            .collect::<Vec<_>>();
        for (handle, events) in deliveries {
            if !self
                .store
                .data()
                .file_subscriptions
                .contains_key(&handle.resource)
            {
                continue;
            }
            // A delegated subscription callback keeps its original permission intersection and source.
            // Observation data must never turn later guest calls into stronger provider-owned requests.
            let context = self
                .store
                .data()
                .plugin_services
                .resources
                .get(&handle.resource)
                .map(|(_, context)| context.clone());
            match events {
                Ok(events) => {
                    for event in events {
                        if !self
                            .store
                            .data()
                            .file_subscriptions
                            .get(&handle.resource)
                            .is_some_and(|subscription| subscription.root.is_live())
                        {
                            break;
                        }
                        self.call_with_service_context(
                            context.clone(),
                            api::Input::Event {
                                panel: None,
                                event: Notification::WorkspaceFile {
                                    subscription: handle.clone(),
                                    event,
                                },
                            },
                        )?;
                    }
                }
                Err(error) => {
                    self.store
                        .data_mut()
                        .file_subscriptions
                        .remove(&handle.resource);
                    self.store.data_mut().roots.remove(&handle);
                    self.store
                        .data_mut()
                        .plugin_services
                        .resources
                        .remove(&handle.resource);
                    self.call_with_service_context(
                        context,
                        api::Input::Event {
                            panel: None,
                            event: Notification::SubscriptionFailed {
                                subscription: handle,
                                error,
                            },
                        },
                    )?;
                }
            }
        }
        Ok(())
    }
}
