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
            tree::Operation::Create => self.allocate_tree(tree::Options::default()),
            tree::Operation::SetExpanded {
                tree,
                nodes,
                expanded,
            } => {
                self.validate_tree_checks()?;
                self.owned_tree(&tree)?.set_expanded(nodes, expanded)?;
                Ok(Value::Unit)
            }
            tree::Operation::CreateWithOptions { options } => {
                if options.single_click_expand {
                    self.validate_tree_action_menus()?;
                }
                if !self.tree_interaction_1_5
                    || !self
                        .api
                        .capabilities
                        .get("ui.tree")
                        .is_some_and(|version| version >= &semver::Version::new(1, 5, 0))
                {
                    return Err(Failure::new(
                        ErrorCode::CapabilityUnavailable,
                        "Tree interaction options require ui.tree >=1.5",
                    ));
                }
                self.allocate_tree(options)
            }
            tree::Operation::Refresh { tree } => {
                self.owned_tree(&tree)?.refresh()?;
                Ok(Value::Unit)
            }
            tree::Operation::Initialize { tree, path } => {
                self.validate_tree_presentation()?;
                self.owned_tree(&tree)?.initialize(path)?;
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
            tree::Operation::ApplyPatch { tree, patch } => {
                if !self.tree_patch_1_4
                    || !self
                        .api
                        .capabilities
                        .get("ui.tree")
                        .is_some_and(|v| v >= &semver::Version::new(1, 4, 0))
                {
                    return Err(Failure::new(
                        ErrorCode::CapabilityUnavailable,
                        "Structural tree patches require ui.tree >=1.4",
                    ));
                }
                let resource = self.owned_tree(&tree)?;
                patch.validate()?;
                let icons = self.prepare_tree_nodes(&patch.nodes, None)?;
                resource.apply_patch(patch, icons)?;
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
    /// Both creation operations use the same instance quota, delegated lifetime and owned root.
    /// `options` are fixed before publication; provider refreshes cannot alter pointer semantics.
    fn allocate_tree(&mut self, options: tree::Options) -> Result<Value, Failure> {
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
        // Source retirement releases the persistent root as well as sealing its native projection.
        if let Some(context) = &self.plugin_services.context {
            self.plugin_services
                .resources
                .insert(handle.resource, (handle.clone(), context.clone()));
        }
        self.trees.insert(
            handle.resource,
            Arc::new(crate::TreeResource::new(handle.clone(), authority, options)),
        );
        Ok(Value::Resource(handle))
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
            if node.checked.is_some() || !node.suffix.is_empty() {
                self.validate_tree_checks()?;
            }
            if !node.action_menu.is_empty() {
                self.validate_tree_action_menus()?;
                let mut rows = Vec::new();
                for item in &node.action_menu {
                    item.visit(&mut |item| rows.push(item));
                }
                self.prepare_action_menu(&rows, &mut icons)?;
            }
            if node.tone.is_some() {
                self.validate_tree_presentation()?;
            }
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
                // Structural deltas can span more than one page; stop at the retained artwork
                // budget instead of reading every path before discovering an oversized batch.
                if icons.values().map(|bytes| bytes.len()).sum::<usize>() > 2 * 1024 * 1024 {
                    return Err(Failure::new(
                        ErrorCode::LimitExceeded,
                        "Tree icon budget exceeded",
                    ));
                }
            }
        }
        Ok(icons)
    }
    /// Inventory alone must not silently opt older providers into new tree presentation semantics.
    fn validate_tree_presentation(&self) -> Result<(), Failure> {
        if !self.tree_presentation_1_3
            || !self
                .api
                .capabilities
                .get("ui.tree")
                .is_some_and(|version| version >= &semver::Version::new(1, 3, 0))
        {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Tree presentation requires ui.tree >=1.3",
            ));
        }
        Ok(())
    }
    /// New cascading menus and directory gestures require an explicit floor excluding all 1.5 patches.
    fn validate_tree_action_menus(&self) -> Result<(), Failure> {
        if !self.tree_action_menus_1_6
            || !self
                .api
                .capabilities
                .get("ui.tree")
                .is_some_and(|version| version >= &semver::Version::new(1, 6, 0))
        {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Action menus require explicit ui.tree >=1.6",
            ));
        }
        Ok(())
    }
    /// A current host cannot opt an older provider into checkbox or disclosure semantics.
    fn validate_tree_checks(&self) -> Result<(), Failure> {
        if !self.tree_checks_1_7
            || !self
                .api
                .capabilities
                .get("ui.tree")
                .is_some_and(|version| version >= &semver::Version::new(1, 7, 0))
        {
            return Err(Failure::new(
                ErrorCode::CapabilityUnavailable,
                "Tree checkboxes and disclosure require explicit ui.tree >=1.7",
            ));
        }
        Ok(())
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

/// Extended menus retain their explicit 1.1 minimum when a provider requires a newer tree minor.
pub(super) fn requires_menu_floor(requirements: Option<&api::Requirements>) -> bool {
    requires_tree_floor(requirements, 1)
}

/// Required ranges must exclude every patch of older minors, not merely 1.0.0 and 1.1.0.
/// SemVer requirements intersect their comparators, so a range admitting 1.2 and any older minor
/// also admits that minor's last representable patch; checking its tail rejects patch-only floors.
pub(super) fn requires_cached_update_floor(requirements: Option<&api::Requirements>) -> bool {
    requires_tree_floor(requirements, 2)
}

/// A required 1.3 range must exclude every patch of all older tree minors.
pub(super) fn requires_presentation_floor(requirements: Option<&api::Requirements>) -> bool {
    requires_tree_floor(requirements, 3)
}

/// Structural patches are additive; older required ranges cannot obtain them from new inventory.
pub(super) fn requires_patch_floor(requirements: Option<&api::Requirements>) -> bool {
    requires_tree_floor(requirements, 4)
}

/// Creation-time interaction preferences are available only to explicit 1.5 consumers.
pub(super) fn requires_interaction_floor(requirements: Option<&api::Requirements>) -> bool {
    requires_tree_floor(requirements, 5)
}

/// Cascading action menus are opt-in and cannot appear through an older tree requirement.
pub(super) fn requires_action_menu_floor(requirements: Option<&api::Requirements>) -> bool {
    requires_tree_floor(requirements, 6)
}

/// Checkable rows and programmatic disclosure are available only to explicit 1.7 consumers.
pub(super) fn requires_check_floor(requirements: Option<&api::Requirements>) -> bool {
    requires_tree_floor(requirements, 7)
}

/// Resolve each feature against the current inventory while rejecting every older minor tail.
fn requires_tree_floor(requirements: Option<&api::Requirements>, minor: u64) -> bool {
    requirements
        .and_then(|requirements| requirements.required.get("ui.tree"))
        .is_some_and(|range| {
            range.matches(&semver::Version::new(1, 7, 0))
                && (0..minor).all(|old| !range.matches(&semver::Version::new(1, old, u64::MAX)))
        })
}

#[cfg(test)]
mod update_tests {
    use super::*;

    /// New host inventory retains prior floors but cannot grant checkboxes to broad or optional ranges.
    #[test]
    fn tree_checkbox_floor_requires_explicit_minor() {
        for (range, expected) in [
            ("^1", false),
            (">=1.6,<2", false),
            (">=1.6.99,<2", false),
            (">=1.7,<2", true),
        ] {
            let requirements = api::Requirements {
                base: "^1".parse().unwrap(),
                required: [("ui.tree".into(), range.parse().unwrap())].into(),
                optional: Default::default(),
            };
            assert_eq!(
                requires_check_floor(Some(&requirements)),
                expected,
                "{range}"
            );
            if expected {
                assert!(requires_action_menu_floor(Some(&requirements)));
            }
        }
    }

    /// New explicit floors include older features; an inventory bump never widens old manifests.
    #[test]
    fn incremental_tree_patch_floor_preserves_prior_features_and_rejects_older_minors() {
        for (range, expected) in [
            ("^1", [false, false, false, false]),
            (">=1.2,<2", [true, true, false, false]),
            (">=1.3,<2", [true, true, true, false]),
            (">=1.3.99,<2", [true, true, true, false]),
            (">=1.4,<2", [true, true, true, true]),
        ] {
            let requirements = api::Requirements {
                base: "^1".parse().unwrap(),
                required: [("ui.tree".into(), range.parse().unwrap())].into(),
                optional: Default::default(),
            };
            assert_eq!(
                [
                    requires_menu_floor(Some(&requirements)),
                    requires_cached_update_floor(Some(&requirements)),
                    requires_presentation_floor(Some(&requirements)),
                    requires_patch_floor(Some(&requirements))
                ],
                expected,
                "{range}"
            );
        }
    }

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
    /// Any 1.0/1.1/1.2 patch admitted by the range forbids initial selection and semantic tones.
    #[test]
    fn initial_selection_requires_explicit_presentation_floor() {
        for (range, allowed) in [
            ("^1", false),
            (">=1.2,<2", false),
            (">=1.2.999,<2", false),
            (">=1.3,<2", true),
            ("^1.3", true),
        ] {
            let requirements = api::Requirements {
                base: "^1".parse().unwrap(),
                required: [("ui.tree".into(), range.parse().unwrap())].into(),
                optional: Default::default(),
            };
            assert_eq!(
                requires_presentation_floor(Some(&requirements)),
                allowed,
                "{range}"
            );
            if allowed {
                assert!(requires_cached_update_floor(Some(&requirements)));
            }
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
