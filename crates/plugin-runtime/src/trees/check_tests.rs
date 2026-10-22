//! Checkboxes remain provider-controlled and stale native presses never acquire a new target.
use super::*;

/// An arbitrary provider can publish checkboxes without declaring business commands.
fn checked_tree() -> (TreeResource, tree::Node) {
    let resource = TreeResource::new(
        ResourceHandle {
            instance: "checks".into(),
            scope: "workspace".into(),
            resource: 1,
        },
        EditorAuthority::new(Arc::new(AtomicBool::new(true)), None),
        tree::Options::default(),
    );
    let mut node = tree::Node::new("folder", "Folder", true);
    node.checked = Some(tree::CheckState::Mixed);
    node.suffix = "12 items".into();
    let request = resource.request(None, false).unwrap().unwrap();
    resource
        .reply(
            &request,
            Ok(tree::Page {
                nodes: vec![node.clone()],
                next: None,
            }),
            BTreeMap::new(),
        )
        .unwrap();
    (resource, node)
}

/// Checkbox admission checks ownership, epoch, displayed value and the requested transition.
#[test]
fn tree_checks_reject_stale_disabled_and_forged_intents() {
    let (resource, mut node) = checked_tree();
    let event = tree::Event::Checked {
        tree: resource.handle().clone(),
        epoch: resource.snapshot().epoch,
        node: node.id.clone(),
        previous: tree::CheckState::Mixed,
        checked: true,
    };
    resource.validate_event(&event).unwrap();
    assert_eq!(
        resource.snapshot().nodes[&node.id].checked,
        Some(tree::CheckState::Mixed)
    );
    let mut forged = event.clone();
    if let tree::Event::Checked { checked, .. } = &mut forged {
        *checked = false;
    }
    assert!(resource.validate_event(&forged).is_err());
    node.checked = Some(tree::CheckState::Checked);
    resource
        .update(vec![node.clone()], BTreeMap::new())
        .unwrap();
    assert!(resource.validate_event(&event).is_err());
    node.checked = Some(tree::CheckState::Mixed);
    node.disabled = true;
    resource.update(vec![node], BTreeMap::new()).unwrap();
    assert!(resource.validate_event(&event).is_err());
    resource.revoke();
    assert!(resource.validate_event(&event).is_err());
}

/// Disclosure retains cached pages and collapse withdraws pending replies instead of applying them late.
#[test]
fn tree_disclosure_keeps_pages_and_cancels_pending_descendants() {
    let (resource, _) = checked_tree();
    let before = resource.snapshot();
    assert!(
        resource
            .set_expanded(vec!["folder".into(), "@reserved".into()], true)
            .is_err()
    );
    assert_eq!(resource.snapshot().expanded, before.expanded);
    resource.set_expanded(vec!["folder".into()], true).unwrap();
    let pending = resource
        .request(Some("folder".into()), false)
        .unwrap()
        .unwrap();
    resource.set_expanded(vec!["folder".into()], false).unwrap();
    assert!(resource.snapshot().expanded.is_empty());
    assert_eq!(resource.snapshot().nodes, before.nodes);
    assert!(
        resource
            .reply(
                &pending,
                Ok(tree::Page {
                    nodes: Vec::new(),
                    next: None
                }),
                BTreeMap::new()
            )
            .is_err()
    );
}
