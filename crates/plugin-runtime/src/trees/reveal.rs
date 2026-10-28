//! Explicit provider navigation uses only the current cached forest and native selection seam.
use super::*;

impl TreeResource {
    /// Validate all ancestors before changing disclosure; no lazy request or business input is made.
    pub(crate) fn reveal(&self, path: Vec<String>) -> Result<(), Failure> {
        self.live()?;
        if path.is_empty()
            || path.len() > 64
            || path.iter().any(|id| !tree::valid_id(id))
            || path.iter().collect::<BTreeSet<_>>().len() != path.len()
        {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid tree reveal path",
            ));
        }
        let mut state = self.state.lock().unwrap();
        let mut parent = None;
        for (index, id) in path.iter().enumerate() {
            let valid =
                state
                    .snapshot
                    .branches
                    .get(&parent)
                    .is_some_and(|branch| branch.loaded && branch.children.contains(id))
                    && state.snapshot.nodes.get(id).is_some_and(|node| {
                        !node.disabled && (index + 1 == path.len() || node.branch)
                    });
            if !valid {
                return Err(stale());
            }
            parent = Some(id.clone());
        }
        state
            .snapshot
            .expanded
            .extend(path[..path.len() - 1].iter().cloned());
        state.initialized = true;
        state.explicit_reveal = true;
        state.snapshot.initial_selection = Some(path);
        state.snapshot.revision = state.snapshot.revision.saturating_add(1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A complete provider forest does not need page callbacks even for collapsed nested folders.
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
        resource
            .apply_patch(
                tree::Patch {
                    nodes: vec![
                        tree::Node::new("src", "src", true),
                        tree::Node::new("a", "a.rs", false),
                        tree::Node::new("b", "b.rs", false),
                    ],
                    branches: vec![
                        tree::BranchPatch {
                            parent: None,
                            children: vec!["src".into()],
                            next: None,
                        },
                        tree::BranchPatch {
                            parent: Some("src".into()),
                            children: vec!["a".into(), "b".into()],
                            next: None,
                        },
                    ],
                },
                BTreeMap::new(),
            )
            .unwrap();
        resource
    }

    #[test]
    fn explicit_tree_reveal_survives_prior_user_selection_and_keeps_pages() {
        let resource = fixture();
        resource.dismiss_initial_selection();
        let before = resource.snapshot();
        resource.reveal(vec!["src".into(), "b".into()]).unwrap();
        let after = resource.snapshot();
        assert!(after.expanded.contains("src"));
        assert_eq!(
            after.initial_selection,
            Some(vec!["src".into(), "b".into()])
        );
        assert_eq!(before.epoch, after.epoch);
        assert_eq!(before.nodes, after.nodes);
        assert_eq!(before.branches, after.branches);
        resource.dismiss_initial_selection();
        resource.initialize(vec!["src".into(), "a".into()]).unwrap();
        assert!(resource.snapshot().initial_selection.is_none());
        resource.reveal(vec!["src".into(), "a".into()]).unwrap();
        assert_eq!(resource.snapshot().initial_selection.unwrap()[1], "a");
        assert!(
            resource
                .request(Some("src".into()), false)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn explicit_tree_reveal_rejects_disconnected_paths_and_retired_owners_atomically() {
        let resource = fixture();
        let before = resource.snapshot();
        for path in [
            vec!["b".into()],
            vec!["src".into(), "missing".into()],
            vec!["src".into(), "a".into(), "b".into()],
        ] {
            assert!(resource.reveal(path).is_err());
            assert_eq!(before.revision, resource.snapshot().revision);
            assert_eq!(before.expanded, resource.snapshot().expanded);
        }
        resource.revoke();
        assert!(resource.reveal(vec!["src".into(), "a".into()]).is_err());
    }

    /// A later commit/forest cannot inherit navigation requested for its predecessor.
    #[test]
    fn explicit_tree_reveal_is_cancelled_by_changed_forest_but_not_noop_patch() {
        let resource = fixture();
        resource.reveal(vec!["src".into(), "a".into()]).unwrap();
        resource
            .apply_patch(tree::Patch::default(), BTreeMap::new())
            .unwrap();
        assert!(resource.snapshot().initial_selection.is_some());
        resource
            .apply_patch(
                tree::Patch {
                    nodes: vec![tree::Node::new("b", "renamed.rs", false)],
                    branches: Vec::new(),
                },
                BTreeMap::new(),
            )
            .unwrap();
        assert!(resource.snapshot().initial_selection.is_none());
        resource.reveal(vec!["src".into(), "b".into()]).unwrap();
        resource.refresh().unwrap();
        assert!(resource.snapshot().initial_selection.is_none());
    }
}
