//! Atomic cached-forest reconciliation; provider deltas never require an empty native projection.
use super::*;

impl TreeResource {
    /// Validate the complete candidate before replacing the snapshot. A changed forest seals old
    /// actions/replies while preserving live cached pages and stable native presentation identities.
    pub(crate) fn apply_patch(
        &self,
        patch: tree::Patch,
        icons: BTreeMap<String, Arc<[u8]>>,
    ) -> Result<(), Failure> {
        self.live()?;
        patch.validate()?;
        let mut state = self.state.lock().unwrap();
        let mut candidate = state.snapshot.clone();
        let supplied = patch
            .nodes
            .iter()
            .map(|n| n.id.clone())
            .collect::<BTreeSet<_>>();
        for node in patch.nodes {
            if candidate
                .nodes
                .get(&node.id)
                .is_some_and(|old| old.branch != node.branch)
            {
                return Err(Failure::new(
                    ErrorCode::Conflict,
                    "Tree patch cannot reuse an ID for another node kind",
                ));
            }
            if let Some(previous) = candidate.nodes.get(&node.id) {
                validate_update_intent(previous, &node)?;
            }
            if node.branch {
                candidate.branches.entry(Some(node.id.clone())).or_default();
            }
            candidate.nodes.insert(node.id.clone(), node);
        }
        for branch in patch.branches {
            if branch
                .parent
                .as_ref()
                .is_some_and(|id| candidate.nodes.get(id).is_none_or(|n| !n.branch))
            {
                return Err(stale());
            }
            candidate.branches.insert(
                branch.parent,
                TreeBranch {
                    children: branch.children,
                    next: branch.next,
                    loaded: true,
                    loading: false,
                    error: None,
                },
            );
        }
        let mut reachable = BTreeSet::new();
        visit(&candidate, None, 0, &mut reachable)?;
        if !supplied.is_subset(&reachable) {
            return Err(Failure::new(
                ErrorCode::Conflict,
                "Tree patch supplied unreachable nodes",
            ));
        }
        candidate.nodes.retain(|id, _| reachable.contains(id));
        candidate
            .branches
            .retain(|parent, _| parent.as_ref().is_none_or(|id| reachable.contains(id)));
        candidate.expanded.retain(|id| reachable.contains(id));
        let paths = artwork_paths(candidate.nodes.values());
        candidate
            .icons
            .extend(icons.into_iter().filter(|(path, _)| paths.contains(path)));
        candidate.icons.retain(|path, _| paths.contains(path));
        if candidate.nodes.len() > tree::MAX_NODES
            || candidate
                .nodes
                .values()
                .map(|n| serde_json::to_vec(n).map_or(usize::MAX, |b| b.len()))
                .fold(0usize, usize::saturating_add)
                > 2 * 1024 * 1024
            || candidate.icons.values().map(|b| b.len()).sum::<usize>() > 2 * 1024 * 1024
        {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Tree retained metadata or icon budget exceeded",
            ));
        }
        if candidate.nodes == state.snapshot.nodes && candidate.branches == state.snapshot.branches
        {
            return Ok(());
        }
        // Every published action/pending page belongs to the preceding incarnation. Keep pages
        // and selection geometry, but make their old credentials unusable before publication.
        candidate.epoch = candidate
            .epoch
            .checked_add(1)
            .ok_or_else(|| Failure::new(ErrorCode::LimitExceeded, "Tree epoch exhausted"))?;
        candidate.revision = candidate.revision.saturating_add(1);
        for branch in candidate.branches.values_mut() {
            branch.loading = false;
        }
        state.pending.clear();
        if state.explicit_reveal {
            candidate.initial_selection = None;
            state.explicit_reveal = false;
        }
        state.snapshot = candidate;
        Ok(())
    }
}

