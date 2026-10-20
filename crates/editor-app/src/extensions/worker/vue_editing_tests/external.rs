//! External script associations keep original dirty native sessions through navigation and one batch Undo.
use super::*;
use gpui_kit::EntityInputHandler as _;

mod fixtures;
use fixtures::{Case, Projects};

/// Legacy TS and modern JS external scripts use installed packages and the ordinary native actions.
#[gpui::test]
#[ignore = "set actual NANOBUG_TYPESCRIPT_PACKAGE/NANOBUG_VUE_PACKAGE and NANOBUG_VUE_FIXTURES; external JS/TS native transactions"]
fn external_vue_scripts_map_dirty_sources_and_native_rename_with_group_undo(
    cx: &mut TestAppContext,
) {
    let projects = Projects::prepare();
    let (fixture, visual) = setup(cx, &["NANOBUG_TYPESCRIPT_PACKAGE", "NANOBUG_VUE_PACKAGE"]);
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "external script language catalog",
        30,
        |app, _| {
            app.providers
                .language_servers()
                .get("vue")
                .is_some_and(Option::is_some)
        },
    );
    for case in &projects.cases {
        eprintln!(
            "external native phase: {} / {} begin",
            case.version, case.extension
        );
        open(&fixture, &case.script, visual);
        replace(&fixture, &case.script_dirty, visual);
        ready(&fixture, &case.script, visual);
        assert_script_primary(&fixture, &case.script, visual);
        open(&fixture, &case.component, visual);
        replace(&fixture, &case.component_dirty, visual);
        ready(&fixture, &case.component, visual);
        super::super::vue_test_support::assert_native_source_diagnostic(
            &fixture.app,
            &fixture.actor,
            &case.component,
            "toUpperCase",
            &["toUpperCase"],
            visual,
        );
        visual.update(|_, cx| {
            let editor = fixture.app.read(cx).editor.read(cx);
            let diagnostic = editor
                .diagnostics()
                .unwrap()
                .iter()
                .find(|item| item.message.contains("toUpperCase"))
                .unwrap();
            assert!(
                diagnostic.message.contains("number"),
                "only the unsaved external script declares numeric extra: {}",
                diagnostic.message
            );
        });
        definition(&fixture, case, visual);
        references(&fixture, case, visual);
        rename(&fixture, case, visual);
        assert_script_primary(&fixture, &case.script, visual);
        projects.assert_unchanged();
        eprintln!(
            "external native phase: {} / {} PASS exact diagnostic, F12, references, preview/cancel/apply and group Undo",
            case.version, case.extension
        );
    }
    let plans = fixture
        .actor
        .state
        .lock()
        .unwrap()
        .language_services
        .values()
        .filter_map(|plan| plan.as_ref().ok().cloned())
        .collect::<Vec<_>>();
    assert!(
        !plans.is_empty(),
        "external semantics must use managed services"
    );
    shutdown(&fixture, visual);
    assert!(
        plans
            .iter()
            .all(|plan| !plan.is_active() && plan.process_count() == 0),
        "external script contexts must retire after checked shutdown"
    );
    projects.assert_unchanged();
}

/// Focus the existing DocumentSession before any real keyboard or native menu action.
fn open(fixture: &Fixture, path: &Path, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        window.activate_window();
        fixture
            .app
            .update(cx, |app, cx| app.open_file(path.to_owned(), window, cx));
        fixture
            .app
            .read(cx)
            .editor
            .clone()
            .update(cx, |editor, cx| editor.focus(window, cx));
        window.draw(cx).clear(cx);
    });
}

/// Replace only the native editor's current text; fixture strings are immutable expected preimages.
fn replace(fixture: &Fixture, value: &str, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        fixture
            .app
            .read(cx)
            .editor
            .clone()
            .update(cx, |editor, cx| {
                editor.set_selected_range(0..editor.text().len(), cx);
                editor.replace_text_in_range(None, value, window, cx);
                editor.focus(window, cx);
                cx.notify();
            });
    });
}

/// The ordinary selected service must publish real language providers before an action is issued.
fn ready(fixture: &Fixture, path: &Path, visual: &mut VisualTestContext) {
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "external native primary",
        180,
        |app, cx| {
            for state in app.language_service_states.values() {
                if let crate::app::language_servers::ServiceLoadState::Failed(error) = state {
                    panic!("external source preparation failed: {error}");
                }
            }
            app.language_server_for_path(path)
                .is_some_and(|server| server.is_active())
                && app.editor.read(cx).lsp().definition_provider.is_some()
                && app.editor.read(cx).lsp().completion_provider.is_some()
        },
    );
}

/// Auxiliary Vue ownership may select a project, but must never replace a script's TS primary.
fn assert_script_primary(fixture: &Fixture, path: &Path, visual: &mut VisualTestContext) {
    visual.update(|_, cx| {
        let app = fixture.app.read(cx);
        let language = app.language_for_path(path);
        assert_eq!(
            app.providers
                .language_servers()
                .get(&language)
                .and_then(Option::as_deref),
            Some("typescript/analysis"),
            "an associated script retains its primary provider"
        );
        let plan = app
            .language_server_for_path(path)
            .unwrap()
            .navigation_context_service()
            .unwrap();
        assert_eq!(plan.owner, "typescript");
    });
}

/// Source byte positions include astral and combining Unicode before the symbol on the same line.
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
                cx.notify();
            });
        window.draw(cx).clear(cx);
    });
}

