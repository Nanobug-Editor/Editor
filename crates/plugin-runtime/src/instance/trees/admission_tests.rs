//! A fixed real component supplies ordinary production State for tree capability/ownership admission.
use super::*;
use sha2::{Digest, Sha256};

/// A newer inventory cannot opt an old provider into pointer preferences. Allocation and later
/// refresh retain the same owned lifetime/grant rules as an ordinary tree.
#[test]
#[ignore = "opt-in frozen database-tree WASM; current runtime interaction admission"]
fn tree_interaction_options_require_explicit_floor_and_preserve_owned_lifecycle() {
    let package = fixed_database();
    let directory = tempfile::tempdir().unwrap();
    let assets = directory.path().join("assets");
    std::fs::create_dir_all(&assets).unwrap();
    for (name, bytes) in &package.files {
        let path = assets.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    let engine = Instance::engine().unwrap();
    for (range, allowed) in [("^1", false), (">=1.4,<2", false), (">=1.5,<2", true)] {
        let mut manifest = package.manifest.clone();
        manifest
            .api
            .as_mut()
            .unwrap()
            .required
            .insert("ui.tree".into(), range.parse().unwrap());
        let mut instance = Instance::prepare(
            &engine,
            package.component().unwrap(),
            &manifest,
            &manifest.permissions,
            Environment {
                workspace: directory.path().display().to_string(),
                ..Default::default()
            },
            directory.path().join("data"),
            assets.clone(),
            None,
        )
        .unwrap();
        instance.activate().unwrap();
        let state = instance.store.data_mut();
        let operation = || tree::Operation::CreateWithOptions {
            options: tree::Options {
                selection: tree::SelectionGesture::DoubleClick,
                ..Default::default()
            },
        };
        let before = state.trees.len();
        let result = state.tree_request(operation());
        if !allowed {
            assert_eq!(
                result.unwrap_err().code,
                ErrorCode::CapabilityUnavailable,
                "{range}"
            );
            assert_eq!(
                state.trees.len(),
                before,
                "denied creation allocates no tree"
            );
            continue;
        }
        let Value::Resource(handle) = result.unwrap() else {
            panic!("owned native tree")
        };
        let resource = state.owned_tree(&handle).unwrap();
        assert_eq!(
            resource.selection_gesture(),
            tree::SelectionGesture::DoubleClick
        );
        state
            .tree_request(tree::Operation::Refresh {
                tree: handle.clone(),
            })
            .unwrap();
        resource.suspend();
        assert_eq!(
            resource.selection_gesture(),
            tree::SelectionGesture::DoubleClick
        );
        state.permissions.remove("ui.panels");
        assert_eq!(
            state.tree_request(operation()).unwrap_err().code,
            ErrorCode::PermissionDenied
        );
        resource.revoke();
        assert_eq!(
            state.owned_tree(&handle).err().unwrap().code,
            ErrorCode::InvalidHandle
        );
    }
}

/// Structural deltas use the ordinary owned-tree dispatcher, negotiated floor and asset authority.
#[test]
#[ignore = "opt-in frozen database-tree WASM; current runtime tree patch admission"]
fn incremental_tree_patch_enforces_capability_ownership_permissions_and_artwork() {
    let package = fixed_database();
    let directory = tempfile::tempdir().unwrap();
    let assets = directory.path().join("assets");
    std::fs::create_dir_all(&assets).unwrap();
    for (name, bytes) in &package.files {
        let path = assets.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    std::fs::write(assets.join("patch.svg"),br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M1 1h12v12H1z"/></svg>"#).unwrap();
    let mut manifest = package.manifest.clone();
    manifest
        .api
        .as_mut()
        .unwrap()
        .required
        .insert("ui.tree".into(), ">=1.4,<2".parse().unwrap());
    manifest
        .api
        .as_mut()
        .unwrap()
        .required
        .insert("ui.icons".into(), "^1".parse().unwrap());
    manifest.permissions.insert("assets.read".into());
    let engine = Instance::engine().unwrap();
    let mut instance = Instance::prepare(
        &engine,
        package.component().unwrap(),
        &manifest,
        &manifest.permissions,
        Environment {
            workspace: directory.path().display().to_string(),
            ..Default::default()
        },
        directory.path().join("data"),
        assets,
        None,
    )
    .unwrap();
    instance.activate().unwrap();
    let state = instance.store.data_mut();
    assert!(
        state.tree_menu_1_1
            && state.tree_update_1_2
            && state.tree_presentation_1_3
            && state.tree_patch_1_4
    );
    let Value::Resource(handle) = state.tree_request(tree::Operation::Create).unwrap() else {
        panic!("tree");
    };
    let resource = state.owned_tree(&handle).unwrap();
    let request = resource.request(None, false).unwrap().unwrap();
    state
        .tree_request(tree::Operation::Reply {
            request,
            result: Ok(tree::Page {
                nodes: vec![tree::Node::new("old", "old", false)],
                next: None,
            }),
        })
        .unwrap();
    let patch = || tree::Patch {
        nodes: vec![tree::Node::new("new", "new", false)],
        branches: vec![tree::BranchPatch {
            parent: None,
            children: vec!["old".into(), "new".into()],
            next: None,
        }],
    };
    let operation = |tree, patch| tree::Operation::ApplyPatch { tree, patch };
    state.tree_patch_1_4 = false;
    assert_eq!(
        state
            .tree_request(operation(handle.clone(), patch()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnavailable
    );
    state.tree_patch_1_4 = true;
    state.permissions.remove("ui.panels");
    assert_eq!(
        state
            .tree_request(operation(handle.clone(), patch()))
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
    state.permissions.insert("ui.panels".into());
    let mut forged = handle.clone();
    forged.instance = "foreign".into();
    assert_eq!(
        state
            .tree_request(operation(forged, patch()))
            .unwrap_err()
            .code,
        ErrorCode::InvalidHandle
    );
    let mut invalid = patch();
    invalid.nodes[0].icon = Some(plugin_protocol::ui::ToolIcon::new(
        "../outside.svg",
        "../outside.svg",
    ));
    assert_eq!(
        state
            .tree_request(operation(handle.clone(), invalid))
            .unwrap_err()
            .code,
        ErrorCode::InvalidPath
    );
    assert_eq!(
        resource.snapshot().nodes.len(),
        1,
        "rejected delta must not mutate cached data"
    );
    let mut valid = patch();
    valid.nodes[0].icon = Some(plugin_protocol::ui::ToolIcon::new("patch.svg", "patch.svg"));
    state
        .tree_request(operation(handle.clone(), valid))
        .unwrap();
    assert_eq!(resource.snapshot().nodes.len(), 2);
    assert_eq!(resource.snapshot().icons.len(), 1);
    resource.revoke();
    assert_eq!(
        state
            .tree_request(operation(handle, tree::Patch::default()))
            .unwrap_err()
            .code,
        ErrorCode::InvalidHandle
    );
}

/// Verify the immutable fixture ledger; this test does not rebuild or modify historical packages.
fn fixed_database() -> crate::Package {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/packages");
    let ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("inputs.json")).unwrap()).unwrap();
    let input = ledger["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|input| input["archive"] == "database-tree-0.1.1.zip")
        .unwrap();
    let bytes = std::fs::read(directory.join(input["archive"].as_str().unwrap())).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        input["sha256"].as_str().unwrap()
    );
    crate::Package::from_bytes(&bytes).unwrap()
}

/// Exercise the actual per-instance operation dispatcher; old WASM is only the bootstrap component,
/// while Update is dispatched by the current runtime rather than decoded by the old guest SDK.
#[test]
#[ignore = "opt-in frozen database-tree WASM; uses exact fixtures/packages ledger and current runtime admission"]
fn cached_metadata_update_enforces_live_dispatch_ownership_grants_and_icon_admission() {
    let package = fixed_database();
    let directory = tempfile::tempdir().unwrap();
    let assets = directory.path().join("assets");
    std::fs::create_dir_all(&assets).unwrap();
    for (name, bytes) in &package.files {
        let path = assets.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    let mut manifest = package.manifest.clone();
    manifest
        .api
        .as_mut()
        .unwrap()
        .required
        .insert("ui.tree".into(), ">=1.2,<2".parse().unwrap());
    manifest
        .api
        .as_mut()
        .unwrap()
        .required
        .insert("ui.icons".into(), "^1".parse().unwrap());
    manifest.permissions.insert("assets.read".into());
    let engine = Instance::engine().unwrap();
    let mut instance = Instance::prepare(
        &engine,
        package.component().unwrap(),
        &manifest,
        &manifest.permissions,
        Environment {
            workspace: directory.path().display().to_string(),
            ..Default::default()
        },
        directory.path().join("data"),
        assets.clone(),
        None,
    )
    .unwrap();
    instance.activate().unwrap();
    let state = instance.store.data_mut();
    assert!(state.tree_menu_1_1 && state.tree_update_1_2);
    let Value::Resource(handle) = state.tree_request(tree::Operation::Create).unwrap() else {
        panic!("owned tree expected")
    };
    let resource = state.owned_tree(&handle).unwrap();
    let request = resource.request(None, false).unwrap().unwrap();
    let node = tree::Node::new("root", "Original", true);
    state
        .tree_request(tree::Operation::Reply {
            request,
            result: Ok(tree::Page {
                nodes: vec![node.clone()],
                next: None,
            }),
        })
        .unwrap();
    let original = resource.snapshot();
    let mut changed = node;
    changed.label = "Updated".into();
    let operation = |tree, nodes| tree::Operation::Update { tree, nodes };

    state.tree_update_1_2 = false;
    assert_eq!(
        state
            .tree_request(operation(handle.clone(), vec![changed.clone()]))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnavailable
    );
    state.tree_update_1_2 = true;
    let mut foreign = handle.clone();
    foreign.instance = "different-instance".into();
    assert_eq!(
        state
            .tree_request(operation(foreign, vec![changed.clone()]))
            .unwrap_err()
            .code,
        ErrorCode::InvalidHandle
    );
    let mut foreign = handle.clone();
    foreign.scope = "different-scope".into();
    assert_eq!(
        state
            .tree_request(operation(foreign, vec![changed.clone()]))
            .unwrap_err()
            .code,
        ErrorCode::InvalidHandle
    );
    state.permissions.remove("ui.panels");
    assert_eq!(
        state
            .tree_request(operation(handle.clone(), vec![changed.clone()]))
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
    state.permissions.insert("ui.panels".into());
    assert_eq!(resource.snapshot().nodes, original.nodes);
    assert_eq!(resource.snapshot().revision, original.revision);

    // Malformed artwork for an uncached ID is never opened/parsed or retained. Its path and
    // grants remain checked, and the same artwork is rejected when it would become visible.
    std::fs::write(assets.join("invalid.svg"), b"not SVG").unwrap();
    let mut unknown = tree::Node::new("uncached", "Unknown", false);
    unknown.icon = Some(ui::ToolIcon {
        light: "invalid.svg".into(),
        dark: "invalid.svg".into(),
    });
    state
        .tree_request(operation(handle.clone(), vec![unknown.clone()]))
        .unwrap();
    assert_eq!(resource.snapshot().revision, original.revision);
    assert!(resource.snapshot().icons.is_empty());
    state.permissions.remove("assets.read");
    assert_eq!(
        state
            .tree_request(operation(handle.clone(), vec![unknown]))
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
    state.permissions.insert("assets.read".into());
    changed.icon = Some(ui::ToolIcon {
        light: "invalid.svg".into(),
        dark: "invalid.svg".into(),
    });
    assert_eq!(
        state
            .tree_request(operation(handle.clone(), vec![changed.clone()]))
            .unwrap_err()
            .code,
        ErrorCode::InvalidRequest
    );
    assert_eq!(resource.snapshot().revision, original.revision);
    changed.icon = None;
    state
        .tree_request(operation(handle.clone(), vec![changed.clone()]))
        .unwrap();
    assert_eq!(resource.snapshot().epoch, original.epoch);
    assert_eq!(resource.snapshot().nodes["root"], changed);
    resource.revoke();
    assert_eq!(
        state
            .tree_request(operation(handle, Vec::new()))
            .unwrap_err()
            .code,
        ErrorCode::InvalidHandle
    );
}
