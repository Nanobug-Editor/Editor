//! Real interactive rebase recovery crosses the packaged helper and public native-editor seam.
use super::*;
use std::path::PathBuf;

/// This fixture owns native text: proposals change memory, and only SaveDocument writes the
/// working tree. Every read/edit/save must name the exact current session revision.
struct EditorFixture {
    root: PathBuf,
    info: Option<api::DocumentInfo>,
    text: String,
    sequence: u64,
    edits: usize,
    saves: usize,
    lists: usize,
}

impl EditorFixture {
    fn new(root: &Path) -> Self {
        Self {
            root: root.into(),
            info: None,
            text: String::new(),
            sequence: 0,
            edits: 0,
            saves: 0,
            lists: 0,
        }
    }

    /// Publish production's ordered metadata ingress; document text only travels in read receipts.
    fn publish(&mut self, manager: &mut Manager, kind: api::DocumentEventKind) {
        self.sequence += 1;
        manager.document_event(api::DocumentEvent {
            sequence: self.sequence,
            resource: Some(api::ResourceIdentity::Local {
                path: "merge.txt".into(),
            }),
            kind,
        });
    }

    /// Advance one native transaction while retaining one authoritative fixture buffer.
    fn changed(&mut self, selection: api::TextRange) -> api::DocumentInfo {
        let info = self.info.as_mut().expect("result document opened");
        info.document.revision += 1;
        info.dirty = true;
        info.byte_len = self.text.len();
        info.eol = api::DocumentEol::of(&self.text);
        info.selection = selection;
        info.clone()
    }

    /// A manual native edit remains unsaved and reports a new revision through the public stream.
    fn draft(&mut self, manager: &mut Manager, text: &str) {
        self.text = text.into();
        let info = self.changed(api::TextRange { start: 0, end: 0 });
        self.publish(manager, api::DocumentEventKind::ContentChanged(info));
        self.idle(manager);
    }

