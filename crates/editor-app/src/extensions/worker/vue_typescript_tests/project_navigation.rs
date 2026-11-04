//! Read-only acceptance against an explicitly selected Vue project and installed package inputs.
use super::super::vue_test_support::{input, setup_for_workspace, shutdown, wait};
use super::*;

/// Opening App.vue must prepare Vue before the first definition request, including large lockfiles.
#[gpui::test]
#[ignore = "set NANOBUG_VUE_NAVIGATION_WORKSPACE, NANOBUG_VUE_PACKAGE and NANOBUG_JAVASCRIPT_PACKAGE"]
fn existing_vue_project_app_starts_and_navigates(cx: &mut TestAppContext) {
    navigate_project_import(
        cx,
        "src/App.vue",
        "ant-design-vue/lib/locale-provider/zh_CN",
        "node_modules/ant-design-vue/lib/locale-provider/zh_CN",
    );
}

/// Opening main.js must prepare its JS provider and resolve the entry component's extensionless import.
#[gpui::test]
#[ignore = "set NANOBUG_VUE_NAVIGATION_WORKSPACE, NANOBUG_VUE_PACKAGE and NANOBUG_JAVASCRIPT_PACKAGE"]
fn existing_vue_project_main_starts_and_navigates(cx: &mut TestAppContext) {
    navigate_project_import(cx, "src/main.js", "@/App", "src/App.vue");
}

/// `relative` and `specifier` select a real import; its result must stay under the expected path.
/// The test reads the supplied project without installing dependencies or editing its source files.
fn navigate_project_import(
    cx: &mut TestAppContext,
    relative: &str,
    specifier: &str,
    expected: &str,
) {
    let workspace = input("NANOBUG_VUE_NAVIGATION_WORKSPACE");
    eprintln!("project navigation: install isolated packages for {relative}");
    let (fixture, visual) = setup_for_workspace(
        cx,
        &["NANOBUG_JAVASCRIPT_PACKAGE", "NANOBUG_VUE_PACKAGE"],
        workspace,
    );
    let app = fixture.app.clone();
    let actor = fixture.actor.clone();
    let path = fixture.workspace.join(relative).canonicalize().unwrap();
    let original = std::fs::read(&path).unwrap();
    eprintln!("project navigation: open {relative}");
    visual.update(|window, cx| {
        window.activate_window();
        app.update(cx, |app, cx| app.open_file(path.clone(), window, cx));
        window.draw(cx).clear(cx);
    });
    // No feature request or key is sent before readiness: navigation cannot mask missing autostart.
    let startup_error = RefCell::new(None);
    wait(
        &app,
        &actor,
        visual,
        "automatic language startup",
        180,
        |app, cx| {
            for state in app.language_service_states.values() {
                if let crate::app::language_servers::ServiceLoadState::Failed(error) = state {
                    *startup_error.borrow_mut() = Some(error.clone());
                    return true;
                }
            }
            app.language_server_for_path(&path)
                .is_some_and(|server| server.is_active())
                && app.editor.read(cx).lsp().definition_provider.is_some()
        },
    );
    if let Some(error) = startup_error.into_inner() {
        // Retire real workers while the deterministic UI executor is still pumping their requests.
        eprintln!("project navigation: automatic startup failed: {error}");
        shutdown(&fixture, visual);
        panic!("automatic startup for {relative} failed: {error}");
    }
    eprintln!("project navigation: automatic startup ready for {relative}");
    let result = Rc::new(RefCell::new(None));
    let capture = result.clone();
    visual.update(|window, cx| {
        let (source, provider) = {
            let editor = app.read(cx).editor.read(cx);
            (
                editor.text().clone(),
                editor.lsp().definition_provider.clone().unwrap(),
            )
        };
        let offset = source
            .to_string()
            .find(specifier)
            .expect("selected import exists")
            + 1;
        let request = provider.definitions(&source, offset, window, cx);
        app.update(cx, |_, cx| {
            cx.spawn(async move |_, _| *capture.borrow_mut() = Some(request.await))
                .detach();
        });
    });
    wait(&app, &actor, visual, "import definition", 60, |_, _| {
        result.borrow().is_some()
    });
    let definitions = result.borrow_mut().take().unwrap();
    shutdown(&fixture, visual);
    let definitions = definitions.expect("import request succeeds");
    let targets = definitions
        .iter()
        .map(|definition| {
            url::Url::parse(definition.target_uri.as_str())
                .unwrap()
                .to_file_path()
                .unwrap()
                .canonicalize()
                .unwrap()
                .strip_prefix(&fixture.workspace)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect::<Vec<_>>();
    assert!(
        targets.iter().any(|target| target.starts_with(expected)),
        "{relative} import {specifier} did not resolve to {expected}: {targets:?}"
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        original,
        "acceptance must preserve source bytes"
    );
    eprintln!("project navigation: {relative} import resolved to {targets:?}");
}
