//! Actual installed TypeScript project choices use native QuickPick, source epochs and private CAS.
use super::vue_test_support::{Fixture, setup_for_workspace, shutdown, wait};
use super::*;
use crate::extensions::protocol;
use crate::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};

/// One shared file has distinct real types in two configured graphs. Cancelling publishes nothing;
/// explicit reselect changes A alone, and navigation never overwrites the durable local preference.
#[gpui::test]
#[ignore = "set NANOBUG_TYPESCRIPT_PACKAGE to the independently built current SDK candidate"]
fn shared_typescript_choice_is_native_scoped_and_retains_navigation_provenance(
    cx: &mut TestAppContext,
) {
    let workspace = tempfile::tempdir().unwrap();
    let shared_text = "import { value } from 'context-value';\nexport const selected = value;\n";
    for name in ["left", "right"] {
        std::fs::create_dir(workspace.path().join(name)).unwrap();
        std::fs::write(workspace.path().join(name).join("tsconfig.json"),
            r#"{"compilerOptions":{"strict":true,"baseUrl":".","paths":{"context-value":["./value.ts"]}},"files":["entry.ts","../shared.ts"]}"#).unwrap();
        std::fs::write(
            workspace.path().join(name).join("entry.ts"),
            format!("import {{ selected }} from '../shared';\nexport const {name} = selected;\n"),
        )
        .unwrap();
        std::fs::write(
            workspace.path().join(name).join("value.ts"),
            if name == "left" {
                "export const value = 'left' as const;\n"
            } else {
                "export const value = 42 as const;\n"
            },
        )
        .unwrap();
    }
    std::fs::write(workspace.path().join("shared.ts"), shared_text).unwrap();
    let (fixture, visual) = setup_for_workspace(
        cx,
        &["NANOBUG_TYPESCRIPT_PACKAGE"],
        workspace.path().to_owned(),
    );
    let shared = fixture.workspace.join("shared.ts").canonicalize().unwrap();
    let left = fixture
        .workspace
        .join("left/entry.ts")
        .canonicalize()
        .unwrap();
    let right = fixture
        .workspace
        .join("right/entry.ts")
        .canonicalize()
        .unwrap();
    let package = Package::read(&super::vue_test_support::input(
        "NANOBUG_TYPESCRIPT_PACKAGE",
    ))
    .unwrap();
    let plugin = package.manifest.id.clone();
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "TS primary catalog",
        30,
        |app, _| {
            app.providers
                .language_servers()
                .get("typescript")
                .is_some_and(Option::is_some)
        },
    );
    open(&fixture, &shared, visual);
    picker(&fixture, visual);
    visual.simulate_keystrokes("escape");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "cancelled project choice",
        30,
        |app, cx| !modal(app, cx),
    );
    assert!(
        visual.update(|_, cx| fixture
            .app
            .read(cx)
            .language_server_for_path(&shared)
            .is_none()),
        "cancel must not publish a partial primary binding"
    );

    command(&fixture, &plugin, visual);
    choose(&fixture, "left", visual);
    ready(&fixture, &shared, visual);
    let (_, first) = hover(&fixture, &shared, lsp_types::Position::new(1, 16), visual);
    assert!(
        first.contains("left"),
        "left graph must type the original shared source: {first}"
    );
    open(&fixture, &right, visual);
    ready(&fixture, &right, visual);
    let (old_b, _) = hover(&fixture, &right, lsp_types::Position::new(1, 15), visual);
    let b_service = visual.update(|_, cx| {
        fixture
            .app
            .read(cx)
            .language_server_for_path(&right)
            .unwrap()
            .clone()
    });
    open(&fixture, &shared, visual);
    let (old_a, _) = hover(&fixture, &shared, lsp_types::Position::new(1, 16), visual);
    let a_service = visual.update(|_, cx| {
        fixture
            .app
            .read(cx)
            .language_server_for_path(&shared)
            .unwrap()
            .clone()
    });
    assert!(old_a.is_live() && old_b.is_live());
    let a_preview = old_a.fix_origin(&a_service);
    let b_preview = old_b.fix_origin(&b_service);
    command(&fixture, &plugin, visual);
    choose_after_focus_moves(&fixture, "right", &right, visual);
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "new selected A binding",
        120,
        |app, _| {
            app.language_server_for_path(&shared)
                .is_some_and(|server| !Arc::ptr_eq(server, &a_service))
        },
    );
    let (_, second) = hover(&fixture, &shared, lsp_types::Position::new(1, 16), visual);
    assert!(
        second.contains("42"),
        "right graph must type the same original source: {second}"
    );
    assert!(
        !old_a.is_live() && !a_preview.is_live(),
        "A result and preview must retire on selection"
    );
    assert!(
        old_b.is_live() && b_preview.is_live(),
        "B result and preview must retain their own source epoch"
    );
    assert!(
        visual.update(|_, cx| fixture
            .app
            .read(cx)
            .language_server_for_path(&right)
            .is_some_and(|server| Arc::ptr_eq(server, &b_service))),
        "B native transport must remain identical"
    );
    incompatible_candidate(&fixture, &package, &shared, &right, visual);

    // Closing forgets ephemeral routing, while a native explicit choice survives as private data.
    visual.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.close_tab(shared.clone(), window, cx))
    });
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "closed selected source",
        30,
        |app, _| !app.tabs.iter().any(|tab| tab.path() == shared),
    );
    open(&fixture, &shared, visual);
    ready_without_picker(&fixture, &shared, visual);
    assert!(
        hover(&fixture, &shared, lsp_types::Position::new(1, 16), visual)
            .1
            .contains("42")
    );

    // A real local definition target inherits left only for this navigation. The saved right choice
    // remains untouched and is restored after the original target session closes and reopens.
    open(&fixture, &left, visual);
    ready(&fixture, &left, visual);
    let prior_display = visual.update(|_, cx| {
        let app = fixture.app.read(cx);
        let source = app.language_server_for_path(&left).unwrap();
        let target = app.language_server_for_path(&shared).unwrap();
        eprintln!(
            "F12 source/old target roots: {:?} / {:?}; bindings: {:?}",
            source.navigation_context_service().map(|plan| (
                plan.owner.clone(),
                plan.provider.id.clone(),
                plan.root.clone()
            )),
            target.navigation_context_service().map(|plan| (
                plan.owner.clone(),
                plan.provider.id.clone(),
                plan.root.clone()
            )),
            fixture
                .actor
                .state
                .lock()
                .unwrap()
                .language_document_bindings
        );
        target.clone()
    });
    visual.update(|window, cx| {
        let editor = fixture.app.read(cx).editor.clone();
        let offset = editor
            .read(cx)
            .text()
            .to_string()
            .rfind("selected")
            .unwrap()
            + 1;
        editor.update(cx, |editor, cx| {
            editor.set_selected_range(offset..offset, cx);
            editor.focus(window, cx);
        });
        window.draw(cx).clear(cx);
    });
    visual.simulate_keystrokes("f12");
    let declaration = shared_text.find("selected").unwrap();
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "native F12 original shared declaration",
        30,
        |app, cx| {
            app.active_path.as_deref() == Some(shared.as_path())
                && (declaration..=declaration + "selected".len())
                    .contains(&app.editor.read(cx).cursor())
        },
    );
    // Target focus is published before asynchronous delegate preparation. Observe that the old
    // right display was actually withdrawn instead of querying it as if navigation had completed.
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "F12 inherited context publication",
        30,
        |app, _| {
            app.language_server_for_path(&shared)
                .is_some_and(|server| !Arc::ptr_eq(server, &prior_display))
        },
    );
    ready_without_picker(&fixture, &shared, visual);
    visual.update(|_, cx| {
        let app = fixture.app.read(cx);
        let inherited = app
            .language_server_for_path(&shared)
            .unwrap()
            .navigation_context_service()
            .unwrap();
        assert_eq!(
            inherited.root,
            left.parent().unwrap(),
            "F12 must publish the canonical left project for the original target"
        );
        eprintln!("F12 inherited target root: {}", inherited.root.display());
    });
    let inherited = hover(&fixture, &shared, lsp_types::Position::new(1, 16), visual).1;
    assert!(
        inherited.contains("left"),
        "F12 source context must type the original target as left: {inherited}"
    );
    visual.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.close_tab(shared.clone(), window, cx))
    });
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "navigation source closed",
        30,
        |app, _| !app.tabs.iter().any(|tab| tab.path() == shared),
    );
    open(&fixture, &shared, visual);
    ready_without_picker(&fixture, &shared, visual);
    assert!(
        hover(&fixture, &shared, lsp_types::Position::new(1, 16), visual)
            .1
            .contains("42")
    );
    assert_eq!(std::fs::read_to_string(shared).unwrap(), shared_text);
    shutdown(&fixture, visual);
}

