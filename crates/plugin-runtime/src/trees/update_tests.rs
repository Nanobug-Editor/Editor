//! Metadata replacement must not invalidate native tree pages or captured page identities.
use super::*;

/// Menu-only artwork survives both display updates and structural patches, then retires with its row.
#[test]
fn nested_action_menu_artwork_survives_tree_updates_and_patches() {
    use plugin_protocol::ui::{ActionMenuItem, ToolIcon};
    let resource = TreeResource::new(
        ResourceHandle {
            instance: "owner".into(),
            scope: "scope".into(),
            resource: 1,
        },
        EditorAuthority::new(Arc::new(AtomicBool::new(true)), None),
        tree::Options::default(),
    );
    let mut node = tree::Node::new("branch", "Branch", true);
    let mut action = ActionMenuItem::action(
        "open",
        tree::Command {
            command: "open".into(),
            label: plugin_protocol::ui::LocalizedText::new("Open", "打开"),
            arguments: serde_json::Value::Null,
            disabled: false,
            separator_before: false,
        },
    );
    action.icon = Some(ToolIcon::new("menu.svg", "menu.svg"));
    node.action_menu.push(action);
    let request = resource.request(None, false).unwrap().unwrap();
    resource
        .reply(
            &request,
            Ok(tree::Page {
                nodes: vec![node.clone()],
                next: None,
            }),
            BTreeMap::from([(
                "menu.svg".into(),
                Arc::<[u8]>::from(b"owned-svg".as_slice()),
            )]),
        )
        .unwrap();
    node.label = "Renamed".into();
    resource.update(vec![node], BTreeMap::new()).unwrap();
    assert!(resource.snapshot().icons.contains_key("menu.svg"));
    resource
        .apply_patch(
            tree::Patch {
                nodes: Vec::new(),
                branches: vec![tree::BranchPatch {
                    parent: None,
                    children: vec!["branch".into()],
                    next: Some("next".into()),
                }],
            },
            BTreeMap::new(),
        )
        .unwrap();
    assert!(resource.snapshot().icons.contains_key("menu.svg"));
    resource
        .apply_patch(
            tree::Patch {
                nodes: Vec::new(),
                branches: vec![tree::BranchPatch {
                    parent: None,
                    children: Vec::new(),
                    next: None,
                }],
            },
            BTreeMap::new(),
        )
        .unwrap();
    assert!(resource.snapshot().icons.is_empty());
}

/// The public wire operation is additive and carries ordinary owned node metadata.
#[test]
fn cached_metadata_update_has_a_public_wire_operation() {
    let operation = serde_json::json!({
        "kind":"update",
        "tree":{"instance":"owner","scope":"scope","resource":1},
        "nodes":[tree::Node::new("branch", "Updated", true)]
    });
    assert!(
        serde_json::from_value::<tree::Operation>(operation).is_ok(),
        "cached metadata needs an operation that does not refresh provider pages"
    );
}

/// Keep a completed branch, its continuation and another pending branch alive simultaneously.
fn cached_tree() -> (TreeResource, tree::Node, tree::Request, tree::Request) {
    let resource = TreeResource::new(
        ResourceHandle {
            instance: "owner".into(),
            scope: "scope".into(),
            resource: 1,
        },
        EditorAuthority::new(Arc::new(AtomicBool::new(true)), None),
        tree::Options::default(),
    );
    let mut node = tree::Node::new("branch", "Branch", true);
    node.activation = Some(tree::Command {
        command: "open".into(),
        label: plugin_protocol::ui::LocalizedText {
            en: "Open".into(),
            zh_cn: "打开".into(),
        },
        arguments: serde_json::json!({"target":"branch"}),
        disabled: false,
        separator_before: false,
    });
    node.menu = vec![node.activation.clone().unwrap()];
    let root = resource.request(None, false).unwrap().unwrap();
    resource
        .reply(
            &root,
            Ok(tree::Page {
                nodes: vec![node.clone(), tree::Node::new("other", "Other", true)],
                next: None,
            }),
            Default::default(),
        )
        .unwrap();
    let child = resource
        .request(Some("branch".into()), false)
        .unwrap()
        .unwrap();
    resource
        .reply(
            &child,
            Ok(tree::Page {
                nodes: vec![tree::Node::new("leaf", "Loaded leaf", false)],
                next: Some("next".into()),
            }),
            Default::default(),
        )
        .unwrap();
    let continuation = resource
        .request(Some("branch".into()), true)
        .unwrap()
        .unwrap();
    let other = resource
        .request(Some("other".into()), false)
        .unwrap()
        .unwrap();
    (resource, node, continuation, other)
}

/// Check retained page state field by field, including pending/loading/error and continuation cursor.
fn assert_pages_unchanged(before: &TreeSnapshot, after: &TreeSnapshot) {
    assert_eq!(after.epoch, before.epoch);
    assert_eq!(after.expanded, before.expanded);
    assert_eq!(after.branches.len(), before.branches.len());
    for (parent, previous) in &before.branches {
        let current = &after.branches[parent];
        assert_eq!(current.children, previous.children);
        assert_eq!(current.next, previous.next);
        assert_eq!(current.loaded, previous.loaded);
        assert_eq!(current.loading, previous.loading);
        assert_eq!(current.error, previous.error);
    }
}

