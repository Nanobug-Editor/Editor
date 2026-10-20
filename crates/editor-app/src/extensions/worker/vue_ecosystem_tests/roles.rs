//! Real Package roles compose in the production Worker and keep every observable native span.
use super::super::vue_test_support::{Fixture, setup_for_workspace};
use super::*;
use gpui_kit::VisualTestContext;

/// All role effects share one dirty source, including Unicode before the mapped numeric token.
const SOURCE: &str = "<script setup lang=\"ts\">\r\nimport { count, shade } from './state';\r\nconst eco = /*未保存 中文😀e\u{301}*/ 42;\r\n</script>\r\n<template lang=\"quill\"><p>{{count.toUpperCase()}}</p><AbsentThing /></template>\r\n<style lang=\"ink\">shade.toUpperCase()</style>\r\n";
/// Public declarations make transformed template/style renames genuinely cross original files.
const STATE_SOURCE: &str = "export const count = 42;\r\nexport const shade = 42;\r\n";

mod mapping;
mod retirement;
mod scenarios;
use mapping::{mapped_rename, original_definition, references};
use retirement::{assert_pending_mapping_withdrawn, pending_mapped_rename};

/// Isolate source/configuration and use only an explicitly selected dependency fixture junction.
fn project() -> tempfile::TempDir {
    // Dependency junctions remain inside the actually authorized workspace. An external TempDir
    // workspace would correctly reject its link back into this separately installed fixture tree.
    let project = tempfile::Builder::new()
        .prefix("s4-roles-")
        .tempdir_in(input("NANOBUG_VUE_FIXTURES").join("target"))
        .unwrap();
    std::fs::write(
        project.path().join("App.vue"),
        "<template><p>saved</p></template>\r\n",
    )
    .unwrap();
    std::fs::write(project.path().join("state.ts"), STATE_SOURCE).unwrap();
    std::fs::write(project.path().join("tsconfig.json"), r#"{
  "compilerOptions":{"target":"ES2020","module":"ESNext","moduleResolution":"Node","strict":true,"skipLibCheck":true},
  "include":["*.vue","*.ts"],
  "vueCompilerOptions":{"target":3.5,"strictTemplates":true,"checkUnknownComponents":false}
}"#).unwrap();
    let dependency = input("NANOBUG_VUE_FIXTURES")
        .join("tests/projects/vue3/node_modules")
        .canonicalize()
        .unwrap();
    // PowerShell -Value treats the verbatim prefix literally. Strip only that Windows spelling;
    // arguments stay in child-only environment variables, never interpolated into shell code.
    let target = dependency
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_owned();
    let mut command = std::process::Command::new("powershell");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        // Keep this non-interactive fixture helper off the user's desktop.
        command.creation_flags(0x0800_0000);
    }
    let output = command.args(["-NoProfile", "-NonInteractive", "-Command",
            "New-Item -ItemType Junction -Path $env:NANOBUG_S4_LINK -Value $env:NANOBUG_S4_TARGET -ErrorAction Stop | Out-Null"])
        .env("NANOBUG_S4_LINK", project.path().join("node_modules"))
        .env("NANOBUG_S4_TARGET", target)
        .output().unwrap();
    assert!(
        output.status.success(),
        "fixture dependency junction: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    project
}

/// Change text through the native input path; the stored disk file intentionally stays different.
fn open_dirty(fixture: &Fixture, path: &Path, visual: &mut VisualTestContext) {
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "role language catalog",
        30,
        |app, _| {
            app.providers
                .language_servers()
                .get("vue")
                .is_some_and(Option::is_some)
        },
    );
    visual.update(|window, cx| {
        window.activate_window();
        fixture
            .app
            .update(cx, |app, cx| app.open_file(path.to_owned(), window, cx));
        window.draw(cx).clear(cx);
    });
    visual.simulate_keystrokes("ctrl-a");
    visual.simulate_input(SOURCE);
}