/// An incompatible metadata-only update is rejected by normal ZIP inspection before installation.
/// Preserve the candidate and receipt beside the explicit package input for coordinator evidence.
fn incompatible_candidate(
    fixture: &Fixture,
    package: &Package,
    source: &Path,
    other: &Path,
    visual: &mut VisualTestContext,
) {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    let private = Manager::persisted_data_directory(
        &fixture.store,
        &package.manifest,
        &fixture.workspace.display().to_string(),
    )
    .unwrap();
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "nonempty explicit project preference",
        30,
        |_, _| {
            preferences(&private).values().any(|bytes| {
                serde_json::from_slice::<serde_json::Value>(bytes).is_ok_and(|value| {
                    value["data"]["source"] == "shared.ts"
                        && value["data"]["project"]
                            .as_str()
                            .is_some_and(|key| !key.is_empty())
                        && value["revision"]
                            .as_u64()
                            .is_some_and(|revision| revision > 0)
                })
            })
        },
    );
    let before = preferences(&private);
    assert!(
        !before.is_empty(),
        "this scenario must retain an actual saved project choice"
    );
    let (a, b) = visual.update(|_, cx| {
        let app = fixture.app.read(cx);
        (
            app.language_server_for_path(source).unwrap().clone(),
            app.language_server_for_path(other).unwrap().clone(),
        )
    });
    let mut files = package.files.clone();
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["version"] = serde_json::json!("99.0.0");
    manifest["api"]["required"]["process"] = serde_json::json!(">=99,<100");
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let contributions = String::from_utf8(files["plugin.toml"].clone())
        .unwrap()
        .replace(
            &format!("version = \"{}\"", package.manifest.version),
            "version = \"99.0.0\"",
        );
    files.insert("plugin.toml".into(), contributions.into_bytes());
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in files {
        archive
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(&bytes).unwrap();
    }
    let bytes = archive.finish().unwrap().into_inner();
    let archive_input = super::vue_test_support::input("NANOBUG_TYPESCRIPT_PACKAGE");
    let evidence = archive_input
        .parent()
        .unwrap()
        .join("shared-context-evidence");
    std::fs::create_dir_all(&evidence).unwrap();
    let candidate = evidence.join("typescript-incompatible-process.zip");
    std::fs::write(&candidate, &bytes).unwrap();
    fixture
        .actor
        .tx
        .send(Work::Inspect(candidate.clone()))
        .unwrap();
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "native incompatible candidate rejection",
        30,
        |app, cx| {
            app.extensions
                .read(cx)
                .status
                .as_ref()
                .is_some_and(|status| {
                    status.message.contains("process") && status.message.contains("99")
                })
        },
    );
    let reason = visual.update(|_, cx| {
        fixture
            .app
            .read(cx)
            .extensions
            .read(cx)
            .status
            .as_ref()
            .unwrap()
            .message
            .clone()
    });
    let after = preferences(&private);
    assert_eq!(
        after, before,
        "incompatible metadata must preserve every saved preference byte"
    );
    let installed = Manager::read_registry(&fixture.store).unwrap();
    assert_eq!(installed[&package.manifest.id].digest, package.digest);
    assert_eq!(
        installed[&package.manifest.id].manifest.version,
        package.manifest.version
    );
    assert!(
        visual.update(|_, cx| {
            let app = fixture.app.read(cx);
            app.language_server_for_path(source)
                .is_some_and(|server| Arc::ptr_eq(server, &a))
                && app
                    .language_server_for_path(other)
                    .is_some_and(|server| Arc::ptr_eq(server, &b))
        }),
        "rejected metadata must retain both original native contexts"
    );
    assert!(
        hover(fixture, source, lsp_types::Position::new(1, 16), visual)
            .1
            .contains("42")
    );
    assert!(
        hover(fixture, other, lsp_types::Position::new(1, 15), visual)
            .1
            .contains("42")
    );
    let digest = format!("{:x}", Sha256::digest(&bytes));
    std::fs::write(evidence.join("metadata-admission.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "candidate":candidate,"sha256":digest,"failure":reason,"installed_digest":package.digest,
        "preferences_before":before,"preferences_after":after,
    })).unwrap()).unwrap();
    eprintln!(
        "TS incompatible API candidate: {} SHA256 {digest}; {reason}",
        candidate.display()
    );
}

