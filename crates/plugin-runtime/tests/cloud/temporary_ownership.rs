//! Temporary cleanup follows the live SDK owner, including rejected same-root configuration candidates.
use super::*;

const ID: &str = "temporary-candidate";
/// Public handles and file replies are the only observation point for temporary lifecycle checks.
fn open_temporary(manager: &mut Manager) -> api::ResourceHandle {
    resource(
        probe(
            manager,
            ID,
            private_op(storage::Operation::Open {
                area: storage::Area::Temporary,
            }),
        )
        .unwrap(),
    )
}
/// All fixture writes traverse the guest's public byte operation, preserving runtime ownership checks.
fn write(manager: &mut Manager, handle: &api::ResourceHandle, path: &str, bytes: &[u8]) {
    probe(
        manager,
        ID,
        Op::WriteFile {
            handle: handle.clone(),
            path: path.into(),
            bytes: bytes.into(),
        },
    )
    .unwrap();
}
/// Typed public read errors distinguish retired handles from missing files without inspecting native bytes.
fn read(
    manager: &mut Manager,
    handle: &api::ResourceHandle,
    path: &str,
) -> Result<Value, api::Failure> {
    probe(
        manager,
        ID,
        Op::ReadFile {
            handle: handle.clone(),
            path: path.into(),
        },
    )
}
/// Rejected candidates preserve the previous instance's file and handle; only a completed retirement removes it.
#[test]
#[ignore = "build temporary-candidate through current host --plugin-package first"]
fn t29_t30_temporary_owner_survives_rejected_configuration_and_cleans_on_retirement() {
    let root = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(root.path().into(), environment(root.path())).unwrap();
    install(&mut manager, &package(ID, |_| {}));
    let old = open_temporary(&mut manager);
    for phase in ["prepare", "validate", "apply"] {
        write(&mut manager, &old, "owned.txt", b"previous temporary bytes");
        manager
            .invoke_command(ID, "reject-at", json!(phase))
            .unwrap();
        let count = manager.resource_count();
        let error = manager
            .update_setting(
                ID,
                plugin_runtime::plugin_protocol::settings::Scope::User,
                "reject",
                Some(json!(true)),
            )
            .unwrap_err();
        assert!(format!("{error:#}").contains(phase));
        assert!(
            matches!(read(&mut manager, &old, "owned.txt").unwrap(),
            Value::Bytes(bytes) if bytes == b"previous temporary bytes"),
            "{phase} candidate must not delete the live owner's temporary directory"
        );
        write(&mut manager, &old, "owned.txt", phase.as_bytes());
        assert!(matches!(read(&mut manager, &old, "owned.txt").unwrap(),
            Value::Bytes(bytes) if bytes == phase.as_bytes()));
        assert_eq!(manager.resource_count(), count);
    }
    manager
        .invoke_command(ID, "reject-at", json!("none"))
        .unwrap();
    manager
        .update_setting(
            ID,
            plugin_runtime::plugin_protocol::settings::Scope::User,
            "reject",
            Some(json!(false)),
        )
        .unwrap();
    assert_eq!(
        read(&mut manager, &old, "owned.txt").unwrap_err().code,
        ErrorCode::InvalidHandle
    );
    let replacement = open_temporary(&mut manager);
    assert_eq!(
        read(&mut manager, &replacement, "owned.txt")
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    write(
        &mut manager,
        &replacement,
        "released.txt",
        b"handle closure does not release folder ownership",
    );
    probe(
        &mut manager,
        ID,
        Op::CloseResource {
            handle: replacement,
        },
    )
    .unwrap();
    manager.disable(ID).unwrap();
    assert_eq!(manager.resource_count(), 0);
    manager.enable(ID).unwrap();
    let enabled = open_temporary(&mut manager);
    assert_eq!(
        read(&mut manager, &enabled, "released.txt")
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    write(
        &mut manager,
        &enabled,
        "restart.txt",
        b"explicit restart retires this folder",
    );
    manager.restart_plugin(ID).unwrap();
    assert_eq!(
        read(&mut manager, &enabled, "restart.txt")
            .unwrap_err()
            .code,
        ErrorCode::InvalidHandle
    );
    let restarted = open_temporary(&mut manager);
    assert_eq!(
        read(&mut manager, &restarted, "restart.txt")
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    write(
        &mut manager,
        &restarted,
        "reopen.txt",
        b"host shutdown retires this folder",
    );
    drop(manager);
    let mut reopened = Manager::open(root.path().into(), environment(root.path())).unwrap();
    let handle = open_temporary(&mut reopened);
    assert_eq!(
        read(&mut reopened, &handle, "reopen.txt").unwrap_err().code,
        ErrorCode::NotFound
    );
    reopened.disable(ID).unwrap();
    assert_eq!(reopened.resource_count(), 0);
}

/// Candidate Snapshot runs after commit_data, but durable publication alone permits temporary ownership.
#[test]
#[ignore = "build temporary-candidate through current host --plugin-package first"]
fn t29_t30_unpublished_temporary_snapshot_and_publish_failure_leave_no_retired_bytes() {
    use plugin_runtime::{InstallControl, InstallStage};
    let root = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(root.path().into(), environment(root.path())).unwrap();
    let old = package(ID, |_| {});
    install(&mut manager, &old);
    for fail_snapshot in [true, false] {
        let data = resource(probe(&mut manager, ID, Op::OpenData).unwrap());
        let cache = resource(
            probe(
                &mut manager,
                ID,
                private_op(storage::Operation::Open {
                    area: storage::Area::Cache,
                }),
            )
            .unwrap(),
        );
        let temporary = open_temporary(&mut manager);
        write(
            &mut manager,
            &data,
            "permanent.txt",
            b"committed private bytes",
        );
        write(
            &mut manager,
            &cache,
            "cached.txt",
            b"committed cached bytes",
        );
        write(
            &mut manager,
            &temporary,
            "retired.txt",
            b"old incarnation temporary bytes",
        );
        let mut files = package(ID, |manifest| manifest["version"] = json!("0.1.1")).files;
        files.insert(
            "snapshot-policy.json".into(),
            serde_json::to_vec(&json!({"open_temporary":true,"fail":fail_snapshot})).unwrap(),
        );
        let candidate = archive_files(files);
        // The ordinary public publish barrier tests the actual backup/restore order, without native test hooks.
        let control_slot = std::sync::Arc::new(std::sync::Mutex::new(None::<InstallControl>));
        let slot = control_slot.clone();
        let control = InstallControl::new(move |stage| {
            if !fail_snapshot && stage == InstallStage::Committing {
                slot.lock().unwrap().as_ref().unwrap().cancel();
            }
        });
        *control_slot.lock().unwrap() = Some(control.clone());
        let error = manager
            .install_with_control(&candidate, candidate.manifest.permissions.clone(), &control)
            .unwrap_err();
        control_slot.lock().unwrap().take();
        if fail_snapshot {
            assert!(
                format!("{error:#}").contains("temporary=InvalidState"),
                "{error:#}"
            );
        } else {
            assert!(format!("{error:#}").contains("cancelled"));
        }
        assert_eq!(manager.installed[ID].manifest.version, old.manifest.version);
        for handle in [data, cache, temporary] {
            assert_eq!(
                read(&mut manager, &handle, "retired.txt").unwrap_err().code,
                ErrorCode::InvalidHandle
            );
        }
        // Observe the restored common root before opening Temporary; reopening cannot conceal orphaned bytes.
        let data = resource(probe(&mut manager, ID, Op::OpenData).unwrap());
        let Value::PrivateListing(list) = probe(
            &mut manager,
            ID,
            private_op(storage::Operation::List {
                handle: data.clone(),
                path: String::new(),
                after: None,
                limit: 128,
            }),
        )
        .unwrap() else {
            panic!("Expected restored data listing");
        };
        assert!(
            list.entries
                .iter()
                .all(|entry| !entry.name.eq_ignore_ascii_case(".temporary"))
        );
        assert!(
            matches!(read(&mut manager, &data, "permanent.txt").unwrap(),
            Value::Bytes(bytes) if bytes == b"committed private bytes")
        );
        let cache = resource(
            probe(
                &mut manager,
                ID,
                private_op(storage::Operation::Open {
                    area: storage::Area::Cache,
                }),
            )
            .unwrap(),
        );
        assert!(matches!(read(&mut manager, &cache, "cached.txt").unwrap(),
            Value::Bytes(bytes) if bytes == b"committed cached bytes"));
        let temporary = open_temporary(&mut manager);
        assert_eq!(
            read(&mut manager, &temporary, "retired.txt")
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
        write(
            &mut manager,
            &temporary,
            "recovered.txt",
            b"new published owner bytes",
        );
        manager.disable(ID).unwrap();
        manager.enable(ID).unwrap();
    }
    manager.disable(ID).unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// Ordinary Data authority cannot create an unowned temporary area or reach it through native aliases.
#[test]
#[ignore = "build temporary-candidate through current host --plugin-package first"]
fn t29_data_root_cannot_bypass_temporary_ownership_or_deny_other_roots() {
    let root = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir(workspace.path().join(".temporary")).unwrap();
    std::fs::write(
        workspace.path().join(".temporary/workspace.txt"),
        b"workspace file",
    )
    .unwrap();
    let mut manager = Manager::open(root.path().into(), environment(workspace.path())).unwrap();
    install(
        &mut manager,
        &package(ID, |manifest| {
            manifest["api"]["required"]["workspace.files"] = json!("^1");
            manifest["api"]["required"]["storage.editor"] = json!("^1");
            manifest["permissions"]
                .as_array_mut()
                .unwrap()
                .push(json!("workspace.read"));
        }),
    );
    let data = resource(probe(&mut manager, ID, Op::OpenData).unwrap());
    assert_eq!(
        probe(
            &mut manager,
            ID,
            private_op(storage::Operation::CreateDirectory {
                handle: data.clone(),
                path: ".temporary".into(),
            })
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidPath
    );
    let temporary = open_temporary(&mut manager);
    write(
        &mut manager,
        &temporary,
        "owned.txt",
        b"one incarnation owns these bytes",
    );
    write(
        &mut manager,
        &data,
        "permanent.txt",
        b"unrelated permanent bytes",
    );
    let mut aliases = vec![
        ".temporary".into(),
        ".TEMPORARY".into(),
        ".temporary.".into(),
        ".temporary ".into(),
    ];
    #[cfg(windows)]
    if let Some(alias) = temporary_short_alias(root.path(), workspace.path()) {
        println!("Controlled Temporary directory has native short alias: {alias}");
        aliases.push(alias);
    }
    for alias in aliases {
        let child = format!("{alias}/owned.txt");
        // Both the directory interface and earlier byte/editor interfaces must use the same area authority.
        for operation in [
            Op::ReadFile {
                handle: data.clone(),
                path: child.clone(),
            },
            Op::WriteFile {
                handle: data.clone(),
                path: child.clone(),
                bytes: b"must not overwrite".to_vec(),
            },
            private_op(storage::Operation::Stat {
                handle: data.clone(),
                path: alias.clone(),
            }),
            private_op(storage::Operation::List {
                handle: data.clone(),
                path: alias.clone(),
                after: None,
                limit: 128,
            }),
            private_op(storage::Operation::CreateDirectory {
                handle: data.clone(),
                path: format!("{alias}/unowned"),
            }),
            private_op(storage::Operation::Rename {
                handle: data.clone(),
                path: "permanent.txt".into(),
                destination: format!("{alias}/moved.txt"),
            }),
            private_op(storage::Operation::Rename {
                handle: data.clone(),
                path: child.clone(),
                destination: "escaped.txt".into(),
            }),
            private_op(storage::Operation::Delete {
                handle: data.clone(),
                path: alias.clone(),
                recursive: true,
            }),
            Op::Editor {
                operation: api::EditorOperation::OpenDataFile { path: child },
                timeout_ms: 1000,
            },
        ] {
            assert_eq!(
                probe(&mut manager, ID, operation).unwrap_err().code,
                ErrorCode::InvalidPath,
                "Data alias {alias}"
            );
        }
    }
    assert!(
        matches!(read(&mut manager, &temporary, "owned.txt").unwrap(), Value::Bytes(bytes) if bytes == b"one incarnation owns these bytes")
    );
    assert!(
        matches!(read(&mut manager, &data, "permanent.txt").unwrap(), Value::Bytes(bytes) if bytes == b"unrelated permanent bytes")
    );
    let cache = resource(
        probe(
            &mut manager,
            ID,
            private_op(storage::Operation::Open {
                area: storage::Area::Cache,
            }),
        )
        .unwrap(),
    );
    probe(
        &mut manager,
        ID,
        private_op(storage::Operation::CreateDirectory {
            handle: cache.clone(),
            path: ".temporary".into(),
        }),
    )
    .unwrap();
    write(
        &mut manager,
        &cache,
        ".temporary/cache.txt",
        b"cache namespace is ordinary content",
    );
    assert!(
        matches!(read(&mut manager, &cache, ".temporary/cache.txt").unwrap(), Value::Bytes(bytes) if bytes == b"cache namespace is ordinary content")
    );
    let workspace = resource(probe(&mut manager, ID, Op::OpenWorkspace).unwrap());
    assert!(
        matches!(read(&mut manager, &workspace, ".temporary/workspace.txt").unwrap(), Value::Bytes(bytes) if bytes == b"workspace file")
    );
    manager.disable(ID).unwrap();
    assert_eq!(manager.resource_count(), 0);
}

/// The controlled Windows fixture obtains only its own area's optional 8.3 alias, never user paths.
#[cfg(windows)]
fn temporary_short_alias(profile: &std::path::Path, workspace: &std::path::Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;
    let directory = profile
        .join("data/temporary-candidate/workspaces")
        .join(format!(
            "{:x}",
            Sha256::digest(
                workspace
                    .canonicalize()
                    .unwrap()
                    .to_string_lossy()
                    .as_bytes()
            )
        ))
        .join("files/.temporary");
    let wide = directory
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // SAFETY: The input is NUL-terminated; the second call receives the exact OS-requested output capacity.
    let capacity = unsafe { GetShortPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0) };
    if capacity == 0 {
        return None;
    }
    let mut output = vec![0; capacity as usize];
    let length = unsafe { GetShortPathNameW(wide.as_ptr(), output.as_mut_ptr(), capacity) };
    if length == 0 || length >= capacity {
        return None;
    }
    let short = std::path::PathBuf::from(std::ffi::OsString::from_wide(&output[..length as usize]));
    let alias = short.file_name()?.to_str()?.to_owned();
    (!alias.eq_ignore_ascii_case(".temporary")).then_some(alias)
}

/// Private editor navigation checks consent and negotiation before revealing any native path state.
#[test]
#[ignore = "build cloud packages through current host --plugin-package first"]
fn t29_private_editor_navigation_checks_authority_before_path_lookup() {
    let root = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(root.path().into(), environment(root.path())).unwrap();
    for (id, capability, consent, expected) in [
        (
            "cloud-translation",
            true,
            false,
            ErrorCode::PermissionDenied,
        ),
        (
            "code-host-query",
            false,
            true,
            ErrorCode::CapabilityUnavailable,
        ),
    ] {
        install(
            &mut manager,
            &package(id, |manifest| {
                if capability {
                    manifest["api"]["required"]["storage.editor"] = json!("^1");
                }
                if !consent {
                    manifest["permissions"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|value| value != "storage");
                }
            }),
        );
        for path in [".temporary/hidden.txt", "missing.txt"] {
            let failure = probe(
                &mut manager,
                id,
                Op::Editor {
                    operation: api::EditorOperation::OpenDataFile { path: path.into() },
                    timeout_ms: 1000,
                },
            )
            .unwrap_err();
            assert_eq!(
                failure.code, expected,
                "Unapproved private target must not be looked up"
            );
        }
        manager.disable(id).unwrap();
    }
    assert_eq!(manager.resource_count(), 0);
}