/// Actual F12 must focus the original external declaration and retain its unsaved native text.
fn definition(fixture: &Fixture, case: &Case, visual: &mut VisualTestContext) {
    caret(
        fixture,
        case.component_dirty.find("message").unwrap() + 1,
        visual,
    );
    visual.simulate_keystrokes("f12");
    let start = case.script_dirty.find("message").unwrap();
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "external F12 original declaration",
        30,
        |app, cx| {
            app.active_path.as_deref() == Some(case.script.as_path())
                && app.editor.read(cx).cursor() == start
        },
    );
    ready(fixture, &case.script, visual);
    assert_eq!(source(fixture, &case.script, visual), case.script_dirty);
    assert_eq!(
        source(fixture, &case.component, visual),
        case.component_dirty
    );
    assert_script_primary(fixture, &case.script, visual);
}

/// Painted References rows must name each original SFC/script position exactly once in UTF-16.
fn references(fixture: &Fixture, case: &Case, visual: &mut VisualTestContext) {
    open(fixture, &case.component, visual);
    ready(fixture, &case.component, visual);
    caret(
        fixture,
        case.component_dirty.find("message").unwrap() + 1,
        visual,
    );
    visual.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            app.request_language_feature(
                plugin_runtime::plugin_protocol::direct::Feature::References,
                window,
                cx,
            )
        })
    });
    let mut expected = [
        (&case.component, &case.component_dirty),
        (&case.script, &case.script_dirty),
    ]
    .into_iter()
    .flat_map(|(path, text)| {
        let relative = path
            .strip_prefix(&fixture.workspace)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        text.match_indices("message").map(move |(offset, _)| {
            // This derives the expectation from the original fixture, not the host's LSP mapper.
            let prefix = &text[..offset];
            let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
            let column = prefix.rsplit('\n').next().unwrap().encode_utf16().count() + 1;
            format!("{relative}:{line}:{column}")
        })
    })
    .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(
        expected.len(),
        2,
        "one original declaration and one template use"
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut observed = loop {
        wait(
            &fixture.app,
            &fixture.actor,
            visual,
            "external References frame",
            1,
            |_, _| true,
        );
        let rows = visual.update(|window, _| {
            gpui_base::test_support::snapshots(window)
                .into_iter()
                .filter(|node| node.visible() && node.role() == Some(gpui_kit::Role::Button))
                .filter_map(|node| node.label().map(str::to_owned))
                .filter(|label| {
                    label.contains(':')
                        && [".vue", ".ts", ".js"]
                            .iter()
                            .any(|extension| label.contains(extension))
                })
                .collect::<Vec<_>>()
        });
        if !rows.is_empty() {
            break rows;
        }
        assert!(
            Instant::now() < deadline,
            "external References produced no native source rows"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    observed.sort();
    assert_eq!(
        observed, expected,
        "references must contain only original dirty source coordinates"
    );
    visual.simulate_keystrokes("escape");
}

/// Preview and cancel preserve both dirty sessions; confirm and group Undo cross the same transaction UI.
fn rename(fixture: &Fixture, case: &Case, visual: &mut VisualTestContext) {
    for apply in [false, true] {
        open(fixture, &case.component, visual);
        ready(fixture, &case.component, visual);
        caret(
            fixture,
            case.component_dirty.find("message").unwrap() + 1,
            visual,
        );
        visual.simulate_keystrokes("f2");
        control(fixture, visual, "editor-rename-prompt");
        visual.simulate_input("caption");
        visual.simulate_keystrokes("enter");
        control(fixture, visual, "workspace-plan-confirm");
        assert!(
            visual.debug_bounds("workspace-plan-details").is_some(),
            "external rename requires its full native preview"
        );
        await_sources(
            fixture,
            case,
            &case.component_dirty,
            &case.script_dirty,
            "external preview unchanged",
            visual,
        );
        case.assert_saved();
        click(
            visual,
            if apply {
                "workspace-plan-confirm"
            } else {
                "workspace-plan-cancel"
            },
        );
        control_visible(fixture, visual, "workspace-plan-confirm", false);
        if !apply {
            await_sources(
                fixture,
                case,
                &case.component_dirty,
                &case.script_dirty,
                "external cancel unchanged",
                visual,
            );
            case.assert_saved();
        }
    }
    let component_expected = case.component_dirty.replace("message", "caption");
    let script_expected = case.script_dirty.replace("message", "caption");
    await_sources(
        fixture,
        case,
        &component_expected,
        &script_expected,
        "external complete rename postimage",
        visual,
    );
    assert_eq!(source(fixture, &case.component, visual), component_expected);
    assert_eq!(source(fixture, &case.script, visual), script_expected);
    case.assert_saved();
    super::super::vue_test_support::receipts::undo(fixture, visual, "external Undo native frame");
    await_sources(
        fixture,
        case,
        &case.component_dirty,
        &case.script_dirty,
        "external group Undo originals",
        visual,
    );
    assert_eq!(
        source(fixture, &case.component, visual),
        case.component_dirty
    );
    assert_eq!(source(fixture, &case.script, visual), case.script_dirty);
    case.assert_saved();
}

/// Observe both original sessions together; neither partial application nor a disk fallback can pass.
fn await_sources(
    fixture: &Fixture,
    case: &Case,
    component: &str,
    script: &str,
    phase: &'static str,
    visual: &mut VisualTestContext,
) {
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        phase,
        15,
        |app, cx| {
            [(&case.component, component), (&case.script, script)]
                .into_iter()
                .all(|(path, expected)| {
                    app.tabs
                        .iter()
                        .find(|tab| tab.path() == path)
                        .and_then(|tab| tab.text.as_ref())
                        .is_some_and(|text| text.editor.read(cx).text().to_string() == expected)
                })
        },
    );
}