/// Read only the fixture owner's actual preference records, not compiler caches or source files.
fn preferences(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return BTreeMap::new(),
        Err(error) => panic!("Cannot inspect fixture preferences: {error}"),
    };
    entries
        .filter_map(|entry| {
            let entry = entry.unwrap();
            let name = entry.file_name().into_string().unwrap();
            (name.starts_with("preference-") && name.ends_with(".json"))
                .then(|| (name, std::fs::read(entry.path()).unwrap()))
        })
        .collect()
}

/// Real native input drives the ordinary picker; no path setting or simulated service reply is used.
fn choose(fixture: &Fixture, value: &str, visual: &mut VisualTestContext) {
    picker(fixture, visual);
    visual.simulate_input(value);
    visual.simulate_keystrokes("enter");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "native choice accepted",
        30,
        |app, cx| !modal(app, cx),
    );
}

/// Hold a real SDK refresh at the existing actor/UI boundary, move focus and start another
/// document's native request, then dispatch the original refresh through the ordinary UI route.
fn choose_after_focus_moves(
    fixture: &Fixture,
    value: &str,
    other: &Path,
    visual: &mut VisualTestContext,
) {
    picker(fixture, visual);
    visual.simulate_input(value);
    visual.simulate_keystrokes("enter");
    // Pause the existing UI queue at its scheduling seam. Production polling still publishes
    // every real request; this fixture executes preceding controls and retains only Refresh.
    visual.update(|_, cx| {
        fixture.app.update(cx, |app, _| {
            app.editor_request_dispatch_scheduled = true;
        })
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    let refresh = loop {
        // The preceding real Control request must run on the UI before the guest can emit
        // Refresh. Drain that ordinary work while withholding only the refresh receipt.
        visual.executor().advance_clock(Duration::from_millis(20));
        visual.run_until_parked();
        let mut requests = {
            let mut state = fixture.actor.state.lock().unwrap();
            std::mem::take(&mut state.editor_requests)
        };
        let found = visual.update(|window, cx| {
            let found = fixture.app.update(cx, |app, cx| {
                requests.append(&mut app.pending_editor_requests);
                let found = requests
                    .iter()
                    .position(|(_, request)| {
                        matches!(
                            request.operation(),
                            protocol::api::EditorOperation::RefreshLanguageContext { .. }
                        )
                    })
                    .map(|index| requests.remove(index));
                for (plugin, request) in requests {
                    app.perform_editor_request(&plugin, request, window, cx);
                }
                found
            });
            window.draw(cx).clear(cx);
            found
        });
        if let Some(refresh) = found {
            break refresh;
        }
        assert!(
            Instant::now() < deadline,
            "real context refresh did not reach the UI boundary"
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    visual.update(|_, cx| {
        fixture.app.update(cx, |app, _| {
            app.editor_request_dispatch_scheduled = false;
        })
    });
    visual.run_until_parked();
    open(fixture, other, visual);
    visual.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            assert!(
                app.native_language_feature_available(protocol::direct::Feature::References, cx)
            );
            app.request_language_feature(protocol::direct::Feature::References, window, cx);
            let sequence = app.language_features_sequence();
            app.pending_editor_requests.push(refresh);
            app.dispatch_editor_requests(window, cx);
            assert_eq!(
                app.language_features_sequence(),
                sequence,
                "a late refresh for A must retain B's current native request"
            );
        });
    });
}
fn picker(fixture: &Fixture, visual: &mut VisualTestContext) {
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "native project QuickPick",
        180,
        modal,
    );
}
/// Modal presence is observed through the same native controller that handles keyboard input.
fn modal(app: &EditorApp, cx: &gpui::App) -> bool {
    app.plugin_interactions
        .iter()
        .any(|entity| entity.read(cx).is_modal())
}
/// Invoke the installed command through its ordinary epoch-checked native dispatch route.
fn command(fixture: &Fixture, plugin: &str, visual: &mut VisualTestContext) {
    visual.update(|_, cx| {
        fixture
            .app
            .read(cx)
            .extensions
            .read(cx)
            .invoke_command(plugin, "select-project", serde_json::Value::Null)
            .unwrap()
    });
}
/// File opening keeps the existing native DocumentSession as the only mutable text owner.
fn open(fixture: &Fixture, path: &Path, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        window.activate_window();
        fixture
            .app
            .update(cx, |app, cx| app.open_file(path.to_owned(), window, cx));
    });
}
/// Both the published service and visible editor hover hook must have attached to this source.
fn ready(fixture: &Fixture, path: &Path, visual: &mut VisualTestContext) {
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "selected primary native source",
        180,
        |app, cx| {
            app.language_server_for_path(path)
                .is_some_and(|server| server.is_active())
                && app
                    .tabs
                    .iter()
                    .find(|tab| tab.path() == path)
                    .and_then(|tab| tab.text.as_ref())
                    .is_some_and(|text| text.editor.read(cx).lsp().hover_provider.is_some())
        },
    );
}
/// A valid persisted or inherited choice must not require another human decision.
fn ready_without_picker(fixture: &Fixture, path: &Path, visual: &mut VisualTestContext) {
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "remembered or inherited native source",
        180,
        |app, cx| {
            assert!(
                !modal(app, cx),
                "valid saved/origin context must not prompt again"
            );
            app.language_server_for_path(path)
                .is_some_and(|server| server.is_active())
        },
    );
}

