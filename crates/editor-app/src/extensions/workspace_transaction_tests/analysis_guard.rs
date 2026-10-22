//! Native analysis conditions stop pending writes while committed recovery retains its real owner.
use super::*;
use crate::language::navigation::{ContextCapture, ContextSource, LanguageServer, file_uri};
use gpui_base::input::Rope;
use plugin_runtime::plugin_protocol::{diagnostics::Fix, language_context};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// A delayed highlighter recognition cannot close a Control lease already opened as Vue. A genuine
/// language change still revokes it, and a late wire close cannot revoke the replacement lifetime.
#[test]
#[ignore = "build plugin-runtime --example lsp_fixture first"]
fn recognition_preserves_the_actual_control_language_and_retires_only_mismatches() {
    let directory = tempfile::tempdir().unwrap();
    let (mut manager, server) = native_owner(directory.path());
    let uri = file_uri(&directory.path().join("source.vue")).unwrap();
    // Unlike idle push-diagnostic polling, a real definition reply waits for prior sync frames.
    let synchronize = |source: &str| {
        assert_eq!(
            server
                .definitions(uri.clone(), source.into(), lsp_types::Position::new(0, 0))
                .unwrap()
                .len(),
            1
        );
    };
    let control = server.open_document_for_language(uri.clone(), "vue");
    synchronize("old");
    let peer = server
        .open_document_for_language(file_uri(&directory.path().join("peer.vue")).unwrap(), "vue");
    assert!(
        server
            .retire_document_if_language_changed(&uri, "vue")
            .is_none(),
        "the UI's delayed recognition must preserve an already matching Control lease"
    );
    assert!(control.is_active());
    let retired = server
        .retire_document_if_language_changed(&uri, "typescript")
        .unwrap();
    assert!(
        !control.is_active(),
        "a genuinely changed recognition withdraws old authority"
    );
    assert!(peer.is_active(), "another source retains its lease");
    let replacement = server.open_document_for_language(uri.clone(), "typescript");
    synchronize("new");
    // The completed definition request is a wire-order barrier. Inspect lifetime frames as well as
    // request success: bind_document could otherwise conceal an erroneous close by reopening it.
    let lifetime_events = || {
        std::fs::read_to_string(directory.path().join("recovery-wire.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .filter(|message| {
                matches!(
                    message["method"].as_str(),
                    Some("textDocument/didOpen" | "textDocument/didClose")
                )
            })
            .collect::<Vec<_>>()
    };
    let before_late_close = lifetime_events();
    assert_eq!(before_late_close.len(), 3, "open old, close old, open new");
    server.close_document(retired).unwrap();
    assert!(
        replacement.is_active(),
        "a late old close cannot close its replacement"
    );
    synchronize("new");
    assert_eq!(
        lifetime_events(),
        before_late_close,
        "late old close must neither close nor reopen the new wire document"
    );
    server.retire_document(&uri).unwrap();
    assert!(
        !replacement.is_active(),
        "true native close remains unconditional"
    );
    manager.shutdown_checked().unwrap();
}

