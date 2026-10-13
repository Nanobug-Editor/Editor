//! Scope contracts are exercised through the installed real WASM guest and Manager boundary.
/// Read fixed, hash-checked Package inputs without depending on removed plugin source or distribution payloads.
#[path = "support/packages.rs"]
mod test_packages;
use plugin_runtime::{
    Manager, Package,
    plugin_protocol::{Environment, ui::Kind},
};
use serde_json::json;

/// Existing scope ownership outlives its pathname, so external moves cannot block checked retirement.
#[test]
fn concurrent_scope_can_retire_after_its_directory_moves() {
    let store = tempfile::tempdir().unwrap();
    let projects = tempfile::tempdir().unwrap();
    let a = projects.path().join("a");
    let b = projects.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let environment = |root: &std::path::Path| Environment {
        workspace: root.canonicalize().unwrap().display().to_string(),
        ..Default::default()
    };
    let original = environment(&a);
    let peer = environment(&b);
    let mut manager = Manager::open(store.path().into(), original.clone()).unwrap();
    manager.select_workspace(peer.clone(), true).unwrap();
    // The move remains within this fixture's absolute project directory; no user path is modified.
    std::fs::rename(&a, projects.path().join("moved-a")).unwrap();
    manager.select_workspace(original.clone(), true).unwrap();
    manager.close_selected_workspace_checked().unwrap();
    assert!(!manager.workspace_is_open(&original.workspace));
    manager.select_workspace(peer.clone(), true).unwrap();
    assert_eq!(manager.workspace(), peer.workspace);
    manager.shutdown_checked().unwrap();
}

