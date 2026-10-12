//! Real independent database package proves paging, sealed callbacks and retirement through Manager.
/// Immutable SDK inputs bind exact versions and hashes, independently of generated target outputs.
#[path = "support/packages.rs"]
mod frozen_packages;
use plugin_runtime::{
    Manager, Package,
    plugin_protocol::{Environment, api, tree, ui},
};
use std::io::{Cursor, Write};
use std::sync::Arc;

#[path = "community_presentation/updates.rs"]
mod updates;

/// Inspect the independently compiled package, keeping its actual component and owned resources.
fn provider(id: &str) -> Package {
    frozen_packages::package(&format!("{id}-0.1.1"))
}

/// The ordinary developer command serializes SDK results in its visible status, without a host test API.
fn probe(
    manager: &mut Manager,
    id: &str,
    operation: api::Operation,
) -> Result<api::Value, api::Failure> {
    manager
        .invoke_command(id, "probe", serde_json::to_value(operation).unwrap())
        .unwrap();
    let mut receipt = None;
    manager.live[id].views["presentation"]
        .root
        .visit(&mut |node| {
            if let ui::Kind::Text { text } = &node.kind {
                if let Ok(result) = serde_json::from_str::<Result<api::Value, api::Failure>>(text) {
                    receipt = Some(result);
                }
            }
        });
    receipt.expect("a real SDK result must reach the public scene")
}

