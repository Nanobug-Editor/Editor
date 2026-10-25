//! Index-only changes preserve native unsaved buffers; intent-to-add retains a real stale-index guard.
use super::*;
#[path = "index_edges/commit_panel.rs"]
mod commit_panel;
#[path = "index_edges/commit_selection.rs"]
mod commit_selection;
#[path = "index_edges/commit_submit.rs"]
mod commit_submit;

/// Load the independently packaged candidate without rebuilding a host or embedding guest sources.
fn candidate() -> Package {
    Package::read(Path::new(
        &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate ZIP required"),
    ))
    .unwrap()
}

/// All identity and line-ending configuration is private to this temporary repository.
fn fixture(unborn: bool) -> tempfile::TempDir {
    let repository = tempfile::tempdir().unwrap();
    let root = repository.path();
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "core.autocrlf", "false"]);
    git(root, &["config", "user.name", "Index fixture"]);
    git(root, &["config", "user.email", "index@example.invalid"]);
    if !unborn {
        std::fs::write(root.join("tracked.txt"), "base\n").unwrap();
        git(root, &["add", "--", "tracked.txt"]);
        git(root, &["commit", "-m", "base"]);
    }
    repository
}

/// A dirty native session can intentionally differ from disk. No action in these tests may save it.
fn dirty_document(path: &str) -> api::DocumentInfo {
    api::DocumentInfo {
        document: api::DocumentVersion {
            id: "native-unsaved-buffer".into(),
            path: path.into(),
            revision: 9,
        },
        resource: api::ResourceIdentity::Local { path: path.into() },
        title: path.into(),
        access: api::DocumentAccess {
            read: true,
            edit: true,
            save: true,
        },
        dirty: true,
        language: "text".into(),
        encoding: api::DocumentEncoding::Utf8,
        eol: api::DocumentEol::Lf,
        byte_len: 100,
        selection: api::TextRange { start: 3, end: 3 },
        visible_rows: None,
    }
}

/// Respond to the public native editor seam with a real unsaved-document observation, counting
/// ListDocuments calls to distinguish an index-only route from an erroneously blocked save check.
fn idle_with_documents(manager: &mut Manager, documents: &[api::DocumentInfo]) -> usize {
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut settled = 0;
    let mut lists = 0;
    loop {
        manager.poll();
        assert_commit_tree_present(manager);
        for (_, request) in manager.take_editor_requests() {
            match request.operation() {
                api::EditorOperation::ListDocuments => {
                    lists += 1;
                    request.finish(Ok(api::EditorValue::Documents {
                        documents: documents.to_vec(),
                        active: documents.first().map(|document| document.document.clone()),
                    }));
                }
                api::EditorOperation::SaveDocument { .. }
                | api::EditorOperation::ReplaceDocumentRange { .. }
                | api::EditorOperation::ApplyEdits { .. } => {
                    panic!("an index operation cannot save or edit a native document")
                }
                // Opening the readonly comparison is irrelevant to the cached patch bytes. This
                // runtime fixture deliberately has no native text window, as in the parent tests.
                _ => request.finish(Err(api::Failure::new(
                    api::ErrorCode::UnsupportedOperation,
                    "No native comparison window in the index fixture",
                ))),
            }
        }
        manager.poll();
        // Inspect every process/document receipt, including the transient guarded-write phase.
        // Waiting only for the final index would miss a tree-wide disabled/re-enabled flash.
        assert_commit_tree_present(manager);
        settled = if manager.live["git"].process_count() == 0 {
            settled + 1
        } else {
            0
        };
        if settled >= 8 {
            return lists;
        }
        assert!(Instant::now() < deadline, "index action did not settle");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A modal temporarily blocks root interaction without removing or disabling the underlying tree.
/// Keep checking root admission when no modal exists, and visible root metadata when one does.
fn assert_commit_tree_present(manager: &Manager) {
    let document = &manager.live["git"].views["staging"];
    if document.dialog.is_none() {
        assert!(
            document.active_node("staging").is_some(),
            "index operation withdrew the whole commit tree"
        );
    } else {
        let mut tree_present = false;
        document.root.visit(&mut |node| {
            if node.id == "staging" && !node.disabled && matches!(node.kind, ui::Kind::Tree { .. })
            {
                tree_present = true;
            }
        });
        assert!(
            tree_present,
            "submission modal removed or disabled the retained tree"
        );
    }
}

/// A staged file can be unstaged both before and after HEAD exists without consuming an unsaved
/// buffer. Both actual index bytes and worktree bytes are observed through independent Git/files.
#[test]
#[ignore = "build independent Git-for-Nanobug candidate; set NANOBUG_GIT_CANDIDATE"]
fn git_package_unstage_with_dirty_buffer_changes_only_the_index_including_unborn_head() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "index_edges::git_package_unstage_with_dirty_buffer_changes_only_the_index_including_unborn_head",
    ) {
        return;
    }
    for unborn in [false, true] {
        let repository = fixture(unborn);
        let root = repository.path();
        std::fs::write(root.join("tracked.txt"), "staged\n").unwrap();
        git(root, &["add", "--", "tracked.txt"]);
        std::fs::write(root.join("tracked.txt"), "disk after stage\n").unwrap();
        let (_profile, mut manager) = activate(&candidate(), root);
        manager
            .invoke_command("git", "unstage", json!({"path":"tracked.txt"}))
            .unwrap();
        assert_eq!(
            idle_with_documents(&mut manager, &[dirty_document("tracked.txt")]),
            0
        );
        if unborn {
            assert_eq!(git(root, &["ls-files", "--", "tracked.txt"]), "");
        } else {
            assert_eq!(git(root, &["show", ":tracked.txt"]), "base\n");
        }
        assert_eq!(
            std::fs::read_to_string(root.join("tracked.txt")).unwrap(),
            "disk after stage\n"
        );
        manager.disable("git").unwrap();
    }
}