    /// Drain real process callbacks and versioned editor receipts before the next UI action.
    fn idle(&mut self, manager: &mut Manager) {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut settled = 0;
        loop {
            manager.poll();
            let requests = manager.take_editor_requests();
            let had_requests = !requests.is_empty();
            for (_, request) in requests {
                self.reply(manager, request);
            }
            manager.poll();
            assert!(
                manager.live["git"].error.is_none(),
                "Git recovery was retired before its next UI action: {:?}",
                manager.diagnostics("git")
            );
            settled = if manager.live["git"].process_count() == 0 && !had_requests {
                settled + 1
            } else {
                0
            };
            if settled >= 8 {
                return;
            }
            assert!(Instant::now() < deadline, "rebase recovery did not settle");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Opening the result again retains the same native buffer instead of reloading an unsaved draft.
    fn open(&mut self, path: String) -> api::DocumentInfo {
        assert_eq!(path, "merge.txt");
        if self.info.is_none() {
            self.text = std::fs::read_to_string(self.root.join(&path)).unwrap();
            self.info = Some(api::DocumentInfo {
                document: api::DocumentVersion {
                    id: "rebase-result-session".into(),
                    path: path.clone(),
                    revision: 1,
                },
                resource: api::ResourceIdentity::Local { path: path.clone() },
                title: path,
                access: api::DocumentAccess {
                    read: true,
                    edit: true,
                    save: true,
                },
                dirty: false,
                language: "text".into(),
                encoding: api::DocumentEncoding::Utf8,
                eol: api::DocumentEol::of(&self.text),
                byte_len: self.text.len(),
                selection: api::TextRange { start: 0, end: 0 },
                visible_rows: None,
            });
        }
        self.info.clone().unwrap()
    }

    /// Reply through the public EditorRequest seam; no fixture calls a private guest state method.
    fn reply(&mut self, manager: &mut Manager, request: plugin_runtime::EditorRequest) {
        match request.operation().clone() {
            api::EditorOperation::ListDocuments => {
                self.lists += 1;
                request.finish(Ok(api::EditorValue::Documents {
                    documents: self.info.iter().cloned().collect(),
                    active: self.info.as_ref().map(|info| info.document.clone()),
                }));
            }
            api::EditorOperation::SetPanelVisibility { panel, visible } => {
                request.finish(Ok(api::EditorValue::PanelVisibility { panel, visible }));
            }
            api::EditorOperation::OpenDocument {
                resource: api::ResourceIdentity::Local { path },
            } => {
                let info = self.open(path);
                request.finish(Ok(api::EditorValue::DocumentOpened(info.clone())));
                self.publish(manager, api::DocumentEventKind::Opened(info));
            }
            api::EditorOperation::ReadDocument { document, range } => {
                assert!(
                    range.is_none(),
                    "small fixture uses complete native snapshots"
                );
                let info = self.info.clone().unwrap();
                assert_eq!(
                    document, info.document,
                    "read cannot follow a stale revision"
                );
                request.finish(Ok(api::EditorValue::DocumentSnapshot(
                    api::DocumentSnapshot {
                        info,
                        range: api::TextRange {
                            start: 0,
                            end: self.text.len(),
                        },
                        text: self.text.clone(),
                    },
                )));
            }
            api::EditorOperation::ApplyEdits {
                document,
                mut edits,
                selection,
            } => {
                assert_eq!(document, self.info.as_ref().unwrap().document);
                edits.sort_by_key(|edit| edit.range.start);
                assert!(
                    edits
                        .windows(2)
                        .all(|pair| pair[0].range.end <= pair[1].range.start)
                );
                for edit in edits.into_iter().rev() {
                    assert!(self.text.is_char_boundary(edit.range.start));
                    assert!(self.text.is_char_boundary(edit.range.end));
                    self.text
                        .replace_range(edit.range.start..edit.range.end, &edit.text);
                }
                assert!(selection.end <= self.text.len());
                let info = self.changed(selection);
                self.edits += 1;
                request.finish(Ok(api::EditorValue::Edited {
                    document: info.document.clone(),
                    selection,
                }));
                self.publish(manager, api::DocumentEventKind::ContentChanged(info));
            }
            api::EditorOperation::SaveDocument { document } => {
                let info = self.info.as_mut().unwrap();
                assert_eq!(document, info.document, "save cannot use a stale result");
                assert!(info.dirty, "UI must apply an actual conflict choice first");
                std::fs::write(self.root.join(&document.path), &self.text).unwrap();
                info.dirty = false;
                self.saves += 1;
                request.finish(Ok(api::EditorValue::Saved {
                    document: document.clone(),
                }));
                self.publish(
                    manager,
                    api::DocumentEventKind::DidSave {
                        document,
                        result: Ok(()),
                    },
                );
            }
            operation => panic!("unexpected rebase fixture editor operation: {operation:?}"),
        }
    }

    /// Send a currently published UI action, then serve its native editor requests.
    fn click(&mut self, manager: &mut Manager, panel: &str, node: &str) {
        let revision = manager.live["git"].views[panel].revision;
        manager
            .event(
                "git",
                Some(panel.into()),
                api::Notification::Ui(ui::UiEvent {
                    revision,
                    node: node.into(),
                    action: ui::Action::Click,
                }),
            )
            .unwrap();
        self.idle(manager);
    }
}

/// B before its prerequisite A causes one conflict; dropping A avoids a second same-path conflict.
/// C remains pending so tampering tests the native helper's complete continuation validation.
struct RebaseFixture {
    repository: tempfile::TempDir,
    _profile: tempfile::TempDir,
    manager: Manager,
    editor: EditorFixture,
    base: String,
    original_head: String,
    plan_file: PathBuf,
}

impl RebaseFixture {
    fn conflict() -> Self {
        let package = Package::read(Path::new(
            &std::env::var_os("NANOBUG_GIT_CANDIDATE")
                .expect("candidate ZIP with native rebase dependency required"),
        ))
        .unwrap();
        let repository = tempfile::tempdir().unwrap();
        let root = repository.path();
        git(root, &["init", "-b", "main"]);
        git(root, &["config", "core.autocrlf", "false"]);
        git(root, &["config", "user.name", "Rebase recovery fixture"]);
        git(root, &["config", "user.email", "rebase@example.invalid"]);
        for (title, text) in [("base", "base\n"), ("A", "A\n"), ("B", "B 中😀\n")] {
            std::fs::write(root.join("merge.txt"), text).unwrap();
            git(root, &["add", "--", "merge.txt"]);
            git(root, &["commit", "-m", title]);
        }
        let base = git(root, &["rev-parse", "HEAD~2"]).trim().to_owned();
        std::fs::write(root.join("tail.txt"), "C\n").unwrap();
        git(root, &["add", "--", "tail.txt"]);
        git(root, &["commit", "-m", "C"]);
        let original_head = git(root, &["rev-parse", "HEAD"]);
        let (profile, mut manager) = activate(&package, root);
        let plan_file = Manager::persisted_data_directory(
            &profile.path().join("plugins"),
            &package.manifest,
            &root.display().to_string(),
        )
        .unwrap()
        .join("rebase-plan.json");
        manager
            .invoke_command("git", "rebase-plan", json!({"target":base}))
            .unwrap();
        idle(&mut manager);
        click(&mut manager, "history", "rebase-up-1");
        ui_action(
            &mut manager,
            "rebase-action-1",
            ui::Action::Select("drop".into()),
        );
        click(&mut manager, "history", "rebase-run");
        click(&mut manager, "history", "confirm-command");
        assert!(
            root.join(".git/rebase-merge").is_dir(),
            "approved real rebase must stop in conflict"
        );
        assert!(!git(root, &["ls-files", "--unmerged", "--", "merge.txt"]).is_empty());
        assert_eq!(git(root, &["rev-parse", "main"]), original_head);
        assert!(
            !root.join("tail.txt").exists(),
            "C remains unapplied at the first conflict"
        );
        let mut editor = EditorFixture::new(root);
        manager
            .invoke_command("git", "resolve-file", json!({"path":"merge.txt"}))
            .unwrap();
        editor.idle(&mut manager);
        Self {
            repository,
            _profile: profile,
            manager,
            editor,
            base,
            original_head,
            plan_file,
        }
    }

    /// The block UI proposes the edit; Save and resolve must write the native result before Git add.
    fn resolve(&mut self) {
        self.editor
            .click(&mut self.manager, "conflicts", "merge-block-0-right");
        assert_eq!(self.editor.edits, 1);
        assert_eq!(self.editor.text, "B 中😀\n");
        assert!(self.editor.info.as_ref().unwrap().dirty);
        assert!(
            std::fs::read_to_string(self.repository.path().join("merge.txt"))
                .unwrap()
                .contains("<<<<<<<")
        );
        assert!(!git(self.repository.path(), &["ls-files", "--unmerged"]).is_empty());
        self.editor
            .click(&mut self.manager, "conflicts", "resolved");
        assert_eq!(self.editor.saves, 1);
        assert!(!self.editor.info.as_ref().unwrap().dirty);
        assert_eq!(
            git(self.repository.path(), &["show", ":merge.txt"]),
            "B 中😀\n"
        );
        assert!(git(self.repository.path(), &["ls-files", "--unmerged"]).is_empty());
        assert!(
            self.repository.path().join(".git/rebase-merge").exists(),
            "saving one file cannot continue rebase"
        );
    }
}

/// A real conflict produces Git's own done/todo files; the approved helper must accept those records.
#[test]
#[ignore = "build Git-for-Nanobug with its native rebase dependency; set NANOBUG_GIT_CANDIDATE"]
fn git_package_interactive_rebase_continues_after_native_conflict_edit_save_and_stage() {
    let mut fixture = RebaseFixture::conflict();
    fixture.resolve();
    let lists = fixture.editor.lists;
    fixture
        .editor
        .click(&mut fixture.manager, "conflicts", "continue");
    let root = fixture.repository.path();
    assert!(
        fixture.editor.lists > lists,
        "resume must check current native dirty documents"
    );
    assert!(
        !root.join(".git/rebase-merge").exists(),
        "helper must accept the unchanged approved resume"
    );
    assert_eq!(git(root, &["symbolic-ref", "HEAD"]), "refs/heads/main\n");
    assert_ne!(git(root, &["rev-parse", "HEAD"]), fixture.original_head);
    assert_eq!(
        git(
            root,
            &[
                "log",
                "--reverse",
                "--format=%s",
                &format!("{}..HEAD", fixture.base)
            ]
        ),
        "B\nC\n"
    );
    assert_eq!(git(root, &["show", "HEAD:merge.txt"]), "B 中😀\n");
    assert_eq!(
        std::fs::read_to_string(root.join("tail.txt")).unwrap(),
        "C\n"
    );
    assert!(git(root, &["status", "--porcelain"]).is_empty());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(&fixture.plan_file).unwrap())
            .unwrap(),
        json!(null)
    );
    fixture.manager.disable("git").unwrap();
}