/// Walk the bounded cached forest to reject dangling children, multiple parents and cycles.
fn visit(
    snapshot: &TreeSnapshot,
    parent: Option<&str>,
    depth: usize,
    seen: &mut BTreeSet<String>,
) -> Result<(), Failure> {
    if depth > 64 {
        return Err(Failure::new(
            ErrorCode::LimitExceeded,
            "Tree patch depth exceeded",
        ));
    }
    if let Some(branch) = snapshot.branches.get(&parent.map(str::to_owned)) {
        if depth >= 64 && !branch.children.is_empty() {
            return Err(Failure::new(
                ErrorCode::LimitExceeded,
                "Tree patch depth exceeded",
            ));
        }
        for id in &branch.children {
            let node = snapshot.nodes.get(id).ok_or_else(|| {
                Failure::new(ErrorCode::Conflict, "Tree patch has a missing child")
            })?;
            if !seen.insert(id.clone()) {
                return Err(Failure::new(
                    ErrorCode::Conflict,
                    "Tree patch contains a cycle or duplicate parent",
                ));
            }
            if node.branch {
                visit(snapshot, Some(id), depth + 1, seen)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Load two sibling folders and their files through ordinary lazy replies.
    fn fixture() -> TreeResource {
        let resource = TreeResource::new(
            ResourceHandle {
                instance: "owner".into(),
                scope: "workspace".into(),
                resource: 1,
            },
            EditorAuthority::new(Arc::new(AtomicBool::new(true)), None),
            tree::Options::default(),
        );
        let root = resource.request(None, false).unwrap().unwrap();
        resource
            .reply(
                &root,
                Ok(tree::Page {
                    nodes: vec![
                        tree::Node::new("src", "src", true),
                        tree::Node::new("docs", "docs", true),
                    ],
                    next: None,
                }),
                BTreeMap::new(),
            )
            .unwrap();
        for parent in ["src", "docs"] {
            let request = resource
                .request(Some(parent.into()), false)
                .unwrap()
                .unwrap();
            resource
                .reply(
                    &request,
                    Ok(tree::Page {
                        nodes: vec![tree::Node::new(format!("{parent}-old"), "old", false)],
                        next: None,
                    }),
                    BTreeMap::new(),
                )
                .unwrap();
        }
        resource
    }

    /// Added/deleted children change one complete branch without discarding any sibling cache.
    #[test]
    fn incremental_tree_patch_reconciles_only_changed_branch_and_seals_old_callbacks() {
        let resource = fixture();
        let before = resource.snapshot();
        let patch = tree::Patch {
            nodes: vec![tree::Node::new("src-new", "new", false)],
            branches: vec![tree::BranchPatch {
                parent: Some("src".into()),
                children: vec!["src-new".into()],
                next: Some("next-page".into()),
            }],
        };
        resource.apply_patch(patch, BTreeMap::new()).unwrap();
        let after = resource.snapshot();
        assert_eq!(after.branches[&None], before.branches[&None]);
        assert_eq!(
            after.branches[&Some("docs".into())],
            before.branches[&Some("docs".into())]
        );
        assert!(after.nodes.contains_key("docs-old") && !after.nodes.contains_key("src-old"));
        assert_eq!(after.expanded, before.expanded);
        assert_eq!(after.branches[&Some("src".into())].children, ["src-new"]);
        assert_eq!(
            after.branches[&Some("src".into())].next.as_deref(),
            Some("next-page")
        );
        assert_eq!(
            resource
                .validate_event(&tree::Event::Selected {
                    tree: resource.handle().clone(),
                    epoch: before.epoch,
                    node: "docs-old".into()
                })
                .unwrap_err()
                .code,
            ErrorCode::StaleRevision
        );
    }

    /// A no-op delta does not cancel an unrelated lazy request or rotate its source credentials.
    #[test]
    fn incremental_tree_patch_noop_keeps_epoch_and_pending_reply() {
        let resource = fixture();
        resource
            .apply_patch(
                tree::Patch {
                    nodes: Vec::new(),
                    branches: vec![tree::BranchPatch {
                        parent: Some("docs".into()),
                        children: vec!["docs-old".into()],
                        next: Some("next".into()),
                    }],
                },
                BTreeMap::new(),
            )
            .unwrap();
        let pending = resource
            .request(Some("docs".into()), true)
            .unwrap()
            .unwrap();
        let before = resource.snapshot();
        resource
            .apply_patch(tree::Patch::default(), BTreeMap::new())
            .unwrap();
        assert_eq!(resource.snapshot().revision, before.revision);
        assert_eq!(resource.snapshot().epoch, before.epoch);
        resource
            .reply(
                &pending,
                Ok(tree::Page {
                    nodes: vec![tree::Node::new("docs-next", "next", false)],
                    next: None,
                }),
                BTreeMap::new(),
            )
            .unwrap();
    }

    /// Malformed graphs/budgets fail atomically, retaining the complete accepted tree.
    #[test]
    fn incremental_tree_patch_rejects_invalid_forests_without_mutation() {
        for children in [vec!["missing"], vec!["docs-old", "docs-old"], vec!["src"]] {
            let resource = fixture();
            let before = resource.snapshot();
            let patch = tree::Patch {
                nodes: Vec::new(),
                branches: vec![tree::BranchPatch {
                    parent: Some("src".into()),
                    children: children.into_iter().map(str::to_owned).collect(),
                    next: None,
                }],
            };
            assert!(resource.apply_patch(patch, BTreeMap::new()).is_err());
            let after = resource.snapshot();
            assert_eq!(after.nodes, before.nodes);
            assert_eq!(after.branches, before.branches);
            assert_eq!(after.epoch, before.epoch);
        }
    }

    /// Existing IDs cannot acquire different command intent, and oversized deltas cannot mutate data.
    #[test]
    fn incremental_tree_patch_keeps_command_identity_and_budgets() {
        let resource = fixture();
        let before = resource.snapshot();
        let mut node = before.nodes["docs-old"].clone();
        node.activation = Some(tree::Command {
            command: "new-command".into(),
            label: plugin_protocol::ui::LocalizedText::new("New", "新命令"),
            arguments: serde_json::Value::Null,
            disabled: false,
            separator_before: false,
        });
        assert_eq!(
            resource
                .apply_patch(
                    tree::Patch {
                        nodes: vec![node],
                        branches: Vec::new()
                    },
                    BTreeMap::new()
                )
                .unwrap_err()
                .code,
            ErrorCode::Conflict
        );
        assert_eq!(
            resource
                .apply_patch(
                    tree::Patch {
                        nodes: Vec::new(),
                        branches: vec![tree::BranchPatch {
                            parent: None,
                            children: vec!["src".into(); tree::MAX_NODES + 1],
                            next: None
                        }]
                    },
                    BTreeMap::new()
                )
                .unwrap_err()
                .code,
            ErrorCode::LimitExceeded
        );
        assert_eq!(resource.snapshot().nodes, before.nodes);
        assert_eq!(resource.snapshot().epoch, before.epoch);
    }
}
