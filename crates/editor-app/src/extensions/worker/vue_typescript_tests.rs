//! Independent candidate archives exercise the production actor and the native unsaved document path.
//! External package/fixture paths are explicit inputs; this test never builds or downloads dependencies.
use super::*;
use crate::extensions::protocol;
use crate::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};
use std::cell::RefCell;
use std::time::{Duration, Instant};

/// Load the candidate's actual grammar and queries through the production WasmStore and parser.
/// Keeping this independent of guest activation distinguishes parser startup from control routing.
#[test]
#[ignore = "set NANOBUG_VUE_PACKAGE to an independently built Vue candidate"]
fn independent_vue_candidate_grammar_uses_the_native_loader() {
    let package = Package::read(&PathBuf::from(
        std::env::var_os("NANOBUG_VUE_PACKAGE").unwrap(),
    ))
    .unwrap();
    let declaration_path = package.manifest.contributions.as_ref().unwrap();
    let manifest = plugin_schema::PluginManifest::parse(
        std::str::from_utf8(&package.files[declaration_path]).unwrap(),
    )
    .unwrap();
    let declaration = manifest
        .highlighters
        .first()
        .expect("candidate contributes a grammar")
        .clone();
    let directory = tempfile::tempdir().unwrap();
    for relative in [&declaration.grammar, &declaration.highlights]
        .into_iter()
        .chain(declaration.injections.iter())
    {
        let target = directory.path().join(relative);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        let key = relative.to_string_lossy().replace('\\', "/");
        std::fs::write(target, &package.files[&key]).unwrap();
    }
    let provider = crate::language::providers::GrammarProvider {
        owner: package.manifest.id,
        // Contribution::read supplies a canonical immutable package root in production. Preserve
        // that same invariant here, including Windows' verbatim prefix, before path admission.
        root: directory.path().canonicalize().unwrap(),
        declaration,
    };
    eprintln!("S1 grammar phase: production module/query validation");
    let (grammar, _) = crate::language::plugins::prepare_dynamic(&provider).unwrap();
    eprintln!("S1 grammar phase: production parser initialization");
    let (mut parser, language) = crate::language::plugins::readonly_parser(grammar).unwrap();
    parser.set_language(&language).unwrap();
    let tree = parser.parse("<template><p>{{ message }}</p></template>\n<script>export default { data() { return { message: '中文😀' } } }</script>",None).unwrap();
    assert!(
        !tree.root_node().has_error(),
        "candidate SFC grammar must parse ordinary source"
    );
}