/// A persisted todo cannot lend unreviewed exec authority to the saved plan's Continue action.
#[test]
#[ignore = "build Git-for-Nanobug with its native rebase dependency; set NANOBUG_GIT_CANDIDATE"]
fn git_package_interactive_rebase_rejects_injected_exec_and_retains_result_plan_and_git_state() {
    let mut fixture = RebaseFixture::conflict();
    fixture.resolve();
    let root = fixture.repository.path();
    let todo_file = root.join(".git/rebase-merge/git-rebase-todo");
    let todo = format!(
        "exec echo unapproved > unapproved-exec.txt\n{}",
        std::fs::read_to_string(&todo_file).unwrap()
    );
    std::fs::write(&todo_file, &todo).unwrap();
    let done = std::fs::read(root.join(".git/rebase-merge/done")).unwrap();
    let plan = std::fs::read(&fixture.plan_file).unwrap();
    let head = git(root, &["rev-parse", "HEAD"]);
    let index = git(root, &["ls-files", "--stage"]);
    let staged = git(root, &["diff", "--cached", "--binary"]);
    let result_info = fixture.editor.info.clone();
    let lists = fixture.editor.lists;
    fixture
        .editor
        .click(&mut fixture.manager, "conflicts", "continue");
    assert!(fixture.editor.lists > lists);
    let scene = &fixture.manager.live["git"].views["conflicts"];
    let ui::Kind::Text { text } = &scene.active_node("window-status").unwrap().kind else {
        panic!("native recovery status must contain the helper rejection")
    };
    assert!(
        text.contains("Active rebase identity changed") || text.contains("active todo changed"),
        "helper must reject modified todo: {text}"
    );
    assert_eq!(git(root, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(root, &["rev-parse", "main"]), fixture.original_head);
    assert_eq!(git(root, &["ls-files", "--stage"]), index);
    assert_eq!(git(root, &["diff", "--cached", "--binary"]), staged);
    assert_eq!(std::fs::read_to_string(&todo_file).unwrap(), todo);
    assert_eq!(
        std::fs::read(root.join(".git/rebase-merge/done")).unwrap(),
        done
    );
    assert_eq!(std::fs::read(&fixture.plan_file).unwrap(), plan);
    assert!(!root.join("unapproved-exec.txt").exists());
    assert!(!root.join("tail.txt").exists());
    assert_eq!(
        fixture.editor.info, result_info,
        "rejected helper cannot retire or mutate result session"
    );
    assert_eq!(fixture.editor.text, "B 中😀\n");
    assert_eq!(fixture.editor.saves, 1);

    // A later unsaved native draft also survives: Continue stops at the dirty-buffer gate without
    // hiding the active rebase, executing the injected command, or saving this new draft.
    fixture
        .editor
        .draft(&mut fixture.manager, "unsaved resolution draft 中😀\n");
    let draft_info = fixture.editor.info.clone();
    fixture
        .editor
        .click(&mut fixture.manager, "conflicts", "continue");
    assert_eq!(fixture.editor.info, draft_info);
    assert!(fixture.editor.info.as_ref().unwrap().dirty);
    assert_eq!(fixture.editor.text, "unsaved resolution draft 中😀\n");
    assert_eq!(fixture.editor.saves, 1);
    assert_eq!(
        std::fs::read_to_string(root.join("merge.txt")).unwrap(),
        "B 中😀\n"
    );
    assert_eq!(git(root, &["ls-files", "--stage"]), index);
    assert_eq!(git(root, &["rev-parse", "HEAD"]), head);
    assert_eq!(std::fs::read_to_string(&todo_file).unwrap(), todo);
    assert_eq!(std::fs::read(&fixture.plan_file).unwrap(), plan);
    assert!(!root.join("unapproved-exec.txt").exists());
    fixture.manager.disable("git").unwrap();
}
