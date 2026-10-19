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
            tree::Operation::Update { tree, nodes } => {
                validate_update_capability(&self.api, self.tree_update_1_2)?;
                let resource = self.owned_tree(&tree)?;
                if nodes.len() > tree::MAX_PAGE
                    || serde_json::to_vec(&nodes).map_or(true, |bytes| bytes.len() > 512 * 1024)
                {
                    return Err(Failure::new(
                        ErrorCode::LimitExceeded,
                        "Tree update quota exceeded",
                    ));
                }
                let cached = resource.cached_ids();
                let icons = self.prepare_tree_nodes(&nodes, Some(&cached))?;
                resource.update(nodes, icons)?;
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
                    icons = self.prepare_tree_nodes(&page.nodes, None)?;
                }
                resource.reply(&request, result, icons)?;
                Ok(Value::Unit)
            }
        }
    }
    /// Pages and cached updates share typed-command, metadata and owned SVG admission before mutation.
    fn prepare_tree_nodes(
        &self,
        nodes: &[tree::Node],
        cached: Option<&BTreeSet<String>>,
    ) -> Result<BTreeMap<String, Arc<[u8]>>, Failure> {
        let mut icons = BTreeMap::new();
        for node in nodes {
            node.validate()?;
            if (node.menu.len() > 8
                || node
                    .activation
                    .iter()
                    .chain(&node.menu)
                    .any(|command| command.disabled || command.separator_before))
                && !self.tree_menu_1_1
            {
                return Err(Failure::new(
                    ErrorCode::CapabilityUnavailable,
                    "Extended tree menus require ui.tree >=1.1",
                ));
            }
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
                if cached.is_none_or(|ids| ids.contains(&node.id)) {
                    self.prepare_icon(icon, &mut icons)?;
                } else {
                    // A provider cannot know which lazy pages the host retained. Ignore their
                    // artwork after validating authority, without opening or parsing SVG bytes.
                    self.owned_icon_paths(icon)?;
                }
            }
        }
        Ok(icons)
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

/// A newer host inventory never opts a 1.0/1.1 provider into cached metadata mutation.
fn validate_update_capability(api: &api::Negotiated, explicit_floor: bool) -> Result<(), Failure> {
    if !explicit_floor
        || !api
            .capabilities
            .get("ui.tree")
            .is_some_and(|version| version >= &semver::Version::new(1, 2, 0))
    {
        return Err(Failure::new(
            ErrorCode::CapabilityUnavailable,
            "Cached tree updates require ui.tree >=1.2",
        ));
    }
    Ok(())
}

/// Required ranges must exclude every patch of older minors, not merely 1.0.0 and 1.1.0.
/// SemVer requirements intersect their comparators, so a range admitting 1.2 and any older minor
/// also admits that minor's last representable patch; checking its tail rejects patch-only floors.
pub(super) fn requires_cached_update_floor(requirements: Option<&api::Requirements>) -> bool {
    requirements
        .and_then(|requirements| requirements.required.get("ui.tree"))
        .is_some_and(|range| {
            range.matches(&semver::Version::new(1, 2, 0))
                && !range.matches(&semver::Version::new(1, 1, u64::MAX))
                && !range.matches(&semver::Version::new(1, 0, u64::MAX))
        })
}

#[cfg(test)]
mod update_tests {
    use super::*;

    /// Negotiating the current inventory alone must not authorize new mutation semantics.
    #[test]
    fn cached_metadata_update_requires_explicit_floor_and_negotiated_version() {
        for (version, explicit_floor, allowed) in [
            (semver::Version::new(1, 0, 0), true, false),
            (semver::Version::new(1, 1, 0), true, false),
            (semver::Version::new(1, 2, 0), false, false),
            (semver::Version::new(1, 2, 0), true, true),
        ] {
            let negotiated = api::Negotiated {
                base: semver::Version::new(1, 0, 0),
                capabilities: [("ui.tree".into(), version)].into(),
            };
            let result = validate_update_capability(&negotiated, explicit_floor);
            assert_eq!(result.is_ok(), allowed);
            if !allowed {
                assert_eq!(result.unwrap_err().code, ErrorCode::CapabilityUnavailable);
            }
        }
    }

    /// New operation semantics cannot be published under either an old minor or a patch-only floor.
    #[test]
    fn cached_metadata_update_floor_excludes_all_older_minor_patches() {
        assert!(!requires_cached_update_floor(None));
        for (range, allowed) in [
            ("^1", false),
            (">=1.0.5,<2", false),
            (">=1.1,<2", false),
            (">=1.1.5,<2", false),
            ("^1.1.99", false),
            (">=1.2,<2", true),
            ("^1.2", true),
        ] {
            let requirements = api::Requirements {
                base: "^1".parse().unwrap(),
                required: [("ui.tree".into(), range.parse().unwrap())].into(),
                optional: Default::default(),
            };
            assert_eq!(
                requires_cached_update_floor(Some(&requirements)),
                allowed,
                "{range}"
            );
        }
    }
}

#[cfg(test)]
#[path = "trees/admission_tests.rs"]
mod admission_tests;
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
    /// A denied scene callback may release only an exact host request owned by this live instance.
    /// Failure is already returned by admission; an obsolete or forged handle changes no resource.
    pub(crate) fn release_undelivered_tree_page(&self, request: &tree::Request) {
        if let Ok(resource) = self.store.data().owned_tree(&request.tree) {
            resource.release_undelivered(request);
        }
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
        action: &tree::NodeAction,
    ) -> Result<tree::Command, Failure> {
        self.store
            .data()
            .owned_tree(tree)?
            .command(epoch, node, action)
    }
}
