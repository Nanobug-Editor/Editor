//! Independent real SDK packages exercise public root admission, revocation and bounded file streams.
/// Stable version/hash inputs preserve the independently built guests after plugin sources move.
#[path = "support/packages.rs"]
mod frozen_packages;
use plugin_runtime::{
    Manager, Package,
    plugin_protocol::{Environment, api, ui, workspace},
};
use std::io::{Cursor, Write};
use std::path::Path;

/// Version/hash inputs were built through the public host; tests do not invent a guest contract.
fn install(manager: &mut Manager, plugin: &str) {
    let package = contract_package(plugin, |_| {}).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
}

/// Declaration variants reuse the exact real SDK component and enter ordinary package admission.
fn contract_package(
    plugin: &str,
    edit: impl FnOnce(&mut serde_json::Value),
) -> anyhow::Result<Package> {
    let package = frozen_packages::package(&format!("{plugin}-0.1.1"));
    let mut files = package.files;
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    edit(&mut manifest);
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
    Package::from_bytes(&archive.finish().unwrap().into_inner())
}

/// The ordinary guest command exposes bounded results through its declared panel.
fn probe(
    manager: &mut Manager,
    plugin: &str,
    operation: api::Operation,
) -> Result<api::Value, api::Failure> {
    manager
        .invoke_command(plugin, "probe", serde_json::to_value(operation).unwrap())
        .unwrap();
    let ui::Kind::Text { text } = &manager.live[plugin]
        .views
        .values()
        .next()
        .unwrap()
        .root
        .kind
    else {
        panic!("probe output expected")
    };
    serde_json::from_str(text).unwrap()
}

fn root(manager: &mut Manager, plugin: &str) -> api::ResourceHandle {
    let api::Value::Resource(root) = probe(manager, plugin, api::Operation::OpenWorkspace).unwrap()
    else {
        panic!("workspace root expected")
    };
    root
}

fn stat(root: api::ResourceHandle, path: &str) -> api::Operation {
    api::Operation::Editor {
        operation: api::EditorOperation::Workspace {
            operation: workspace::Operation::Stat {
                target: workspace::FileTarget {
                    root,
                    path: path.into(),
                },
            },
        },
        timeout_ms: 30_000,
    }
}

fn events(manager: &mut Manager, plugin: &str) -> Vec<api::Notification> {
    manager
        .invoke_command(plugin, "events", serde_json::Value::Null)
        .unwrap();
    let ui::Kind::Text { text } = &manager.live[plugin]
        .views
        .values()
        .next()
        .unwrap()
        .root
        .kind
    else {
        panic!("observation panel expected")
    };
    serde_json::from_str(text).unwrap()
}