/// Semantics and stale-preview authority are captured on the actual native connection off the UI.
fn hover(
    fixture: &Fixture,
    path: &Path,
    position: lsp_types::Position,
    visual: &mut VisualTestContext,
) -> (crate::language::navigation::ContextCapture, String) {
    let (server, capture, uri, source) = visual.update(|_, cx| {
        let app = fixture.app.read(cx);
        let index = app.tabs.iter().position(|tab| tab.path() == path).unwrap();
        let text = app.tabs[index]
            .text
            .as_ref()
            .unwrap()
            .editor
            .read(cx)
            .text()
            .to_string();
        let server = app.language_server_for_path(path).unwrap().clone();
        let capture = app
            .capture_language_context(
                &server,
                &app.plugin_document_version(index).unwrap(),
                &text,
                cx,
            )
            .unwrap()
            .unwrap();
        (
            server,
            capture,
            crate::language::navigation::file_uri(path).unwrap(),
            text,
        )
    });
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let value = server
            .with_context_snapshot(&capture, || server.hover(uri, source, position))
            .map(|value| serde_json::to_string(&value).unwrap())
            .map_err(|error| format!("{error:#}"));
        send.send(value.map(|value| (capture, value))).unwrap();
    });
    let result = std::cell::RefCell::new(None);
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "real selected-project hover",
        30,
        |_, _| {
            if let Ok(value) = receive.try_recv() {
                *result.borrow_mut() = Some(value);
            }
            result.borrow().is_some()
        },
    );
    result.into_inner().unwrap().unwrap()
}