/// Use one resource-only Package and the real stdio fixture, without compiling or loading a guest.
fn native_owner(root: &Path) -> (Manager, LanguageServer) {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/lsp_fixture.exe")
        .canonicalize()
        .expect("build plugin-runtime --example lsp_fixture first");
    let manifest = serde_json::json!({
        "id":"native-recovery", "name":"Native recovery", "version":"0.1.0", "protocol":7,
        "api":{"base":"^1","required":{"language.lsp":"^1.5","process":"^1",
            "workspace.files":"^1.4","editor.edit":"^1.1","language.editing":"^1","language.control":"^1"}},
        "permissions":["process.service.analysis","editor.read","editor.write","workspace.read"],
        "storage_limit":1024,"contributions":"plugin.toml",
        "services":{"analysis":{"program":executable,"args":[root.join("recovery-wire.jsonl")]}},
        "language_servers":[{"id":"analysis","language":"unfamiliar","service":"analysis","editing":true,
            "related_documents":"fixture/context/all-documents",
            "control_methods":{"fixture/context/all-documents":language_context::related_documents_method()}}]
    });
    let package = Package::from_files(std::collections::BTreeMap::from([
        ("manifest.json".into(), serde_json::to_vec(&manifest).unwrap()),
        ("plugin.toml".into(), b"[plugin]\nid='native-recovery'\nname='Native recovery'\nversion='0.1.0'\nhost_version='^0.1'\n".to_vec()),
    ])).unwrap();
    let mut manager = Manager::open(
        root.join(".native-recovery-profile"),
        Environment {
            workspace: root.display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    crate::extensions::demand_plugin_fixture(
        &mut manager,
        "native-recovery",
        plugin_runtime::DemandReason::Language {
            provider: "native-recovery/analysis".into(),
            role: plugin_runtime::LanguageRole::Lsp,
        },
    )
    .unwrap();
    let service = manager.language_services()["native-recovery/analysis"]
        .as_ref()
        .unwrap()
        .clone();
    (manager, LanguageServer::from_service(service).unwrap())
}

/// Seal both original native inputs through the provider's actual graph and framed connection.
/// Tests revoke the same configuration/source/connection seals that production previews retain.
fn capture_for(
    server: &LanguageServer,
    root: &Path,
    documents: &[api::DocumentVersion; 2],
    global: Arc<AtomicBool>,
    source: Arc<AtomicBool>,
) -> ContextCapture {
    let capture = ContextCapture::new(
        root.into(),
        1,
        documents[0].id.clone(),
        documents
            .iter()
            .zip(["one", "two"])
            .map(|(document, text)| ContextSource {
                document: document.clone(),
                uri: file_uri(&root.join(&document.path)).unwrap(),
                language: "unfamiliar".into(),
                text: Rope::from(text),
            })
            .collect(),
    )
    .with_guard(global)
    .with_source_guard(source);
    assert_eq!(
        server
            .with_context_snapshot(&capture, || server.definitions(
                file_uri(&root.join(&documents[0].path)).unwrap(),
                "one".into(),
                lsp_types::Position::new(0, 1),
            ))
            .unwrap()
            .len(),
        1
    );
    assert!(capture.is_live());
    capture
}

/// A complete public proposal keeps both readonly conditions and the real edit target versions.
fn native_fix(documents: &[api::DocumentVersion; 2], texts: [&str; 2]) -> Fix {
    Fix {
        title: "Two native documents".into(),
        plan: workspace::Plan {
            title: "Two native documents".into(),
            origin: Some(documents[0].clone()),
            conditions: documents.to_vec(),
            changes: documents
                .iter()
                .zip(texts)
                .map(|(document, text)| workspace::Change::EditDocument {
                    document: document.clone(),
                    edits: vec![api::TextEdit {
                        range: api::TextRange { start: 0, end: 3 },
                        text: text.into(),
                    }],
                    selection: api::TextRange {
                        start: text.len(),
                        end: text.len(),
                    },
                })
                .collect(),
        },
    }
}

/// Poll ordinary native callbacks with a fixed deadline; no made-up response advances the transaction.
fn wait_native(
    visual: &mut VisualTestContext,
    label: &str,
    mut ready: impl FnMut(&mut VisualTestContext) -> bool,
) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        visual
            .executor()
            .advance_clock(std::time::Duration::from_millis(10));
        visual.run_until_parked();
        visual.update(|window, cx| window.draw(cx).clear(cx));
        if ready(visual) {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "native recovery did not reach {label}"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

/// Native service and guest plans use the same preview/commit/Undo dispatcher. Revoking an analysis
/// condition before consent must prevent all edits; revoking it after effects must not revoke Undo.
#[gpui::test]
#[ignore = "build plugin-runtime --example lsp_fixture; native Package/stdio with no WASM or TS JIT"]
fn committed_native_recovery_survives_analysis_expiry_but_not_owner_retirement(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        // The fixture's temporary path need not use the editor's canonical Windows prefix.
        // Observe the actual opened incarnation before using exact native path comparisons.
        let opened_path = visual.update(|_, cx| app.read(cx).active_path.clone().unwrap());
        assert_eq!(
            opened_path.canonicalize().unwrap(),
            path.canonicalize().unwrap()
        );
        let path = opened_path.as_path();
        let root = path.parent().unwrap();
        let second_path = root.join("recovery-second.unfamiliar");
        std::fs::write(&second_path, "two").unwrap();
        let second_path = second_path.canonicalize().unwrap();
        let original_disk = std::fs::read_to_string(path).unwrap();
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input("one");
        visual.run_until_parked();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.open_file(second_path.clone(), window, cx);
                app.open_file(path.into(), window, cx);
            })
        });
        // Capture only real text identities after normal file activation/publication has settled.
        wait_native(visual, "both opened text sessions", |visual| {
            visual.update(|_, cx| {
                let app = app.read(cx);
                [path, second_path.as_path()].into_iter().all(|path| {
                    app.tabs
                        .iter()
                        .position(|tab| tab.path() == path)
                        .is_some_and(|index| app.plugin_document_version(index).is_ok())
                })
            })
        });
        let documents = visual.update(|_, cx| {
            let app = app.read(cx);
            [path, second_path.as_path()].map(|path| {
                let index = app.tabs.iter().position(|tab| tab.path() == path).unwrap();
                app.plugin_document_version(index).unwrap()
            })
        });
        let (mut manager, server) = native_owner(root);
        // The original owner launches its actual declared service before issuing any native plan.
        assert_eq!(
            server
                .definitions(
                    file_uri(path).unwrap(),
                    "one".into(),
                    lsp_types::Position::new(0, 1)
                )
                .unwrap()
                .len(),
            1
        );
        let service = manager.language_services()["native-recovery/analysis"]
            .as_ref()
            .unwrap()
            .clone();
        let fix = native_fix(&documents, ["FIRST", "SECOND"]);
        let mut committed_batch = None;
        for scenario in ["configuration", "source", "connection", "committed"] {
            let commit = scenario == "committed";
            let guard = Arc::new(AtomicBool::new(true));
            let source_guard = Arc::new(AtomicBool::new(true));
            let capture = capture_for(
                &server,
                root,
                &documents,
                guard.clone(),
                source_guard.clone(),
            );
            let origin = capture.fix_origin(&server);
            manager
                .submit_language_fix(&origin, documents[0].clone(), fix.clone())
                .unwrap();
            let (owner, request) = manager.take_editor_requests().pop().unwrap();
            let batch = request
                .workspace_request()
                .unwrap()
                .batch()
                .unwrap()
                .clone();
            visual.update(|window, cx| {
                app.update(cx, |app, cx| {
                    app.perform_editor_request(&owner, request.clone(), window, cx)
                })
            });
            wait_native(visual, "preview", |visual| {
                visual.debug_bounds("workspace-plan-confirm").is_some()
            });
            if !commit {
                match scenario {
                    "configuration" => guard.store(false, Ordering::Release),
                    "source" => source_guard.store(false, Ordering::Release),
                    "connection" => {
                        service.stop_processes();
                        assert!(
                            server
                                .definitions(
                                    file_uri(path).unwrap(),
                                    "one".into(),
                                    lsp_types::Position::new(0, 1)
                                )
                                .is_err()
                        );
                    }
                    _ => unreachable!(),
                }
                assert!(
                    !capture.is_live() && !origin.is_live(),
                    "{scenario} must revoke the rendered result"
                );
                wait_native(visual, "revoked preview completion", |_| {
                    !matches!(
                        batch.receipt().state,
                        workspace::BatchState::Preparing
                            | workspace::BatchState::Preview
                            | workspace::BatchState::Applying
                    )
                });
                assert!(request.status().is_terminal());
                assert!(
                    !batch.is_live(),
                    "an uncommitted stale analysis cannot retain recovery authority"
                );
                assert_eq!(text(visual, &app), "one");
                assert_eq!(
                    visual.update(|_, cx| {
                        app.read(cx)
                            .tabs
                            .iter()
                            .find(|tab| tab.path() == second_path)
                            .unwrap()
                            .text
                            .as_ref()
                            .unwrap()
                            .editor
                            .read(cx)
                            .text()
                            .to_string()
                    }),
                    "two",
                    "{scenario} cannot expose a partially applied proposal"
                );
                continue;
            }
            confirm(visual);
            wait_native(visual, "actual committed receipt", |_| {
                request.status().is_terminal()
            });
            assert!(matches!(
                request.status(),
                api::RequestUpdate::Completed { result: Ok(_) }
            ));
            assert_eq!(batch.receipt().state, workspace::BatchState::Complete);
            assert_eq!(text(visual, &app), "FIRST");
            guard.store(false, Ordering::Release);
            source_guard.store(false, Ordering::Release);
            service.stop_processes();
            assert!(
                server
                    .definitions(
                        file_uri(path).unwrap(),
                        "FIRST".into(),
                        lsp_types::Position::new(0, 1)
                    )
                    .is_err()
            );
            assert!(
                batch.is_live(),
                "a consumed analysis condition must not revoke recovery of actual effects"
            );
            wait_native(visual, "group Undo control", |visual| {
                visual.debug_bounds("workspace-batch-undo").is_some()
            });
            let bounds = visual.debug_bounds("workspace-batch-undo").unwrap();
            visual.simulate_click(bounds.center(), Default::default());
            wait_native(visual, "all native targets restored", |_| {
                batch.receipt().state == workspace::BatchState::Undone
            });
            assert_eq!(text(visual, &app), "one");
            let second = visual.update(|_, cx| {
                app.read(cx)
                    .tabs
                    .iter()
                    .find(|tab| tab.path() == second_path)
                    .unwrap()
                    .text
                    .as_ref()
                    .unwrap()
                    .editor
                    .read(cx)
                    .text()
                    .to_string()
            });
            assert_eq!(second, "two");
            committed_batch = Some(batch);
        }
        // A completed noop is not an effect. Its expired analysis must still release this resource,
        // while the independently committed real batch above keeps its original recovery owner.
        let current = visual.update(|_, cx| {
            let app = app.read(cx);
            [path, second_path.as_path()].map(|path| {
                app.plugin_document_version(
                    app.tabs.iter().position(|tab| tab.path() == path).unwrap(),
                )
                .unwrap()
            })
        });
        let noop_guard = Arc::new(AtomicBool::new(true));
        let noop = capture_for(
            &server,
            root,
            &current,
            noop_guard.clone(),
            Arc::new(AtomicBool::new(true)),
        );
        manager
            .submit_language_fix(
                &noop.fix_origin(&server),
                current[0].clone(),
                native_fix(&current, ["one", "two"]),
            )
            .unwrap();
        let (owner, request) = manager.take_editor_requests().pop().unwrap();
        let noop_batch = request
            .workspace_request()
            .unwrap()
            .batch()
            .unwrap()
            .clone();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.perform_editor_request(&owner, request.clone(), window, cx)
            })
        });
        wait_native(visual, "noop preview", |visual| {
            visual.debug_bounds("workspace-plan-confirm").is_some()
        });
        confirm(visual);
        wait_native(visual, "noop completion", |_| {
            request.status().is_terminal()
        });
        assert!(matches!(
            request.status(),
            api::RequestUpdate::Completed { result: Ok(_) }
        ));
        noop_guard.store(false, Ordering::Release);
        assert!(
            !noop_batch.is_live(),
            "a noop cannot consume preparation conditions"
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), original_disk);
        assert_eq!(std::fs::read_to_string(&second_path).unwrap(), "two");
        manager.disable("native-recovery").unwrap();
        assert!(!service.is_active());
        assert!(
            !committed_batch.unwrap().is_live(),
            "owner retirement still revokes committed recovery"
        );
    });
}