/// Traversal/streams/devices, private roots, peers, released roots and junctions fail before host IO.
#[test]
#[ignore = "opt-in fixed SDK/WASM packages; rebuild sources: crates/plugin-runtime/tests/fixtures/community; see fixtures/packages/README.md"]
fn workspace_transaction_roots_reject_escape_private_peer_and_revoked_authority() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("owned.txt"), "owned").unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    // A legitimate private grant still cannot repurpose its owned root for workspace file work.
    let package = contract_package("workspace-scaffold", |manifest| {
        manifest["api"]["required"]["storage.private"] = serde_json::json!("^1");
        manifest["permissions"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!("storage"));
    })
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    install(&mut manager, "workspace-converter");
    let handle = root(&mut manager, "workspace-scaffold");
    for path in [
        "../outside",
        "/outside",
        "C:/outside",
        "owned.txt:secret",
        "CON",
        "nul.txt",
        "child/../owned.txt",
        "child\\owned.txt",
        "\\\\.\\NUL",
        "trailing.",
        "trailing ",
    ] {
        assert_eq!(
            probe(
                &mut manager,
                "workspace-scaffold",
                stat(handle.clone(), path)
            )
            .unwrap_err()
            .code,
            api::ErrorCode::InvalidPath,
            "accepted {path}"
        );
    }
    assert_eq!(
        probe(
            &mut manager,
            "workspace-converter",
            stat(handle.clone(), "owned.txt")
        )
        .unwrap_err()
        .code,
        api::ErrorCode::InvalidHandle
    );
    let api::Value::Resource(private) =
        probe(&mut manager, "workspace-scaffold", api::Operation::OpenData).unwrap()
    else {
        panic!("private root expected")
    };
    assert_eq!(
        probe(
            &mut manager,
            "workspace-scaffold",
            stat(private, "owned.txt")
        )
        .unwrap_err()
        .code,
        api::ErrorCode::InvalidHandle
    );
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), "outside").unwrap();
        let junction = workspace.path().join("redirect");
        let result = std::process::Command::new("powershell.exe").creation_flags(0x08000000).args(["-NoProfile", "-NonInteractive", "-Command", "$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:NANOBUG_TRANSACTION_LINK -Value $env:NANOBUG_TRANSACTION_TARGET | Out-Null"]).env("NANOBUG_TRANSACTION_LINK", &junction).env("NANOBUG_TRANSACTION_TARGET", outside.path()).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            probe(
                &mut manager,
                "workspace-scaffold",
                stat(handle.clone(), "redirect/secret.txt")
            )
            .unwrap_err()
            .code,
            api::ErrorCode::InvalidPath
        );
        // Remove only the verified link, through the same native filesystem API used to inspect it.
        use std::os::windows::fs::MetadataExt;
        assert_ne!(
            std::fs::symlink_metadata(&junction)
                .unwrap()
                .file_attributes()
                & 0x400,
            0
        );
        std::fs::remove_dir(junction).unwrap();
    }
    assert!(matches!(
        probe(
            &mut manager,
            "workspace-scaffold",
            stat(handle.clone(), "owned.txt")
        )
        .unwrap(),
        api::Value::Accepted(_)
    ));
    let request = manager
        .live
        .get_mut("workspace-scaffold")
        .unwrap()
        .take_editor_requests()
        .pop()
        .unwrap();
    let proof = request.workspace_request().unwrap().files()[0].clone();
    assert!(proof.guard().is_ok());
    probe(
        &mut manager,
        "workspace-scaffold",
        api::Operation::CloseResource {
            handle: handle.clone(),
        },
    )
    .unwrap();
    assert_eq!(
        proof.guard().err().unwrap().code,
        api::ErrorCode::InvalidHandle
    );
    assert_eq!(
        probe(
            &mut manager,
            "workspace-scaffold",
            stat(handle, "owned.txt")
        )
        .unwrap_err()
        .code,
        api::ErrorCode::InvalidHandle
    );
    let handle = root(&mut manager, "workspace-scaffold");
    manager.disable("workspace-scaffold").unwrap();
    manager.enable("workspace-scaffold").unwrap();
    assert_eq!(
        probe(
            &mut manager,
            "workspace-scaffold",
            stat(handle, "owned.txt")
        )
        .unwrap_err()
        .code,
        api::ErrorCode::InvalidHandle
    );
}

/// Capability negotiation and installation grants reject writes before allocating native work.
#[test]
#[ignore = "opt-in fixed SDK/WASM packages; rebuild sources: crates/plugin-runtime/tests/fixtures/community; see fixtures/packages/README.md"]
fn workspace_transaction_missing_write_grants_and_unsupported_required_version_have_no_effects() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    let unsupported = contract_package("workspace-converter", |manifest| {
        manifest["api"]["required"]["workspace.files"] = serde_json::json!("^99");
    });
    // Required-version rejection already happens at public package admission, before Manager install.
    assert!(unsupported.is_err());
    assert!(manager.live.is_empty());

    let readonly = contract_package("workspace-converter", |manifest| {
        manifest["permissions"]
            .as_array_mut()
            .unwrap()
            .retain(|permission| permission != "workspace.write" && permission != "editor.write");
    })
    .unwrap();
    manager
        .install(&readonly, readonly.manifest.permissions.clone())
        .unwrap();
    let root = root(&mut manager, "workspace-converter");
    let changes = [
        workspace::Change::Write {
            target: workspace::FileTarget {
                root,
                path: "denied.txt".into(),
            },
            expected: Some(workspace::FileStamp::Missing),
            bytes: b"must not be written".to_vec(),
        },
        workspace::Change::EditDocument {
            document: api::DocumentVersion {
                id: "versioned-session".into(),
                path: "owned.txt".into(),
                revision: 1,
            },
            edits: vec![api::TextEdit {
                range: api::TextRange { start: 0, end: 0 },
                text: "must not enter native editing".into(),
            }],
            selection: api::TextRange { start: 0, end: 0 },
        },
    ];
    for change in changes {
        assert_eq!(
            probe(
                &mut manager,
                "workspace-converter",
                api::Operation::Editor {
                    operation: api::EditorOperation::Workspace {
                        operation: workspace::Operation::ApplyPlan {
                            plan: workspace::Plan {
                                origin: None,
                                conditions: Vec::new(),
                                title: "Denied proposal".into(),
                                changes: vec![change]
                            },
                        },
                    },
                    timeout_ms: 30_000,
                },
            )
            .unwrap_err()
            .code,
            api::ErrorCode::PermissionDenied
        );
    }
    assert!(
        manager
            .live
            .get_mut("workspace-converter")
            .unwrap()
            .take_editor_requests()
            .is_empty()
    );
    assert!(!workspace.path().join("denied.txt").exists());
}