/// Vue 2.6/2.7/3 coexist under one workspace, share their TS owner's exact connection, and retain
/// unsaved source through an actual Base navigation request instead of reading source from disk.
#[gpui::test]
#[ignore = "set NANOBUG_TYPESCRIPT_PACKAGE, NANOBUG_VUE_PACKAGE and NANOBUG_VUE_FIXTURES to built independent candidates"]
fn independent_vue_typescript_packages_analyze_native_mixed_project_tabs(cx: &mut TestAppContext) {
    eprintln!("S1 phase: install independent candidates");
    let input = |name| PathBuf::from(std::env::var_os(name).expect(name));
    let workspace = input("NANOBUG_VUE_FIXTURES").canonicalize().unwrap();
    // An explicit private output keeps this exact installed state for subsequent desktop checks.
    // Ordinary runs still remove it, and a failed scenario never retains a purportedly ready profile.
    let native_profiles = std::env::var_os("NANOBUG_S1_NATIVE_PROFILES").map(PathBuf::from);
    let profile = match &native_profiles {
        Some(parent) => tempfile::Builder::new()
            .prefix("s1-verified-")
            .tempdir_in(parent)
            .unwrap(),
        None => tempfile::tempdir().unwrap(),
    };
    let root = profile.path().join("runtime-plugins");
    let environment = protocol::Environment {
        workspace: workspace.display().to_string(),
        ..Default::default()
    };
    {
        let mut manager = Manager::open(root.clone(), environment.clone()).unwrap();
        for name in ["NANOBUG_TYPESCRIPT_PACKAGE", "NANOBUG_VUE_PACKAGE"] {
            let package = Package::read(&input(name)).unwrap();
            manager
                .install(&package, package.manifest.permissions.clone())
                .unwrap();
        }
    }
    eprintln!("S1 phase: construct native project and production actor");
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::controls::init(cx);
        crate::bind_editor_shell_keys(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let session = SessionState::load_in_profile(&workspace, profile.path());
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let editor_workspace = Workspace::open(&workspace).unwrap();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| {
            EditorApp::new_with_session(editor_workspace, None, Some(session), window, cx)
        });
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    // This acceptance uses the actual background actor, including deferred guest calls, native
    // EditorRequests and immutable publications; the usual recorded test mailbox is not pumped.
    let actor = Arc::new(Worker::start_background(root.clone(), environment, true));
    visual.update(|_, cx| {
        app.read(cx)
            .extensions
            .clone()
            .update(cx, |panel, _| panel.worker = actor.clone())
    });
    visual.simulate_resize(size(px(1200.), px(800.)));
    let paths = ["vue26", "vue27", "vue3"].map(|name| {
        workspace
            .join(format!("tests/projects/{name}/App.vue"))
            .canonicalize()
            .unwrap()
    });
    // Separate cold registry publication from document preparation. A missing UI contribution must
    // fail here instead of looking like a language worker that silently indexed for five minutes.
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        frame(&app, visual);
        let ready = {
            let state = actor.state.lock().unwrap();
            assert!(
                state.entries.iter().all(|entry| entry.error.is_none()),
                "candidate admission failed: {:?}",
                state
                    .entries
                    .iter()
                    .map(|entry| (&entry.manifest.id, &entry.error))
                    .collect::<Vec<_>>()
            );
            state.ready && state.entries.len() == 2
        };
        if ready {
            let recognized = visual.update(|_, cx| {
                let app = app.read(cx);
                app.providers.store_root() == root.canonicalize().unwrap()
                    && app.session_state.workspace_trusted
                    && paths.iter().all(|path| {
                        let language = app.language_for_path(path);
                        app.providers
                            .language_servers()
                            .get(&language)
                            .is_some_and(Option::is_some)
                    })
            });
            if recognized {
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "cold actor/contribution publication did not reach the native project: {}",
            mixed_status(&app, &actor, &paths, visual)
        );
    }
    eprintln!("S1 phase: cold actor and native recognition ready");
    let originals = paths
        .each_ref()
        .map(|path| std::fs::read_to_string(path).unwrap());
    let missing = ["s1_missing_vue26", "s1_missing_vue27", "s1_missing_vue3"];
    for ((path, original), unknown) in paths.iter().zip(&originals).zip(missing) {
        visual.update(|window, cx| {
            window.activate_window();
            app.update(cx, |app, cx| app.open_file(path.clone(), window, cx));
            window.draw(cx).clear(cx);
        });
        // A distinct unresolved template expression makes wrong-backend or duplicate semantic
        // publications observable. The valid message binding remains the navigation target.
        let source = original.replacen(
            ":title=\"message\"",
            &format!(":title=\"message\" :data-s1=\"{unknown}\""),
            1,
        );
        assert_ne!(
            source, *original,
            "the real fixture must have its typed binding"
        );
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input(&format!("<!-- unsaved 中文😀 -->\n{source}"));
    }
    let deadline = Instant::now() + Duration::from_secs(300);
    eprintln!("S1 phase: prepare opened native document contexts");
    let mut previous_request_stage = String::new();
    loop {
        frame(&app, visual);
        let request_stage = mixed_request_stage(&app, &actor, visual);
        if request_stage != previous_request_stage {
            // Report state transitions only. Publication generation advances on each actor tick
            // and is deliberately excluded so a stuck phase cannot flood or hide the first cause.
            eprintln!("S1 request phase: {request_stage}");
            previous_request_stage = request_stage;
        }
        let failed = visual.update(|_, cx| {
            app.read(cx).language_service_states.values().any(|state| {
                matches!(
                    state,
                    crate::app::language_servers::ServiceLoadState::Failed(_)
                )
            })
        });
        assert!(
            !failed,
            "native context preparation failed: {}",
            mixed_status(&app, &actor, &paths, visual)
        );
        let ready = visual.update(|_, cx| {
            paths.iter().all(|path| {
                app.read(cx)
                    .tabs
                    .iter()
                    .find(|tab| tab.path() == *path)
                    .and_then(|tab| tab.text.as_ref())
                    .is_some_and(|tab| tab.editor.read(cx).lsp().definition_provider.is_some())
            })
        });
        if ready {
            break;
        }
        if Instant::now() >= deadline {
            panic!(
                "native candidates did not attach: {}",
                mixed_status(&app, &actor, &paths, visual)
            );
        }
    }
    eprintln!("S1 phase: native providers attached; request actual source semantics");
    let requests = visual.update(|window, cx| {
        let captures = paths
            .iter()
            .map(|path| {
                let app = app.read(cx);
                let editor = app
                    .tabs
                    .iter()
                    .find(|tab| tab.path() == *path)
                    .unwrap()
                    .text
                    .as_ref()
                    .unwrap()
                    .editor
                    .read(cx);
                let source = editor.text().to_string();
                assert!(source.contains("unsaved 中文😀"));
                let offset = source.rfind("message").unwrap() + 2;
                (
                    path.clone(),
                    editor.lsp().definition_provider.clone().unwrap(),
                    editor.lsp().hover_provider.clone().unwrap(),
                    editor.lsp().completion_provider.clone().unwrap(),
                    editor.text().clone(),
                    offset,
                )
            })
            .collect::<Vec<_>>();
        captures
            .into_iter()
            .map(|(path, definition, hover, completion, source, offset)| {
                (
                    path,
                    source.to_string(),
                    definition.definitions(&source, offset, window, cx),
                    hover.hover(&source, offset, window, cx),
                    completion.completions(
                        &source,
                        offset,
                        lsp_types::CompletionContext {
                            trigger_kind: lsp_types::CompletionTriggerKind::INVOKED,
                            trigger_character: None,
                        },
                        window,
                        cx,
                    ),
                )
            })
            .collect::<Vec<_>>()
    });
    let results = Rc::new(RefCell::new(Vec::new()));
    for (path, source, definition, hover, completion) in requests {
        let results = results.clone();
        visual.update(|_, cx| {
            app.update(cx, |_, cx| {
                cx.spawn(async move |_, _| {
                    let observed = (definition.await, hover.await, completion.await);
                    results.borrow_mut().push((path, source, observed));
                })
                .detach()
            })
        });
    }
    let deadline = Instant::now() + Duration::from_secs(120);
    while results.borrow().len() != 3 {
        frame(&app, visual);
        assert!(
            Instant::now() < deadline,
            "native source semantics did not return"
        );
    }
    for (path, source, (definitions, hover, completions)) in results.borrow().iter() {
        let definitions = definitions
            .as_ref()
            .expect("actual template definition must succeed");
        assert!(
            !definitions.is_empty(),
            "the template symbol must navigate to its declaration: {}",
            path.display()
        );
        for definition in definitions {
            let uri = url::Url::parse(definition.target_uri.as_str()).unwrap();
            assert_eq!(
                uri.to_file_path().unwrap().canonicalize().unwrap(),
                *path,
                "navigation must reveal the original source, never a generated URI"
            );
            let range = definition.target_selection_range;
            assert_eq!(range.start.line, range.end.line);
            // Protocol columns are UTF-16 code units. Compare their selected original text
            // directly instead of repeating the host's byte/character conversion helper.
            let selected = source
                .lines()
                .nth(range.start.line as usize)
                .unwrap()
                .encode_utf16()
                .skip(range.start.character as usize)
                .take((range.end.character - range.start.character) as usize)
                .collect::<Vec<_>>();
            assert_eq!(String::from_utf16(&selected).unwrap(), "message");
        }
        let hover = hover
            .as_ref()
            .expect("actual template hover must succeed")
            .as_ref()
            .expect("the template symbol must have semantic hover");
        assert!(
            serde_json::to_string(&hover.contents)
                .unwrap()
                .contains("message")
        );
        let completions = completions
            .as_ref()
            .expect("actual template completion must succeed");
        let items = match completions {
            lsp_types::CompletionResponse::Array(items) => items,
            lsp_types::CompletionResponse::List(list) => &list.items,
        };
        assert!(
            items.iter().any(|item| item.label == "message"),
            "template completion must contain the original symbol"
        );
    }
    drop(results);
    for (path, unknown) in paths.iter().zip(missing) {
        super::vue_test_support::assert_native_references(
            &app, &actor, &workspace, path, "message", visual,
        );
        super::vue_test_support::assert_native_source_diagnostic(
            &app, &actor, path, unknown, &missing, visual,
        );
    }
    for (path, original) in paths.iter().zip(originals) {
        assert_eq!(std::fs::read_to_string(path).unwrap(), original);
    }
    let state = actor.state.lock().unwrap();
    let plans = state
        .language_document_bindings
        .iter()
        .filter(|((provider, _), _)| provider == "vue/vue")
        .map(|(_, key)| state.language_services[key].as_ref().unwrap().clone())
        .collect::<Vec<_>>();
    assert_eq!(plans.len(), 3);
    for (index, plan) in plans.iter().enumerate() {
        assert_eq!(plan.owner, "typescript");
        assert_eq!(plan.process_count(), 1);
        assert!(plans[..index].iter().all(|other| !Arc::ptr_eq(other, plan)));
    }
    drop(state);
    // Explicit shutdown reaps every managed compiler before temporary private storage is removed.
    let (send, mut receive) = futures::channel::oneshot::channel();
    actor.tx.send(Work::Shutdown(Some(send))).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        frame(&app, visual);
        // The actor is a real OS thread. Poll its acknowledgement without installing a foreground
        // test-scheduler waker that that thread is not permitted to schedule directly.
        if let Some(result) = receive.try_recv().unwrap() {
            result.unwrap();
            break;
        }
        assert!(
            Instant::now() < deadline,
            "candidate workers did not shut down"
        );
    }
    if native_profiles.is_some() {
        eprintln!("S1 verified desktop profile: {}", profile.keep().display());
    }
}