/// Check final native UTF-8 ranges independently from the LSP conversion helper.
fn role_effects(app: &EditorApp, cx: &gpui::App, style: Option<bool>) -> bool {
    let editor = app.editor.read(cx);
    let Some(items) = editor.diagnostics() else {
        return false;
    };
    let missing = items
        .iter()
        .filter(|item| item.message.contains("No ecosystem contribution supports"))
        .collect::<Vec<_>>();
    let numeric = items
        .iter()
        .filter(|item| item.message.contains("Argument of type") && item.message.contains("string"))
        .collect::<Vec<_>>();
    let methods = items
        .iter()
        // TS retains const 42 here; both the literal and its widened numeric type reject this
        // string-only method. Exact original-source spans below still distinguish both roles.
        .filter(|item| {
            item.message.contains("toUpperCase")
                && (item.message.contains("type '42'") || item.message.contains("type 'number'"))
        })
        .collect::<Vec<_>>();
    let unknown = items
        .iter()
        .filter(|item| item.message.contains("AbsentThing"))
        .collect::<Vec<_>>();
    match style {
        Some(has_style) => {
            let expected = SOURCE
                .match_indices("toUpperCase")
                .map(|(start, text)| start..start + text.len())
                .collect::<Vec<_>>();
            let mut observed = methods
                .iter()
                .map(|item| (item.range.start, item.range.end))
                .collect::<Vec<_>>();
            observed.sort_unstable();
            let wanted = expected[..if has_style { 2 } else { 1 }]
                .iter()
                .map(|range| (range.start, range.end))
                .collect::<Vec<_>>();
            numeric.len() == 1
                && numeric[0].range == (SOURCE.find("42").unwrap()..SOURCE.find("42").unwrap() + 2)
                && unknown.len() == 1
                && SOURCE.get(unknown[0].range.clone()) == Some("AbsentThing")
                // Equality keeps duplicate template markers from standing in for a style result.
                && observed == wanted
                && if has_style {
                    missing.is_empty()
                } else {
                    missing.len() == 1
                        && missing[0].message.contains("style language \"ink\"")
                        && SOURCE.get(missing[0].range.clone()) == Some("<style lang=\"ink\">")
                }
        }
        None => {
            numeric.is_empty()
                && methods.is_empty()
                && unknown.is_empty()
                && missing.len() == 2
                && missing
                    .iter()
                    .any(|item| item.message.contains("template language \"quill\""))
                && missing
                    .iter()
                    .any(|item| item.message.contains("style language \"ink\""))
        }
    }
}

/// A real semantic marker set is necessary; merely attaching a provider cannot satisfy role coverage.
fn wait_effects(fixture: &Fixture, visual: &mut VisualTestContext, style: bool) {
    let last_report = RefCell::new(Instant::now());
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "all mapped role effects",
        180,
        |app, cx| {
            assert_preparation(app, &fixture.actor);
            let matches = app.editor.read(cx).lsp().definition_provider.is_some()
                && role_effects(app, cx, Some(style));
            if !matches && last_report.borrow().elapsed() >= Duration::from_secs(15) {
                eprintln!(
                    "S4 role markers style={style}: {:?}",
                    app.editor.read(cx).diagnostics().map(|items| items
                        .iter()
                        .map(|item| (&item.message, &item.range))
                        .collect::<Vec<_>>())
                );
                *last_report.borrow_mut() = Instant::now();
            }
            matches
        },
    );
}

/// Retain the original transport, incarnation and runtime plan so lifecycle checks cannot inspect a replacement.
fn capture(
    fixture: &Fixture,
    path: &Path,
    visual: &mut VisualTestContext,
) -> (
    Arc<crate::language::navigation::LanguageServer>,
    crate::language::navigation::DocumentLease,
    Arc<plugin_runtime::LanguageService>,
) {
    visual.update(|_, cx| {
        let app = fixture.app.read(cx);
        let server = app.language_server_for_path(path).unwrap().clone();
        let document = server
            .document(&crate::language::navigation::file_uri(path).unwrap())
            .unwrap();
        let state = fixture.actor.state.lock().unwrap();
        let plan = state
            .language_document_bindings
            .iter()
            .find(|((provider, _), _)| provider == "vue/vue")
            .map(|(_, key)| state.language_services[key].as_ref().unwrap().clone())
            .unwrap();
        assert_eq!(plan.owner, "typescript");
        assert_eq!(plan.process_count(), 1);
        (server, document, plan)
    })
}

/// Revocation must kill old calls and actual processes, even when a replacement is already usable.
fn wait_retired(
    fixture: &Fixture,
    visual: &mut VisualTestContext,
    original: &(
        Arc<crate::language::navigation::LanguageServer>,
        crate::language::navigation::DocumentLease,
        Arc<plugin_runtime::LanguageService>,
    ),
) {
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "old role context retired",
        30,
        |_, _| !original.2.is_active(),
    );
    assert!(!original.0.is_active());
    assert_eq!(original.2.process_count(), 0);
    assert!(
        original
            .0
            .diagnostics_for(original.1.clone(), SOURCE)
            .is_err(),
        "retired mappings cannot publish"
    );
}

