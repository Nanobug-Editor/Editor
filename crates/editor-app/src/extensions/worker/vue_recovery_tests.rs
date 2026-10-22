//! Real installed compatibility backends retain independent transport and trust lifetimes.
use super::vue_test_support::{Fixture, setup, shutdown, wait};
use super::*;
use crate::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};
use std::cell::RefCell;

/// Terminate only the selected managed plan, observe its real pipe failure and recover Vue semantics.
/// The other compatibility backend must remain the same live transport throughout.
#[gpui::test]
#[ignore = "set NANOBUG_TYPESCRIPT_PACKAGE, NANOBUG_VUE_PACKAGE and NANOBUG_VUE_FIXTURES to real candidates"]
fn real_vue_backend_exit_is_scoped_and_trust_revocation_reaps_all_workers(cx: &mut TestAppContext) {
    let (fixture, visual) = setup(cx, &["NANOBUG_TYPESCRIPT_PACKAGE", "NANOBUG_VUE_PACKAGE"]);
    let paths = ["vue26", "vue3"].map(|name| {
        fixture
            .workspace
            .join(format!("tests/projects/{name}/App.vue"))
            .canonicalize()
            .unwrap()
    });
    let originals = paths
        .each_ref()
        .map(|path| std::fs::read_to_string(path).unwrap());
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "cold languages",
        30,
        |app, _| {
            app.providers
                .language_servers()
                .get("vue")
                .is_some_and(Option::is_some)
        },
    );
    for path in &paths {
        visual.update(|window, cx| {
            window.activate_window();
            fixture
                .app
                .update(cx, |app, cx| app.open_file(path.clone(), window, cx));
            window.draw(cx).clear(cx);
        });
        visual.simulate_input("<!-- recovery unsaved 中文😀 -->\n");
    }
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "independent Vue backends",
        180,
        |app, cx| {
            paths.iter().all(|path| {
                app.tabs
                    .iter()
                    .find(|tab| tab.path() == *path)
                    .and_then(|tab| tab.text.as_ref())
                    .is_some_and(|tab| tab.editor.read(cx).lsp().hover_provider.is_some())
            })
        },
    );
    let servers = visual.update(|_, cx| {
        paths.each_ref().map(|path| {
            let app = fixture.app.read(cx);
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
            (
                app.language_server_for_path(path).unwrap().clone(),
                editor.text().to_string(),
            )
        })
    });
    let plans = {
        let state = fixture.actor.state.lock().unwrap();
        paths.each_ref().map(|path| {
            let relative = path
                .strip_prefix(&fixture.workspace)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let (_, key) = state
                .language_document_bindings
                .iter()
                .find(|((provider, document), _)| {
                    provider == "vue/vue" && document.path == relative
                })
                .unwrap();
            state.language_services[key].as_ref().unwrap().clone()
        })
    };
    assert!(!Arc::ptr_eq(&plans[0], &plans[1]));
    for index in 0..2 {
        let (server, source) = servers[index].clone();
        let uri = crate::language::navigation::file_uri(&paths[index]).unwrap();
        let position = crate::language::navigation::position_at_byte(
            &source,
            source.rfind("message").unwrap() + 1,
        );
        let hover = job(&fixture, visual, "initial real hover", move || {
            server.hover(uri, source, position)
        })
        .unwrap();
        assert!(hover.is_some());
        assert_eq!(plans[index].process_count(), 1);
    }
    // This is the existing plan-scoped managed stop operation, not a process-name kill or a new
    // test API. The UI connection still believes it is live, so its next request observes real EOF.
    plans[0].stop_processes();
    assert_eq!(plans[0].process_count(), 0);
    assert_eq!(plans[1].process_count(), 1);
    let (server, source) = servers[0].clone();
    let uri = crate::language::navigation::file_uri(&paths[0]).unwrap();
    let position = crate::language::navigation::position_at_byte(
        &source,
        source.rfind("message").unwrap() + 1,
    );
    let failure = job(&fixture, visual, "terminated backend request", move || {
        server.hover(uri, source, position)
    });
    assert!(
        failure.is_err(),
        "an interrupted request must not be replayed as a success"
    );
    assert!(servers[0].0.recovery_status().is_some());
    let (unaffected, source) = servers[1].clone();
    let uri = crate::language::navigation::file_uri(&paths[1]).unwrap();
    let position = crate::language::navigation::position_at_byte(
        &source,
        source.rfind("message").unwrap() + 1,
    );
    assert!(
        job(&fixture, visual, "unaffected backend hover", move || {
            unaffected.hover(uri, source, position)
        })
        .unwrap()
        .is_some()
    );
    assert_eq!(plans[1].process_count(), 1);
    let recovering = servers[0].0.clone();
    job(&fixture, visual, "bounded backend recovery", move || {
        recovering.prepare_until_ready()
    })
    .unwrap();
    let (recovered, source) = servers[0].clone();
    let uri = crate::language::navigation::file_uri(&paths[0]).unwrap();
    let position = crate::language::navigation::position_at_byte(
        &source,
        source.rfind("message").unwrap() + 1,
    );
    let hover = job(
        &fixture,
        visual,
        "recovered Vue semantic hover",
        move || recovered.hover(uri, source, position),
    )
    .unwrap();
    assert!(
        hover.is_some(),
        "recovery must restore the adapter, not merely the stdio handshake"
    );
    visual.update(|_, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.set_workspace_trusted(false, cx))
    });
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "trust revocation",
        30,
        |app, _| {
            !app.session_state.workspace_trusted
                && plans
                    .iter()
                    .all(|plan| !plan.is_active() && plan.process_count() == 0)
        },
    );
    assert!(servers.iter().all(|(server, _)| !server.is_active()));
    for (path, original) in paths.iter().zip(originals) {
        assert_eq!(std::fs::read_to_string(path).unwrap(), original);
    }
    shutdown(&fixture, visual);
}

/// Keep actual transport IO off the native UI executor while frames publish normal lifecycle work.
fn job<T: Send + Sync + 'static>(
    fixture: &Fixture,
    visual: &mut VisualTestContext,
    label: &str,
    operation: impl FnOnce() -> T + Send + 'static,
) -> T {
    let result = Rc::new(RefCell::new(None));
    let delivered = result.clone();
    visual.update(|_, cx| {
        fixture.app.update(cx, |_, cx| {
            cx.spawn(async move |_, cx| {
                let value = cx
                    .background_executor()
                    .scheduler_executor()
                    .spawn_dedicated(move |_| async move { operation() })
                    .await;
                *delivered.borrow_mut() = Some(value);
            })
            .detach();
        })
    });
    wait(&fixture.app, &fixture.actor, visual, label, 45, |_, _| {
        result.borrow().is_some()
    });
    result.borrow_mut().take().unwrap()
}