/// Report publication and native selection together without holding the worker lock across UI reads.
fn mixed_status(
    app: &Entity<EditorApp>,
    actor: &Worker,
    paths: &[PathBuf],
    visual: &mut VisualTestContext,
) -> String {
    let actor_status = {
        let state = actor.state.lock().unwrap();
        format!(
            "ready={} generation={} status={:?} entries={:?} startup={:?} bindings={:?} compilation={:?}",
            state.ready,
            state.generation,
            state.status.as_ref().map(|status| &status.message),
            state
                .entries
                .iter()
                .map(|entry| (&entry.manifest.id, entry.enabled, &entry.error))
                .collect::<Vec<_>>(),
            state.startup,
            state.language_document_bindings,
            state
                .entries
                .iter()
                .map(|entry| (
                    &entry.manifest.id,
                    state
                        .logs
                        .records(&entry.manifest.id)
                        .into_iter()
                        .filter(|record| record.source == "wasm/compile")
                        .map(|record| record.message)
                        .collect::<Vec<_>>()
                ))
                .collect::<Vec<_>>()
        )
    };
    let native_status = visual.update(|_, cx| {
        let app = app.read(cx);
        format!(
            "status={} states={:?} trusted={} store={} languages={:?} selected={:?}",
            app.status,
            app.language_service_states,
            app.session_state.workspace_trusted,
            app.providers.store_root().display(),
            paths
                .iter()
                .map(|path| app.language_for_path(path))
                .collect::<Vec<_>>(),
            app.providers.language_servers()
        )
    });
    format!(
        "{actor_status}; {native_status}; {}",
        mixed_request_stage(app, actor, visual)
    )
}