/// Cosmetic and disabled-state changes retain every accepted page and pending provider callback.
#[test]
fn cached_metadata_update_preserves_loaded_expanded_and_pending_pages() {
    let (resource, mut node, continuation, other) = cached_tree();
    let before = resource.snapshot();
    node.label = "Renamed branch".into();
    node.description = "Updated metadata".into();
    node.menu[0].disabled = true;
    node.menu[0].label.en = "Currently unavailable".into();
    resource
        .update(vec![node.clone()], Default::default())
        .unwrap();
    let after = resource.snapshot();
    assert_eq!(after.nodes["branch"], node);
    assert_eq!(after.nodes["leaf"], before.nodes["leaf"]);
    assert_eq!(after.revision, before.revision + 1);
    assert_pages_unchanged(&before, &after);
    for request in [continuation, other] {
        resource
            .validate_event(&tree::Event::Request(request.clone()))
            .unwrap();
        resource
            .reply(
                &request,
                Ok(tree::Page {
                    nodes: Vec::new(),
                    next: None,
                }),
                Default::default(),
            )
            .unwrap();
    }
}

/// Repeated metadata or provider IDs not cached by this projection must not trigger a native redraw.
#[test]
fn cached_metadata_update_noop_and_unknown_ids_preserve_revision() {
    let (resource, node, _, _) = cached_tree();
    let before = resource.snapshot();
    resource
        .update(
            vec![node, tree::Node::new("uncached", "Unknown", false)],
            Default::default(),
        )
        .unwrap();
    let after = resource.snapshot();
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.nodes, before.nodes);
    assert_pages_unchanged(&before, &after);
    resource.update(Vec::new(), Default::default()).unwrap();
    assert_eq!(resource.snapshot().revision, before.revision);
}

/// Captured native action slots resolve against current disabled state without requiring a new epoch.
#[test]
fn cached_metadata_update_rejects_newly_disabled_captured_actions() {
    let (resource, mut node, _, _) = cached_tree();
    let epoch = resource.snapshot().epoch;
    let action = tree::NodeAction::Menu { index: 0 };
    resource.command(epoch, "branch", &action).unwrap();
    node.menu[0].disabled = true;
    resource
        .update(vec![node.clone()], Default::default())
        .unwrap();
    assert_eq!(
        resource.command(epoch, "branch", &action).unwrap_err().code,
        ErrorCode::StaleRevision
    );
    resource
        .command(epoch, "branch", &tree::NodeAction::Activation)
        .unwrap();
    node.disabled = true;
    resource.update(vec![node], Default::default()).unwrap();
    assert_eq!(
        resource
            .command(epoch, "branch", &tree::NodeAction::Activation)
            .unwrap_err()
            .code,
        ErrorCode::StaleRevision
    );
}

/// An invalid node later in the batch cannot partially change an earlier valid node.
#[test]
fn cached_metadata_update_rejects_structure_and_command_intent_atomically() {
    let (resource, original, _, _) = cached_tree();
    let mut edits = Vec::new();
    let mut changed = original.clone();
    changed.branch = false;
    edits.push(changed);
    let mut changed = original.clone();
    changed.activation.as_mut().unwrap().arguments = serde_json::json!({"target":"other"});
    edits.push(changed);
    let mut changed = original.clone();
    changed.menu[0].command = "other".into();
    edits.push(changed);
    let mut changed = original.clone();
    changed.menu[0].separator_before = true;
    edits.push(changed);
    let mut changed = original.clone();
    changed.menu.clear();
    edits.push(changed);
    let mut changed = original.clone();
    changed.activation = None;
    edits.push(changed);
    let before = resource.snapshot();
    for changed in edits {
        let mut leaf = before.nodes["leaf"].clone();
        leaf.label = "Must not be partially published".into();
        assert_eq!(
            resource
                .update(vec![leaf, changed], Default::default())
                .unwrap_err()
                .code,
            ErrorCode::Conflict
        );
        let after = resource.snapshot();
        assert_eq!(after.nodes, before.nodes);
        assert_eq!(after.revision, before.revision);
        assert_pages_unchanged(&before, &after);
    }
}

/// Duplicate and invalid metadata checks apply even to uncached IDs; retirement seals mutation.
#[test]
fn cached_metadata_update_rejects_duplicates_invalid_metadata_and_retired_owner() {
    let (resource, node, _, _) = cached_tree();
    let before = resource.snapshot();
    assert_eq!(
        resource
            .update(vec![node.clone(), node], Default::default())
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    let mut invalid = tree::Node::new("unknown", "Unknown", false);
    invalid.label = "x".repeat(1025);
    assert_eq!(
        resource
            .update(vec![invalid], Default::default())
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );
    assert_eq!(resource.snapshot().revision, before.revision);
    resource.revoke();
    assert_eq!(
        resource
            .update(Vec::new(), Default::default())
            .unwrap_err()
            .code,
        ErrorCode::InvalidHandle
    );
}

/// Per-operation and retained-data quotas reject complete batches before changing accepted pages.
#[test]
fn cached_metadata_update_budgets_are_atomic() {
    let (resource, node, _, _) = cached_tree();
    let before = resource.snapshot();
    let oversized = (0..=tree::MAX_PAGE)
        .map(|index| tree::Node::new(format!("unknown-{index}"), "Unknown", false))
        .collect();
    assert_eq!(
        resource
            .update(oversized, Default::default())
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );
    let mut artwork_node = node;
    artwork_node.icon = Some(plugin_protocol::ui::ToolIcon {
        light: "icons/new.svg".into(),
        dark: "icons/new.svg".into(),
    });
    let artwork = [(
        "icons/new.svg".into(),
        Arc::<[u8]>::from(vec![0u8; 2 * 1024 * 1024 + 1]),
    )]
    .into();
    assert_eq!(
        resource
            .update(vec![artwork_node], artwork)
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );
    let after = resource.snapshot();
    assert_eq!(after.nodes, before.nodes);
    assert_eq!(after.revision, before.revision);
    assert_pages_unchanged(&before, &after);
}
