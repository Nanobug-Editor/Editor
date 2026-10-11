//! Trees are admitted by negotiated capabilities and instance ownership, never package identifiers.
use super::*;
use api::{ErrorCode, Failure, Value};
use plugin_protocol::tree;
use resource_roots::RootKind;
use std::{collections::BTreeMap, sync::Arc};

impl State {
    /// Check grants on every call; retained resources additionally carry delegated caller lifetimes.
    pub(super) fn tree_request(&mut self, operation: tree::Operation) -> Result<Value, Failure> {
        if !self.active || self.roots.retired {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Tree requires an active instance",
            ));
        }
        if !self.api.capabilities.contains_key("ui.tree")
            || !self.api.capabilities.contains_key("ui.panels")
            || !self.api.capabilities.contains_key("ui.native")
        {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "ui.tree, ui.panels and ui.native must be negotiated",
            ));
        }
        if !self.permissions.contains("ui.panels") {
            return Err(Failure::new(
                ErrorCode::PermissionDenied,
                "ui.panels grant required",
            ));
        }
        match operation {
            tree::Operation::Create => {
                if self.trees.len() >= tree::MAX_TREES {
                    return Err(Failure::new(
                        ErrorCode::LimitExceeded,
                        "Tree resource quota exceeded",
                    ));
                }
                let Value::Resource(handle) = self.roots.open(RootKind::Tree)? else {
                    unreachable!()
                };
                let authority = crate::EditorAuthority::new(
                    self.plugin_services.alive.clone(),
                    self.plugin_services.context.as_ref(),
                );
                // A delegated view belongs to its initiating source chain, like native marker layers.
                // Source retirement must release the persistent root, not only seal its projection.
                if let Some(context) = &self.plugin_services.context {
                    self.plugin_services
                        .resources
                        .insert(handle.resource, (handle.clone(), context.clone()));
                }
                self.trees.insert(
                    handle.resource,
                    Arc::new(crate::TreeResource::new(handle.clone(), authority)),
                );
                Ok(Value::Resource(handle))
            }
            tree::Operation::Refresh { tree } => {
                self.owned_tree(&tree)?.refresh()?;
                Ok(Value::Unit)
            }
            tree::Operation::Reply { request, result } => {
                let resource = self.owned_tree(&request.tree)?;
                let mut icons = BTreeMap::new();
                if let Ok(page) = &result {
                    // Metadata is checked before icon reads so a malicious page cannot trigger unbounded I/O.
                    if page.nodes.len() > tree::MAX_PAGE
                        || serde_json::to_vec(page).map_or(true, |bytes| bytes.len() > 512 * 1024)
                    {
                        return Err(Failure::new(
                            ErrorCode::LimitExceeded,
                            "Tree page quota exceeded",
                        ));
                    }
                    for node in &page.nodes {
                        node.validate()?;
                        for command in node.activation.iter().chain(&node.menu) {
                            if !self.api.capabilities.contains_key("plugin.commands") {
                                return Err(Failure::new(
                                    ErrorCode::CapabilityUnavailable,
                                    "Node commands require plugin.commands",
                                ));
                            }
                            let signature = self
                                .plugin_services
                                .commands
                                .get(&command.command)
                                .ok_or_else(|| {
                                    Failure::new(
                                        ErrorCode::InvalidRequest,
                                        "Tree command must be declared with a typed signature",
                                    )
                                })?;
                            signature.parameters.accepts(&command.arguments)?;
                        }
                        if let Some(icon) = &node.icon {
                            self.prepare_icon(icon, &mut icons)?;
                        }
                    }
                }
                resource.reply(&request, result, icons)?;
                Ok(Value::Unit)
            }
        }
    }
    /// Handle resolution never accepts a different incarnation, scope or resource kind.
    pub(super) fn owned_tree(
        &self,
        handle: &api::ResourceHandle,
    ) -> Result<Arc<crate::TreeResource>, Failure> {
        if !matches!(self.roots.resolve(handle)?, RootKind::Tree) {
            return Err(Failure::new(
                ErrorCode::InvalidHandle,
                "Not a tree resource",
            ));
        }
        self.trees
            .get(&handle.resource)
            .filter(|resource| resource.is_live())
            .cloned()
            .ok_or_else(|| Failure::new(ErrorCode::InvalidHandle, "Tree was revoked"))
    }
}
impl Instance {
    /// Native revocation returns both the tree-specific and shared root quotas before another guest tick.
    pub(super) fn prune_trees(&mut self) -> anyhow::Result<bool> {
        let revoked = self
            .store
            .data()
            .trees
            .values()
            .filter(|resource| !resource.is_live())
            .map(|resource| resource.handle().clone())
            .collect::<Vec<_>>();
        let changed = !revoked.is_empty();
        for handle in revoked {
            self.store
                .data_mut()
                .resource_request(api::Operation::CloseResource { handle })?;
        }
        Ok(changed)
    }
    /// Native projections borrow opaque runtime resources; publication exposes no mutable worker state.
    pub fn tree_resources(&self) -> Vec<Arc<crate::TreeResource>> {
        self.store
            .data()
            .trees
            .values()
            .filter(|resource| resource.is_live())
            .cloned()
            .collect()
    }
    /// Validate the tree epoch before ordinary UI callbacks or typed native commands can run.
    pub(crate) fn validate_tree_event(&self, event: &tree::Event) -> Result<(), Failure> {
        self.store
            .data()
            .owned_tree(event.handle())?
            .validate_event(event)
    }
    pub(crate) fn tree_command(
        &self,
        tree: &api::ResourceHandle,
        epoch: u64,
        node: &str,
        command: &str,
    ) -> Result<tree::Command, Failure> {
        self.store
            .data()
            .owned_tree(tree)?
            .command(epoch, node, command)
    }
}