/// Observe both public actor ingress and the native request queue without executing queued work.
/// No argument payload or source text is recorded; method/source identities locate a stalled hop.
fn mixed_request_stage(
    app: &Entity<EditorApp>,
    actor: &Worker,
    visual: &mut VisualTestContext,
) -> String {
    let worker_stage = {
        let state = actor.state.lock().unwrap();
        let requests = state
            .editor_requests
            .iter()
            .map(|(plugin, request)| (plugin, control_request_summary(request)))
            .collect::<Vec<_>>();
        let plans = state
            .language_services
            .iter()
            .map(|(key, plan)| {
                (
                    key,
                    plan.as_ref().map(|plan| {
                        (
                            &plan.owner,
                            &plan.provider.id,
                            plan.is_active(),
                            plan.process_count(),
                        )
                    }),
                )
            })
            .collect::<Vec<_>>();
        let startup_logs = state
            .entries
            .iter()
            .map(|entry| {
                (
                    &entry.manifest.id,
                    state
                        .logs
                        .records(&entry.manifest.id)
                        .into_iter()
                        .filter(|record| {
                            record.source == "wasm/compile" || record.source.starts_with("lsp/")
                        })
                        .map(|record| (record.source, record.message))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        format!("worker_requests={requests:?} plans={plans:?} startup_logs={startup_logs:?}")
    };
    let native_stage = visual.update(|_, cx| {
        let app = app.read(cx);
        let requests = app
            .pending_editor_requests
            .iter()
            .map(|(plugin, request)| (plugin, control_request_summary(request)))
            .collect::<Vec<_>>();
        format!(
            "native_requests={requests:?} dispatch_scheduled={} states={:?}",
            app.editor_request_dispatch_scheduled, app.language_service_states
        )
    });
    format!("{worker_stage}; {native_stage}")
}

/// A public language operation's bounded envelope identifies resolve/attach without decoding it.
fn control_request_summary(request: &plugin_runtime::EditorRequest) -> String {
    match request.operation() {
        protocol::api::EditorOperation::LanguageRequest { request } => format!(
            "provider={} method={} document={:?}",
            request.provider, request.method, request.document
        ),
        _ => "other native operation".into(),
    }
}

/// Advance timers and paint the actual native view while compiler IO runs on real worker threads.
fn frame(app: &Entity<EditorApp>, visual: &mut VisualTestContext) {
    visual.executor().advance_clock(Duration::from_millis(20));
    visual.run_until_parked();
    visual.update(|window, cx| {
        // Drive the native publication boundary explicitly; deterministic UI timers and real worker
        // clocks need not advance together in a test. No synthetic result or source state is supplied.
        app.read(cx)
            .extensions
            .clone()
            .update(cx, |panel, cx| panel.poll(cx));
        window.draw(cx).clear(cx)
    });
    std::thread::sleep(Duration::from_millis(20));
}