/// A reverse cached hunk uses HEAD/index alone, even if the native worktree session is dirty.
/// The ordinary worktree hunk still requires a save and must not silently stage stale disk bytes.
#[test]
#[ignore = "build independent Git-for-Nanobug candidate; set NANOBUG_GIT_CANDIDATE"]
fn git_package_unstage_hunk_ignores_dirty_native_buffer_but_staging_still_requires_saving() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "index_edges::git_package_unstage_hunk_ignores_dirty_native_buffer_but_staging_still_requires_saving",
    ) {
        return;
    }
    let repository = fixture(false);
    let root = repository.path();
    let baseline = (0..40)
        .map(|row| format!("line {row}\n"))
        .collect::<String>();
    std::fs::write(root.join("tracked.txt"), &baseline).unwrap();
    git(root, &["add", "--", "tracked.txt"]);
    git(root, &["commit", "-m", "hunk baseline"]);
    let modified = baseline
        .replace("line 1\n", "first change\n")
        .replace("line 35\n", "last change\n");
    std::fs::write(root.join("tracked.txt"), &modified).unwrap();
    git(root, &["add", "--", "tracked.txt"]);
    let (_profile, mut manager) = activate(&candidate(), root);
    manager
        .invoke_command("git", "diff-index", json!({"path":"tracked.txt"}))
        .unwrap();
    idle_with_documents(&mut manager, &[dirty_document("tracked.txt")]);
    click(&mut manager, "staging", "commit-hunks");
    let scene = &manager.live["git"].views["staging"];
    assert!(scene.active_node("hunk-1").is_some());
    let revision = scene.revision;
    manager
        .event(
            "git",
            Some("staging".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: "hunk-0".into(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
    assert_eq!(
        idle_with_documents(&mut manager, &[dirty_document("tracked.txt")]),
        0
    );
    let partially_staged = baseline.replace("line 35\n", "last change\n");
    assert_eq!(git(root, &["show", ":tracked.txt"]), partially_staged);
    assert_eq!(
        std::fs::read_to_string(root.join("tracked.txt")).unwrap(),
        modified
    );

    manager
        .invoke_command("git", "refresh-hunks", json!({"path":"tracked.txt"}))
        .unwrap();
    idle_with_documents(&mut manager, &[]);
    click(&mut manager, "staging", "commit-hunks");
    let revision = manager.live["git"].views["staging"].revision;
    manager
        .event(
            "git",
            Some("staging".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: "hunk-0".into(),
                action: ui::Action::Click,
            }),
        )
        .unwrap();
    assert_eq!(
        idle_with_documents(&mut manager, &[dirty_document("tracked.txt")]),
        1
    );
    assert_eq!(git(root, &["show", ":tracked.txt"]), partially_staged);
    assert_eq!(
        std::fs::read_to_string(root.join("tracked.txt")).unwrap(),
        modified
    );
    manager.disable("git").unwrap();
}

/// Porcelain reports an intent-to-add entry with zero mode/OID, while ls-files still contains a
/// real empty blob. Staging succeeds, and a later externally staged change invalidates approval.
#[test]
#[ignore = "build independent Git-for-Nanobug candidate; set NANOBUG_GIT_CANDIDATE"]
fn git_package_stages_intent_to_add_literal_paths_and_rejects_an_external_index_change() {
    // Keep factory and Manager/native Git children in one private configuration process.
    if !run_isolated_test(
        "index_edges::git_package_stages_intent_to_add_literal_paths_and_rejects_an_external_index_change",
    ) {
        return;
    }
    let repository = fixture(false);
    let root = repository.path();
    for (path, text) in [("[ab].txt", "intent 中😀\n"), ("a.txt", "sibling\n")] {
        std::fs::write(root.join(path), text).unwrap();
        git(root, &["--literal-pathspecs", "add", "-N", "--", path]);
    }
    let (_profile, mut manager) = activate(&candidate(), root);
    manager
        .invoke_command("git", "stage", json!({"path":"[ab].txt"}))
        .unwrap();
    assert_eq!(idle_with_documents(&mut manager, &[]), 1);
    assert_eq!(git(root, &["show", ":[ab].txt"]), "intent 中😀\n");
    assert_eq!(
        git(root, &["show", ":a.txt"]),
        "",
        "a pathspec sibling must stay intent-to-add"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("[ab].txt")).unwrap(),
        "intent 中😀\n"
    );
    // Keep the plugin's original .A observation, then independently replace its index precondition.
    git(root, &["add", "--", "a.txt"]);
    std::fs::write(root.join("a.txt"), "later disk bytes\n").unwrap();
    manager
        .invoke_command("git", "stage", json!({"path":"a.txt"}))
        .unwrap();
    idle_with_documents(&mut manager, &[]);
    assert_eq!(git(root, &["show", ":a.txt"]), "sibling\n");
    assert_eq!(
        std::fs::read_to_string(root.join("a.txt")).unwrap(),
        "later disk bytes\n"
    );
    manager.disable("git").unwrap();
}
