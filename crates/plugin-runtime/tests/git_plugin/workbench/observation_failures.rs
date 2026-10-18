//! Optional Git observations must publish native recovery work before retaining a busy editor request.
use super::*;

/// This native result already contains an unsaved large edit; Git's immutable stages stay small.
/// The host rejects a whole-document snapshot over 256 KiB without changing that document.
fn result_document() -> api::DocumentInfo {
    api::DocumentInfo {
        document: api::DocumentVersion {
            id: "observation-large-native-result".into(),
            path: "merge.txt".into(),
            revision: 7,
        },
        resource: api::ResourceIdentity::Local {
            path: "merge.txt".into(),
        },
        title: "merge.txt".into(),
        access: api::DocumentAccess {
            read: true,
            edit: true,
            save: true,
        },
        dirty: true,
        language: "text".into(),
        encoding: api::DocumentEncoding::Utf8,
        eol: api::DocumentEol::Lf,
        byte_len: 256 * 1024 + 1,
        selection: api::TextRange { start: 0, end: 0 },
        visible_rows: None,
    }
}

/// Retain exactly the public native read while completing ordinary metadata/visibility receipts.
/// No private guest state, fabricated process events or native window is involved.
fn next_read(manager: &mut Manager, info: &api::DocumentInfo) -> plugin_runtime::EditorRequest {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        manager.poll();
        for (_, request) in manager.take_editor_requests() {
            match request.operation().clone() {
                api::EditorOperation::ListDocuments => {
                    request.finish(Ok(api::EditorValue::Documents {
                        documents: vec![info.clone()],
                        active: Some(info.document.clone()),
                    }));
                }
                api::EditorOperation::SetPanelVisibility { panel, visible } => {
                    request.finish(Ok(api::EditorValue::PanelVisibility { panel, visible }));
                }
                api::EditorOperation::OpenDocument { resource } => {
                    assert_eq!(resource, info.resource);
                    request.finish(Ok(api::EditorValue::DocumentOpened(info.clone())));
                }
                api::EditorOperation::ReadDocument { document, range } => {
                    assert_eq!(
                        document, info.document,
                        "read must name the retained native version"
                    );
                    assert!(range.is_none());
                    return request;
                }
                operation => panic!("unexpected native recovery request: {operation:?}"),
            }
        }
        assert!(
            Instant::now() < deadline,
            "visible timer never retried the native result read"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A failed native read may be retried by an unchanged automatic Git sample. Its pending native
/// work must revoke the old enabled controls rather than silently dropping a current user action.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real conflict and native read failure"]
fn git_package_automatic_native_reread_publishes_busy_authority() {
    if !run_isolated_test(
        "workbench::observation_failures::git_package_automatic_native_reread_publishes_busy_authority",
    ) {
        return;
    }
    let repository = isolated_repository();
    let root = repository.path();
    commit_file(root, "merge.txt", "base\n", "base");
    git(root, &["switch", "-c", "topic"]);
    commit_file(root, "merge.txt", "incoming\n", "incoming");
    git(root, &["switch", "main"]);
    commit_file(root, "merge.txt", "current\n", "current");
    let merge = Command::new("git")
        .current_dir(root)
        .args(["merge", "--no-edit", "topic"])
        .output()
        .unwrap();
    assert!(
        !merge.status.success(),
        "fixture must enter a real conflict"
    );
    assert!(root.join(".git/MERGE_HEAD").is_file());
    let index = fs::read(root.join(".git/index")).unwrap();
    let disk = fs::read(root.join("merge.txt")).unwrap();
    let (_profile, mut manager) = activate(&candidate(), root);
    manager
        .event(
            "git",
            Some("history".into()),
            api::Notification::PanelVisibility {
                panel: "history".into(),
                visible: true,
            },
        )
        .unwrap();
    idle(&mut manager);
    manager
        .invoke_command("git", "resolve-file", json!({"path":"merge.txt"}))
        .unwrap();
    let info = result_document();
    next_read(&mut manager, &info).finish(Err(api::Failure::new(
        api::ErrorCode::LimitExceeded,
        "Snapshot exceeds 256 KiB; request a smaller range",
    )));
    // Deliver the failed public receipt before waiting for the real periodic local observation.
    manager.poll();
    let first_failure_revision = manager.live["git"].views["history"].revision;
    assert!(
        manager.live["git"].views["history"]
            .active_node("log")
            .is_some()
    );
    let reread = next_read(&mut manager, &info);
    let scene = &manager.live["git"].views["history"];
    assert!(
        scene.revision > first_failure_revision && scene.active_node("log").is_none(),
        "automatic native reread retained enabled scene {} (failure scene {}); a current input would be silently swallowed by its busy guard",
        scene.revision,
        first_failure_revision,
    );
    reread.finish(Err(api::Failure::new(
        api::ErrorCode::LimitExceeded,
        "Snapshot exceeds 256 KiB; request a smaller range",
    )));
    idle(&mut manager);
    assert!(
        manager.live["git"].views["history"]
            .active_node("log")
            .is_some()
    );
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read(root.join("merge.txt")).unwrap(), disk);
    manager.disable("git").unwrap();
    assert_eq!(manager.resource_count(), 0);
}