/// Change only negotiation or grants; this remains the same real domain consumer and WASM artifact.
fn repack(original: &Package, manifest: plugin_runtime::plugin_protocol::Manifest) -> Package {
    let mut files = original.files.clone();
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        archive
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(&bytes).unwrap();
    }
    Package::from_bytes(&archive.finish().unwrap().into_inner()).unwrap()
}
fn event(manager: &mut Manager, action: tree::Event) -> anyhow::Result<()> {
    manager.event(
        "database-tree",
        Some("presentation".into()),
        api::Notification::Ui(ui::UiEvent {
            revision: 1,
            node: "catalog-tree".into(),
            action: ui::Action::Tree(action),
        }),
    )
}
fn load(
    manager: &mut Manager,
    tree: &Arc<plugin_runtime::TreeResource>,
    parent: Option<&str>,
    continuation: bool,
) -> tree::Request {
    let request = tree
        .request(parent.map(str::to_string), continuation)
        .unwrap()
        .unwrap();
    event(manager, tree::Event::Request(request.clone())).unwrap();
    request
}
/// One real consumer publishes independent pages and cannot resurrect folded or refreshed child work.
#[test]
#[ignore = "opt-in fixed SDK/WASM packages; rebuild sources: crates/plugin-runtime/tests/fixtures/community; see fixtures/packages/README.md"]
fn community_database_tree_real_pages_retry_late_callbacks_and_retirement() {
    let runtime = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let package = frozen_packages::package("database-tree-0.1.1");
    let mut manager = Manager::open(
        runtime.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let resource = manager.live["database-tree"]
        .tree_resources()
        .pop()
        .unwrap();
    load(&mut manager, &resource, None, false);
    load(&mut manager, &resource, Some("main"), false);
    load(&mut manager, &resource, Some("users"), false);
    assert_eq!(
        resource.snapshot().branches[&Some("users".into())]
            .children
            .len(),
        16
    );
    load(&mut manager, &resource, Some("users"), true);
    let snapshot = resource.snapshot();
    assert_eq!(snapshot.branches[&Some("users".into())].children.len(), 32);
    assert_eq!(snapshot.nodes["user-31"].label, "User 031");
    let epoch = snapshot.epoch;
    event(
        &mut manager,
        tree::Event::Invoke {
            tree: resource.handle().clone(),
            epoch,
            node: "user-31".into(),
            action: tree::NodeAction::Activation,
        },
    )
    .unwrap();
    // Typed native commands are queued by the public manager and delivered on its ordinary poll.
    manager.poll();
    let scene = &manager.live["database-tree"].views["presentation"];
    let mut found = false;
    scene.root.visit(&mut |node| {
        if let ui::Kind::Text { text } = &node.kind {
            found |= text.contains("user-31");
        }
    });
    assert!(found);
    load(&mut manager, &resource, Some("fault-once"), false);
    assert!(
        resource.snapshot().branches[&Some("fault-once".into())]
            .error
            .is_some()
    );
    load(&mut manager, &resource, Some("fault-once"), false);
    assert_eq!(resource.snapshot().nodes["recovered"].label, "已恢复");
    // The real package legally reuses one signature for different action arguments. Admission
    // resolves the exact owned node slot, rather than picking the first matching command name.
    for (action, expected) in [
        (tree::NodeAction::Activation, "recovered:summary"),
        (tree::NodeAction::Menu { index: 0 }, "recovered:details"),
        (tree::NodeAction::Menu { index: 1 }, "recovered:schema"),
    ] {
        event(
            &mut manager,
            tree::Event::Invoke {
                tree: resource.handle().clone(),
                epoch,
                node: "recovered".into(),
                action,
            },
        )
        .unwrap();
        manager.poll();
        let mut delivered = false;
        manager.live["database-tree"].views["presentation"]
            .root
            .visit(&mut |node| {
                if let ui::Kind::Text { text } = &node.kind {
                    delivered |= text.ends_with(expected);
                }
            });
        assert!(delivered, "the exact node action must deliver {expected}");
    }
    let error = event(
        &mut manager,
        tree::Event::Invoke {
            tree: resource.handle().clone(),
            epoch,
            node: "recovered".into(),
            action: tree::NodeAction::Menu { index: 2 },
        },
    )
    .unwrap_err();
    assert_eq!(
        error.downcast_ref::<api::Failure>().unwrap().code,
        api::ErrorCode::StaleRevision
    );
    load(&mut manager, &resource, Some("slow"), false);
    resource.collapse("slow");
    manager
        .invoke_command("database-tree", "complete-pending", serde_json::Value::Null)
        .unwrap();
    assert!(!resource.snapshot().nodes.contains_key("delayed-child"));
    load(&mut manager, &resource, Some("slow"), false);
    manager
        .invoke_command("database-tree", "refresh-database", serde_json::Value::Null)
        .unwrap();
    manager
        .invoke_command("database-tree", "complete-pending", serde_json::Value::Null)
        .unwrap();
    assert!(!resource.snapshot().nodes.contains_key("delayed-child"));
    assert!(
        event(
            &mut manager,
            tree::Event::Invoke {
                tree: resource.handle().clone(),
                epoch,
                node: "user-31".into(),
                action: tree::NodeAction::Activation
            }
        )
        .is_err()
    );
    manager.disable("database-tree").unwrap();
    assert!(!resource.is_live());
    assert!(resource.request(None, false).is_err());
}

/// Actual SDK calls preserve owner isolation and bounded atomic pages; optional absence stays observable.
#[test]
#[ignore = "opt-in fixed SDK/WASM packages; rebuild sources: crates/plugin-runtime/tests/fixtures/community; see fixtures/packages/README.md"]
fn community_trees_enforce_negotiation_ownership_and_bounded_atomic_pages() {
    let runtime = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let environment = Environment {
        workspace: workspace.path().display().to_string(),
        ..Default::default()
    };
    let original = provider("database-tree");
    let mut manager = Manager::open(runtime.path().into(), environment.clone()).unwrap();
    manager
        .install(&original, original.manifest.permissions.clone())
        .unwrap();
    let api::Value::Resource(handle) = probe(
        &mut manager,
        "database-tree",
        api::Operation::Tree {
            operation: tree::Operation::Create,
        },
    )
    .unwrap() else {
        panic!("tree allocation expected")
    };
    let resource = manager.live["database-tree"]
        .tree_resources()
        .into_iter()
        .find(|resource| resource.handle() == &handle)
        .unwrap();
    let request = resource.request(None, false).unwrap().unwrap();
    let reply = |request, nodes, next| api::Operation::Tree {
        operation: tree::Operation::Reply {
            request,
            result: Ok(tree::Page { nodes, next }),
        },
    };
    let oversized = (0..129)
        .map(|index| tree::Node::new(format!("node-{index}"), "Bounded node", false))
        .collect();
    assert_eq!(
        probe(
            &mut manager,
            "database-tree",
            reply(request.clone(), oversized, None)
        )
        .unwrap_err()
        .code,
        api::ErrorCode::LimitExceeded
    );
    assert!(
        resource.snapshot().nodes.is_empty(),
        "rejected pages publish no partial metadata"
    );
    let mut forged = request.clone();
    forged.sequence += 1;
    assert_eq!(
        probe(
            &mut manager,
            "database-tree",
            reply(forged, Vec::new(), None)
        )
        .unwrap_err()
        .code,
        api::ErrorCode::StaleRevision
    );
    probe(
        &mut manager,
        "database-tree",
        reply(
            request.clone(),
            vec![tree::Node::new("root", "Current root", true)],
            Some("next".into()),
        ),
    )
    .unwrap();
    assert_eq!(
        probe(
            &mut manager,
            "database-tree",
            reply(request, Vec::new(), None)
        )
        .unwrap_err()
        .code,
        api::ErrorCode::StaleRevision
    );
    let continuation = resource.request(None, true).unwrap().unwrap();
    assert_eq!(
        probe(
            &mut manager,
            "database-tree",
            reply(
                continuation.clone(),
                vec![tree::Node::new("root", "Duplicate root", false)],
                None
            )
        )
        .unwrap_err()
        .code,
        api::ErrorCode::Conflict
    );
    assert_eq!(resource.snapshot().nodes["root"].label, "Current root");
    probe(
        &mut manager,
        "database-tree",
        reply(
            continuation,
            vec![tree::Node::new("second", "Next node", false)],
            None,
        ),
    )
    .unwrap();
    assert_eq!(resource.snapshot().nodes.len(), 2);

    let other = provider("todo-tree");
    manager
        .install(&other, other.manifest.permissions.clone())
        .unwrap();
    assert_eq!(
        probe(
            &mut manager,
            "todo-tree",
            api::Operation::Tree {
                operation: tree::Operation::Refresh {
                    tree: handle.clone()
                }
            }
        )
        .unwrap_err()
        .code,
        api::ErrorCode::InvalidHandle
    );
    for _ in 0..6 {
        probe(
            &mut manager,
            "database-tree",
            api::Operation::Tree {
                operation: tree::Operation::Create,
            },
        )
        .unwrap();
    }
    assert_eq!(
        probe(
            &mut manager,
            "database-tree",
            api::Operation::Tree {
                operation: tree::Operation::Create
            }
        )
        .unwrap_err()
        .code,
        api::ErrorCode::LimitExceeded
    );
    probe(
        &mut manager,
        "database-tree",
        api::Operation::CloseResource {
            handle: handle.clone(),
        },
    )
    .unwrap();
    assert!(!resource.is_live());
    assert_eq!(
        probe(
            &mut manager,
            "database-tree",
            api::Operation::Tree {
                operation: tree::Operation::Refresh { tree: handle }
            }
        )
        .unwrap_err()
        .code,
        api::ErrorCode::InvalidHandle
    );
    probe(
        &mut manager,
        "database-tree",
        api::Operation::Tree {
            operation: tree::Operation::Create,
        },
    )
    .unwrap();
    let surviving = manager.live["todo-tree"].tree_resources().pop().unwrap();
    let retired = manager.live["database-tree"].tree_resources();
    manager.uninstall("database-tree", false).unwrap();
    assert!(retired.iter().all(|resource| !resource.is_live()));
    assert!(
        surviving.is_live(),
        "one provider cannot retire another provider's tree"
    );

    for (permission, expected) in [
        (true, api::ErrorCode::PermissionDenied),
        (false, api::ErrorCode::CapabilityUnavailable),
    ] {
        let mut manifest = original.manifest.clone();
        if permission {
            manifest.permissions.remove("ui.panels");
        } else {
            let api = manifest.api.as_mut().unwrap();
            api.required.remove("ui.tree");
            api.optional.insert("ui.tree".into(), "^9".parse().unwrap());
        }
        let package = repack(&original, manifest);
        let isolated = tempfile::tempdir().unwrap();
        let mut denied = Manager::open(isolated.path().into(), environment.clone()).unwrap();
        denied
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        assert_eq!(
            probe(
                &mut denied,
                "database-tree",
                api::Operation::Tree {
                    operation: tree::Operation::Create
                }
            )
            .unwrap_err()
            .code,
            expected
        );
        assert!(denied.live["database-tree"].tree_resources().is_empty());
    }
}

/// A native resource close must return the same quota as a guest close at the next ordinary tick.
#[test]
#[ignore = "opt-in fixed SDK/WASM packages; rebuild sources: crates/plugin-runtime/tests/fixtures/community; see fixtures/packages/README.md"]
fn community_native_closed_tree_releases_quota_on_manager_tick() {
    let runtime = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        runtime.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    let package = provider("database-tree");
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    for _ in 0..7 {
        probe(
            &mut manager,
            "database-tree",
            api::Operation::Tree {
                operation: tree::Operation::Create,
            },
        )
        .unwrap();
    }
    assert_eq!(
        probe(
            &mut manager,
            "database-tree",
            api::Operation::Tree {
                operation: tree::Operation::Create
            }
        )
        .unwrap_err()
        .code,
        api::ErrorCode::LimitExceeded
    );
    let closed = manager.live["database-tree"]
        .tree_resources()
        .pop()
        .unwrap();
    closed.revoke();
    manager.poll();
    probe(
        &mut manager,
        "database-tree",
        api::Operation::Tree {
            operation: tree::Operation::Create,
        },
    )
    .unwrap();
    assert!(!closed.is_live());
}