/// Actual guests opt in independently; sequences coalesce, overflow terminates and root release stops.
#[test]
#[ignore = "opt-in fixed SDK/WASM packages; rebuild sources: crates/plugin-runtime/tests/fixtures/community; see fixtures/packages/README.md"]
fn workspace_transaction_stream_coalesces_overflows_and_stops_on_root_release() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    install(&mut manager, "workspace-scaffold");
    install(&mut manager, "workspace-converter");
    let handle = root(&mut manager, "workspace-scaffold");
    let api::Value::Resource(subscription) = probe(
        &mut manager,
        "workspace-scaffold",
        api::Operation::SubscribeWorkspaceFiles {
            root: handle.clone(),
        },
    )
    .unwrap() else {
        panic!("subscription expected")
    };
    let changes = [
        workspace::FileEventKind::Created {
            path: "unopened.txt".into(),
        },
        workspace::FileEventKind::Modified {
            path: "unopened.txt".into(),
        },
        workspace::FileEventKind::Modified {
            path: "unopened.txt".into(),
        },
        workspace::FileEventKind::Renamed {
            source: "unopened.txt".into(),
            target: "renamed.txt".into(),
        },
        workspace::FileEventKind::Deleted {
            path: "renamed.txt".into(),
        },
        workspace::FileEventKind::RescanRequired,
    ];
    for (index, kind) in changes.into_iter().enumerate() {
        manager.workspace_file_event(workspace::FileEvent {
            sequence: index as u64 + 1,
            kind,
        });
    }
    manager.poll();
    let log = events(&mut manager, "workspace-scaffold");
    let observed = log
        .iter()
        .filter_map(|event| match event {
            api::Notification::WorkspaceFile {
                subscription: owner,
                event,
            } if *owner == subscription => Some(event),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        observed
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        [1, 3, 4, 5, 6]
    );
    assert!(
        events(&mut manager, "workspace-converter").is_empty(),
        "unsubscribed independent guest receives no paths"
    );
    for index in 7..140 {
        manager.workspace_file_event(workspace::FileEvent {
            sequence: index,
            kind: workspace::FileEventKind::Created {
                path: format!("file-{index}.txt"),
            },
        });
    }
    manager.poll();
    assert!(events(&mut manager, "workspace-scaffold").iter().any(|event| matches!(event, api::Notification::SubscriptionFailed { subscription: owner, error } if *owner == subscription && error.code == api::ErrorCode::LimitExceeded)));
    let api::Value::Resource(second) = probe(
        &mut manager,
        "workspace-scaffold",
        api::Operation::SubscribeWorkspaceFiles {
            root: handle.clone(),
        },
    )
    .unwrap() else {
        panic!("resubscription expected")
    };
    probe(
        &mut manager,
        "workspace-scaffold",
        api::Operation::CloseResource { handle },
    )
    .unwrap();
    manager.workspace_file_event(workspace::FileEvent {
        sequence: 140,
        kind: workspace::FileEventKind::Created {
            path: "after-release.txt".into(),
        },
    });
    manager.poll();
    let log = events(&mut manager, "workspace-scaffold");
    assert!(log.iter().any(|event| matches!(event, api::Notification::SubscriptionFailed { subscription: owner, error } if *owner == second && error.code == api::ErrorCode::InvalidHandle)));
    assert!(!log.iter().any(|event| matches!(event, api::Notification::WorkspaceFile { event, .. } if event.sequence == 140)));
}