/// Activate the frozen command through its ordinary cold-demand receipt before invoking its business hook.
fn activate_scope_command(manager: &mut Manager, id: &str) {
    use plugin_runtime::{DemandReason, plugin_protocol::api::RequestUpdate};
    let token = manager.availability(id).unwrap();
    let request = manager
        .begin_demand(&token, DemandReason::Command("scope-write".into()), 120_000)
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(130);
    loop {
        manager.poll();
        match request.status() {
            RequestUpdate::Completed {
                result: Ok(receipt),
            } => {
                manager.admit_receipt(&token, &receipt).unwrap();
                return;
            }
            RequestUpdate::Completed { result: Err(error) } => {
                panic!("Activation failed: {error:?}")
            }
            RequestUpdate::Cancelled { reason, .. } => panic!("Activation cancelled: {reason:?}"),
            _ => {}
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Command activation did not complete"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// A second project's native execution must not suspend the first, and closing it retires only its own child.
#[cfg(windows)]
#[test]
#[ignore = "opt-in frozen capability-example Package/WASM and real Windows native executions"]
fn concurrent_workspaces_keep_both_native_programs_active_and_close_only_the_owner() {
    use std::time::{Duration, Instant};
    let root = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let pulse_a = a.path().join("native-pulse.txt");
    let pulse_b = b.path().join("native-pulse.txt");
    std::fs::write(a.path().join("source.txt"), "workspace A").unwrap();
    std::fs::write(b.path().join("source.txt"), "workspace B").unwrap();
    let env = |path: &std::path::Path| Environment {
        workspace: path.display().to_string(),
        ..Default::default()
    };
    let program = |pulse: &std::path::Path| {
        plugin_runtime::RunRequest {
        program: "powershell.exe".into(),
        // Fixture text is fixed; the owned path travels as an environment value, never shell interpolation.
        args: vec!["-NoProfile".into(), "-Command".into(),
            "while ($true) { [IO.File]::AppendAllText($env:NANOBUG_TEST_PULSE, 'tick'); Start-Sleep -Milliseconds 30 }".into()],
        env: vec![plugin_runtime::RunEnvEntry {
            name: "NANOBUG_TEST_PULSE".into(), value: pulse.display().to_string(),
        }],
        cwd: None,
        name: Some("Owned native activity".into()),
    }
    };
    let package = Package::read(&test_packages::archive("capability-example")).unwrap();
    let id = &package.manifest.id;
    let mut manager = Manager::open(root.path().into(), env(a.path())).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    activate_scope_command(&mut manager, id);
    manager
        .invoke_command(id, "scope-write", json!({"text":"private A"}))
        .unwrap();
    manager.start_execution(program(&pulse_a)).unwrap();
    // Use actual child output, rather than a manager map or a running-state enum, as the liveness evidence.
    let wait = |manager: &mut Manager, pulse: &std::path::Path| {
        let deadline = Instant::now() + Duration::from_secs(15);
        while !pulse.exists() && Instant::now() < deadline {
            manager.poll();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            pulse.exists(),
            "native program did not produce its first pulse"
        );
    };
    wait(&mut manager, &pulse_a);
    manager.select_workspace(env(b.path()), true).unwrap();
    activate_scope_command(&mut manager, id);
    manager
        .invoke_command(id, "scope-write", json!({"text":"private B"}))
        .unwrap();
    assert_eq!(text(&manager, id), "workspace B|private B");
    manager.start_execution(program(&pulse_b)).unwrap();
    wait(&mut manager, &pulse_b);
    let before_a = std::fs::metadata(&pulse_a).unwrap().len();
    let before_b = std::fs::metadata(&pulse_b).unwrap().len();
    std::thread::sleep(Duration::from_millis(200));
    manager.poll();
    assert!(
        std::fs::metadata(&pulse_a).unwrap().len() > before_a,
        "selecting B stopped A's native activity"
    );
    assert!(std::fs::metadata(&pulse_b).unwrap().len() > before_b);
    manager.close_selected_workspace_checked().unwrap();
    let closed_b = std::fs::metadata(&pulse_b).unwrap().len();
    manager.select_workspace(env(a.path()), true).unwrap();
    manager
        .invoke_command(id, "scope-read", json!(null))
        .unwrap();
    assert_eq!(text(&manager, id), "workspace A|private A");
    let retained_a = std::fs::metadata(&pulse_a).unwrap().len();
    std::thread::sleep(Duration::from_millis(200));
    manager.poll();
    assert!(std::fs::metadata(&pulse_a).unwrap().len() > retained_a);
    assert_eq!(
        std::fs::metadata(&pulse_b).unwrap().len(),
        closed_b,
        "closed B retained native activity"
    );
    // A restricted reopening cannot borrow the native execution authority of the still-running trusted project.
    manager.select_workspace(env(b.path()), false).unwrap();
    assert!(manager.start_execution(program(&pulse_b)).is_err());
    manager.close_selected_workspace_checked().unwrap();
    manager.select_workspace(env(a.path()), true).unwrap();
    manager
        .invoke_command(id, "scope-read", json!(null))
        .unwrap();
    assert_eq!(text(&manager, id), "workspace A|private A");
    manager.shutdown_checked().unwrap();
}

/// Read only the public native text output emitted by the fixture's declared command.
fn text(manager: &Manager, id: &str) -> String {
    let scene = manager.live[id].views.values().next().unwrap();
    let Kind::Text { text } = &scene.as_ref().root.kind else {
        panic!("native text expected")
    };
    text.clone()
}

/// The same package cache must never imply shared workspace data or native view state.
#[test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn workspace_instances_keep_private_data_and_workspace_reads_separate() {
    let root = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    std::fs::write(a.path().join("source.txt"), "workspace A").unwrap();
    std::fs::write(b.path().join("source.txt"), "workspace B").unwrap();
    let package = Package::read(&test_packages::archive("capability-example")).unwrap();
    let env = |path: &std::path::Path| Environment {
        workspace: path.display().to_string(),
        ..Default::default()
    };
    let mut manager = Manager::open(root.path().into(), env(a.path())).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    manager
        .invoke_command(
            &package.manifest.id,
            "scope-write",
            json!({"text":"private A"}),
        )
        .unwrap();
    assert_eq!(
        text(&manager, &package.manifest.id),
        "workspace A|private A"
    );
    // Opening another logical scope must not retire the first scope's live instance.
    manager.switch_workspace(env(b.path()), true).unwrap();
    manager
        .invoke_command(
            &package.manifest.id,
            "scope-write",
            json!({"text":"private B"}),
        )
        .unwrap();
    assert_eq!(
        text(&manager, &package.manifest.id),
        "workspace B|private B"
    );
    manager.switch_workspace(env(a.path()), true).unwrap();
    manager
        .invoke_command(&package.manifest.id, "scope-read", json!(null))
        .unwrap();
    assert_eq!(
        text(&manager, &package.manifest.id),
        "workspace A|private A"
    );
}

/// The fixture returns typed SDK results as native text instead of trapping on expected failures.
fn probe(
    manager: &mut Manager,
    id: &str,
    operation: plugin_runtime::plugin_protocol::api::Operation,
) -> Result<
    plugin_runtime::plugin_protocol::api::Value,
    plugin_runtime::plugin_protocol::api::Failure,
> {
    manager
        .invoke_command(id, "scope-probe", serde_json::to_value(operation).unwrap())
        .unwrap();
    serde_json::from_str(&text(manager, id)).unwrap()
}

/// A handle cannot be borrowed by another instance or reused after explicit release or shutdown.
#[test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn resource_handles_reject_foreign_and_retired_owners_and_release_all_views() {
    use plugin_runtime::plugin_protocol::api::{ErrorCode, Operation as Op, Value};
    let root = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let env = |path: &std::path::Path| Environment {
        workspace: path.display().to_string(),
        ..Default::default()
    };
    let package = Package::read(&test_packages::archive("capability-example")).unwrap();
    let id = &package.manifest.id;
    let mut manager = Manager::open(root.path().into(), env(a.path())).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let Value::Resource(first) = probe(&mut manager, id, Op::OpenData).unwrap() else {
        panic!("resource expected")
    };
    // Scoped storage rejects traversal, and workspace handles never grant writes.
    assert_eq!(
        probe(
            &mut manager,
            id,
            Op::ReadFile {
                handle: first.clone(),
                path: "../state.json".into()
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidPath
    );
    let Value::Resource(workspace) = probe(&mut manager, id, Op::OpenWorkspace).unwrap() else {
        panic!("resource expected")
    };
    assert_eq!(
        probe(
            &mut manager,
            id,
            Op::WriteFile {
                handle: workspace,
                path: "forbidden.txt".into(),
                bytes: vec![]
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::PermissionDenied
    );
    probe(
        &mut manager,
        id,
        Op::WriteFile {
            handle: first.clone(),
            path: "owned.txt".into(),
            bytes: b"owner A".to_vec(),
        },
    )
    .unwrap();
    manager.switch_workspace(env(b.path()), true).unwrap();
    let read = || Op::ReadFile {
        handle: first.clone(),
        path: "owned.txt".into(),
    };
    assert_eq!(
        probe(&mut manager, id, read()).unwrap_err().code,
        ErrorCode::InvalidHandle
    );
    let Value::Resource(second) = probe(&mut manager, id, Op::OpenData).unwrap() else {
        panic!("resource expected")
    };
    probe(
        &mut manager,
        id,
        Op::CloseResource {
            handle: second.clone(),
        },
    )
    .unwrap();
    assert_eq!(
        probe(
            &mut manager,
            id,
            Op::ReadFile {
                handle: second,
                path: "owned.txt".into()
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidHandle
    );
    manager
        .close_workspace(&b.path().display().to_string())
        .unwrap();
    assert!(manager.live.is_empty());
    manager.switch_workspace(env(a.path()), true).unwrap();
    assert!(
        matches!(probe(&mut manager, id, read()).unwrap(), Value::Bytes(bytes) if bytes == b"owner A")
    );
    manager
        .close_workspace(&a.path().display().to_string())
        .unwrap();
    assert_eq!(manager.resource_count(), 0);
    assert!(
        manager
            .invoke_command(id, "scope-probe", json!(null))
            .is_err()
    );
    manager.switch_workspace(env(a.path()), true).unwrap();
    assert_eq!(
        probe(&mut manager, id, read()).unwrap_err().code,
        ErrorCode::InvalidHandle
    );
    manager.switch_workspace(env(b.path()), true).unwrap();
    manager.disable(id).unwrap();
    assert_eq!(manager.resource_count(), 0);
    manager.uninstall(id, true).unwrap();
    assert!(manager.installed.is_empty());
}

/// A project override must not cancel another workspace's live ownership or background state.
#[test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn project_disable_preserves_other_workspace_handles() {
    use plugin_runtime::plugin_protocol::api::{Operation as Op, Value};
    let root = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let env = |path: &std::path::Path| Environment {
        workspace: path.display().to_string(),
        ..Default::default()
    };
    let package = scoped_package(false);
    let id = &package.manifest.id;
    let mut manager = Manager::open(root.path().into(), env(a.path())).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    manager.disable(id).unwrap();
    manager.set_project_enabled(id, true).unwrap();
    let Value::Resource(first) = probe(&mut manager, id, Op::OpenData).unwrap() else {
        panic!("resource expected")
    };
    manager.switch_workspace(env(b.path()), true).unwrap();
    manager.set_project_enabled(id, true).unwrap();
    manager.set_project_enabled(id, false).unwrap();
    manager.switch_workspace(env(a.path()), true).unwrap();
    probe(&mut manager, id, Op::CloseResource { handle: first }).unwrap();
    // Package replacement must preserve project enablement across canonical path aliases too.
    manager
        .close_workspace(&b.path().display().to_string())
        .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    assert!(!manager.installed[id].enabled);
    assert!(manager.published_entries()[0].enabled);
    probe(&mut manager, id, Op::OpenData).unwrap();
}

/// A failed registry commit must restore an owner whose subsequent successful writes reach disk.
#[test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn failed_update_restores_durable_writes_and_trust_revocation_skips_guest_checkpoint() {
    use plugin_runtime::plugin_protocol::api::{Operation as Op, Value};
    let root = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let env = Environment {
        workspace: workspace.path().display().to_string(),
        ..Default::default()
    };
    let package = scoped_package(false);
    let id = &package.manifest.id;
    let mut manager = Manager::open(root.path().into(), env.clone()).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    // The real persistence boundary fails after activation, forcing the normal update rollback.
    let registry = root.path().join("registry.json");
    let saved = std::fs::read(&registry).unwrap();
    std::fs::remove_file(&registry).unwrap();
    std::fs::create_dir(&registry).unwrap();
    assert!(
        manager
            .install(&package, package.manifest.permissions.clone())
            .is_err()
    );
    std::fs::remove_dir(&registry).unwrap();
    std::fs::write(&registry, saved).unwrap();
    let Value::Resource(handle) = probe(&mut manager, id, Op::OpenData).unwrap() else {
        panic!("resource expected")
    };
    probe(
        &mut manager,
        id,
        Op::WriteFile {
            handle,
            path: "durable.txt".into(),
            bytes: b"after rollback".to_vec(),
        },
    )
    .unwrap();
    let checkpoint = manager
        .data_directory(id)
        .parent()
        .unwrap()
        .join("state.json");
    // A sentinel makes any unexpected guest checkpoint observable without a special host test API.
    std::fs::write(&checkpoint, b"last good checkpoint").unwrap();
    manager.set_workspace_trust(false).unwrap();
    assert_eq!(std::fs::read(&checkpoint).unwrap(), b"last good checkpoint");
    assert_eq!(manager.resource_count(), 0);
    std::fs::remove_file(checkpoint).unwrap();
    drop(manager);
    let mut reopened = Manager::open(root.path().into(), env).unwrap();
    let Value::Resource(handle) = probe(&mut reopened, id, Op::OpenData).unwrap() else {
        panic!("resource expected")
    };
    assert!(
        matches!(probe(&mut reopened, id, Op::ReadFile { handle, path:"durable.txt".into() }).unwrap(), Value::Bytes(bytes) if bytes == b"after rollback")
    );
}

/// Reuse the independently built guest with declarations changed only at the public package boundary.
fn scoped_package(application: bool) -> Package {
    use std::io::{Cursor, Write};
    let mut package = Package::read(&test_packages::archive("capability-example")).unwrap();
    if application {
        package.manifest.id = "application-example".into();
        package.manifest.scope = plugin_runtime::plugin_protocol::api::InstanceScope::Application;
    }
    package.files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&package.manifest).unwrap(),
    );
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in package.files {
        archive
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(&bytes).unwrap();
    }
    Package::from_bytes(&archive.finish().unwrap().into_inner()).unwrap()
}

/// Application ownership is explicit: it survives workspace closure without borrowing project access.
#[test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn application_instance_survives_workspace_closure_and_restricted_workspace_cannot_grant_access() {
    use plugin_runtime::plugin_protocol::api::{ErrorCode, Operation as Op, Value};
    let root = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let env = |path: &std::path::Path| Environment {
        workspace: path.display().to_string(),
        ..Default::default()
    };
    let package = scoped_package(true);
    let id = &package.manifest.id;
    let mut manager = Manager::open_with_trust(root.path().into(), env(a.path()), false).unwrap();
    assert!(
        manager
            .install(&package, package.manifest.permissions.clone())
            .is_err()
    );
    manager.set_workspace_trust(true).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    assert_eq!(
        probe(&mut manager, id, Op::OpenWorkspace).unwrap_err().code,
        ErrorCode::PermissionDenied
    );
    let Value::Resource(handle) = probe(&mut manager, id, Op::OpenData).unwrap() else {
        panic!("resource expected")
    };
    probe(
        &mut manager,
        id,
        Op::WriteFile {
            handle: handle.clone(),
            path: "app.txt".into(),
            bytes: b"application data".to_vec(),
        },
    )
    .unwrap();
    manager.switch_workspace(env(b.path()), false).unwrap();
    assert!(manager.set_project_enabled(id, true).is_err());
    manager
        .close_workspace(&b.path().display().to_string())
        .unwrap();
    assert!(
        matches!(probe(&mut manager, id, Op::ReadFile { handle:handle.clone(), path:"app.txt".into() }).unwrap(), Value::Bytes(bytes) if bytes == b"application data")
    );
    manager.shutdown();
    assert_eq!(manager.resource_count(), 0);
    assert!(
        manager
            .invoke_command(id, "scope-probe", json!(null))
            .is_err()
    );
    drop(manager);
    let mut restricted =
        Manager::open_with_trust(root.path().into(), env(a.path()), false).unwrap();
    assert!(restricted.live.is_empty());
    assert!(restricted.enable(id).is_err());
    restricted.set_workspace_trust(true).unwrap();
    assert_eq!(
        probe(
            &mut restricted,
            id,
            Op::ReadFile {
                handle,
                path: "app.txt".into()
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidHandle
    );
    let Value::Resource(fresh) = probe(&mut restricted, id, Op::OpenData).unwrap() else {
        panic!("resource expected")
    };
    assert!(
        matches!(probe(&mut restricted, id, Op::ReadFile { handle:fresh, path:"app.txt".into() }).unwrap(), Value::Bytes(bytes) if bytes == b"application data")
    );
    restricted.disable(id).unwrap();
    assert_eq!(restricted.resource_count(), 0);
}