/// Keep the native editor as the sole caret and source owner when invoking ordinary keyboard commands.
fn caret(fixture: &Fixture, offset: usize, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        fixture
            .app
            .read(cx)
            .editor
            .clone()
            .update(cx, |editor, cx| {
                editor.set_selected_range(offset..offset, cx);
                editor.focus(window, cx);
            });
        window.draw(cx).clear(cx);
    });
}

/// Paint the ordinary dialog while the real actor publishes; no test-only completion channel is added.
fn control(
    fixture: &Fixture,
    visual: &mut VisualTestContext,
    selector: &'static str,
    visible: bool,
) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        wait(
            &fixture.app,
            &fixture.actor,
            visual,
            "role edit frame",
            1,
            |_, _| true,
        );
        if visual.debug_bounds(selector).is_some() == visible {
            return;
        }
        if Instant::now() >= deadline {
            // Capture the production status and actual native target only at failure. A missing
            // preview must distinguish rejected edits from a lost input/focus or stale source.
            let native = visual.update(|window, cx| {
                let app = fixture.app.read(cx);
                let editor = app.editor.read(cx);
                let document = app
                    .tabs
                    .iter()
                    .position(|tab| tab.owns_editor(&app.editor))
                    .and_then(|index| app.plugin_document_version(index).ok());
                let controls = gpui_base::test_support::snapshots(window)
                    .into_iter()
                    .filter(|node| {
                        node.visible()
                            && (node.role() == Some(gpui_kit::Role::Dialog)
                                || node.path().iter().any(|id| {
                                    matches!(id, gpui_kit::ElementId::Name(name)
                                        if name.starts_with("workspace-")
                                            || name.starts_with("editor-rename"))
                                }))
                    })
                    .take(12)
                    .collect::<Vec<_>>();
                format!(
                    "status={}; path={:?}; editor={:?}; document={document:?}; selection={:?}; source_focused={}; focused={:?}; services={:?}; controls={controls:?}",
                    app.status,
                    app.active_path,
                    app.editor.entity_id(),
                    editor.selected_range(),
                    app.editor.focus_handle(cx).is_focused(window),
                    window.focused(cx),
                    app.language_service_states
                )
            });
            panic!("role edit control {selector} visibility={visible}: {native}");
        }
    }
}

/// Use the painted Base button's actual bounds for preview confirmation, cancellation and group Undo.
fn click(visual: &mut VisualTestContext, selector: &'static str) {
    let bounds = visual.debug_bounds(selector).unwrap();
    visual.simulate_mouse_move(bounds.center(), None, gpui_kit::Modifiers::default());
    visual.simulate_click(bounds.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
}

/// Require a response from the replacement's ordinary script engine, independent of cleared markers.
fn actual_script_hover(fixture: &Fixture, visual: &mut VisualTestContext) {
    let task = visual.update(|window, cx| {
        let app = fixture.app.read(cx);
        let editor = app.editor.read(cx);
        editor.lsp().hover_provider.clone().unwrap().hover(
            &editor.text().clone(),
            SOURCE.find("eco").unwrap() + 1,
            window,
            cx,
        )
    });
    let result = Rc::new(RefCell::new(None));
    let delivered = result.clone();
    visual.update(|_, cx| {
        fixture.app.update(cx, |_, cx| {
            cx.spawn(async move |_, _| {
                *delivered.borrow_mut() = Some(task.await);
            })
            .detach()
        })
    });
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "ordinary script hover after uninstall",
        30,
        |_, _| result.borrow().is_some(),
    );
    let borrowed = result.borrow();
    let hover = borrowed
        .as_ref()
        .unwrap()
        .as_ref()
        .unwrap()
        .as_ref()
        .unwrap();
    let text = serde_json::to_string(hover).unwrap();
    assert!(
        text.contains("eco") && text.contains("42"),
        "ordinary hover: {text}"
    );
}

/// Neither contribution analysis nor native edit previews may save source/configuration to disk.
fn assert_source(
    fixture: &Fixture,
    visual: &mut VisualTestContext,
    path: &Path,
    disk: &str,
    config: &str,
) {
    assert!(visual.update(|_, cx| mapping::sources_equal(
        fixture.app.read(cx),
        cx,
        path,
        SOURCE,
        STATE_SOURCE
    )));
    assert_eq!(std::fs::read_to_string(path).unwrap(), disk);
    assert_eq!(
        std::fs::read_to_string(path.parent().unwrap().join("state.ts")).unwrap(),
        STATE_SOURCE
    );
    assert_eq!(
        std::fs::read_to_string(path.parent().unwrap().join("tsconfig.json")).unwrap(),
        config
    );
}
