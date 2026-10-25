//! Actual Maven import and Java source controls enter the native actor and EditorState stream.
use super::vue_test_support::{setup_for_workspace, shutdown};
use super::*;
use crate::EditorApp;
use gpui_base::input::RopeExt;
use gpui_kit::{Entity, TestAppContext, VisualTestContext, gpui};
use std::time::{Duration, Instant};

// Share the real offline JAR fixture, rather than mocking dependency resolution in the UI test.
#[path = "../../../../plugin-runtime/tests/maven_package/repository.rs"]
mod repository;

/// Unsaved Unicode and binary navigation must survive preparation before the first source opens.
#[gpui::test]
#[ignore = "Build Maven and Java; set NANOBUG_MAVEN_PACKAGE and NANOBUG_JAVA_PACKAGE"]
fn maven_package_analyzes_native_unsaved_sources_and_opens_dependency(cx: &mut TestAppContext) {
    let workspace = tempfile::Builder::new()
        .prefix("Maven 中文 空格 ")
        .tempdir()
        .unwrap();
    for module in ["app", "lib"] {
        std::fs::create_dir_all(workspace.path().join(format!("{module}/src/main/java"))).unwrap();
    }
    std::fs::write(workspace.path().join("pom.xml"),r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><groupId>sample</groupId><artifactId>native</artifactId><version>1</version><packaging>pom</packaging><modules><module>lib</module><module>app</module></modules></project>"#).unwrap();
    for module in ["lib", "app"] {
        let dependencies = if module == "app" {
            "<dependencies><dependency><groupId>sample</groupId><artifactId>lib</artifactId><version>1</version></dependency><dependency><groupId>fixture</groupId><artifactId>api</artifactId><version>1</version></dependency></dependencies>"
        } else {
            ""
        };
        std::fs::write(workspace.path().join(format!("{module}/pom.xml")),format!(r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><groupId>sample</groupId><artifactId>{module}</artifactId><version>1</version><properties><maven.compiler.release>25</maven.compiler.release></properties>{dependencies}</project>"#)).unwrap();
    }
    std::fs::write(
        workspace.path().join("app/src/main/java/Main.java"),
        "public class Main {}\n",
    )
    .unwrap();
    std::fs::write(
        workspace.path().join("lib/src/main/java/Library.java"),
        "public class Library {}\n",
    )
    .unwrap();
    let (fixture, visual) = setup_for_workspace(
        cx,
        &["NANOBUG_MAVEN_PACKAGE", "NANOBUG_JAVA_PACKAGE"],
        workspace.path().into(),
    );
    eprintln!("Maven native fixture store: {}", fixture.store.display());
    let repository = fixture.store.parent().unwrap().join("m2");
    repository::seed(&repository);
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Maven command discovery",
        30,
        |_, _| {
            fixture
                .actor
                .state
                .lock()
                .unwrap()
                .command_epoch("maven", "import")
                .is_some()
        },
    );
    let previous_configuration =
        fixture.actor.state.lock().unwrap().availabilities["maven"].configuration_generation();
    fixture
        .actor
        .tx
        .send(Work::SetSetting {
            request: 1,
            plugin: "maven".into(),
            scope: settings::Scope::Project,
            key: "maven.repository".into(),
            value: Some(serde_json::json!(repository)),
        })
        .unwrap();
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Private Maven repository",
        120,
        |_, _| {
            let state = fixture.actor.state.lock().unwrap();
            let errors = state
                .logs
                .records("maven")
                .into_iter()
                .filter(|record| record.level == plugin_runtime::LogLevel::Error)
                .map(|record| record.message)
                .collect::<Vec<_>>();
            assert!(errors.is_empty(), "Native Maven command errors: {errors:?}");
            match &state.configuration_result {
                Some((1, result)) => {
                    assert!(result.is_ok(), "{result:?}");
                    state.availabilities.get("maven").is_some_and(|token| {
                        token.configuration_generation() != previous_configuration
                    })
                }
                _ => false,
            }
        },
    );
    {
        let state = fixture.actor.state.lock().unwrap();
        fixture
            .actor
            .tx
            .send(Work::Invoke {
                plugin: "maven".into(),
                command: "import".into(),
                arguments: serde_json::json!({"pom":"pom.xml","profiles":[],"offline":true}),
                context: None,
                expected_epoch: state.command_epoch("maven", "import").unwrap(),
                origin: state
                    .availabilities
                    .get("maven")
                    .cloned()
                    .map(|availability| super::demands::CommandOrigin {
                        availability,
                        guard: None,
                    }),
            })
            .unwrap();
    }
    let last_model_view = std::cell::Cell::new(0);
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Native Maven imported model",
        180,
        |_, _| {
            let state = fixture.actor.state.lock().unwrap();
            if let Some(view) = state.views.get("maven/project") {
                let text = serde_json::to_string(view).unwrap();
                if last_model_view.replace(view.revision) != view.revision {
                    eprintln!("Maven model view: {text}");
                }
                assert!(!text.contains("Import cancelled"), "{text}");
                text.contains("sample:native:1")
            } else {
                false
            }
        },
    );
    visual.update(|_, cx| {
        fixture
            .app
            .read(cx)
            .providers
            .choose(settings::Scope::Project, "lsp:java", Some("maven/analysis"))
            .unwrap()
    });
    let library = fixture
        .workspace
        .join("lib/src/main/java/Library.java")
        .canonicalize()
        .unwrap();
    eprintln!("Native Maven: opening unsaved library");
    visual.update(|window, cx| {
        window.activate_window();
        fixture
            .app
            .update(cx, |app, cx| app.open_file(library.clone(), window, cx));
        window.draw(cx).clear(cx);
    });
    visual.simulate_keystrokes("ctrl-a");
    visual.simulate_input(
        "public class Library { public String message() { return \"未保存\"; } }\n",
    );
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Native library context",
        180,
        |app, _| {
            app.language_server_for_path(&library)
                .is_some_and(|server| server.is_active())
        },
    );
    let path = fixture
        .workspace
        .join("app/src/main/java/Main.java")
        .canonicalize()
        .unwrap();
    eprintln!("Native Maven: opening application");
    visual.update(|window, cx| {
        window.activate_window();
        fixture
            .app
            .update(cx, |app, cx| app.open_file(path.clone(), window, cx));
        window.draw(cx).clear(cx);
    });
    let source = "import fixture.Api;\npublic class Main { String 输出 = \"中文😀\"; String message = new Library().message(); int value = Api.answer(); int broken = \"wrong\"; }\n";
    visual.simulate_keystrokes("ctrl-a");
    visual.simulate_input(source);
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Maven native unsaved diagnostic",
        180,
        |app, cx| {
            for state in app.language_service_states.values() {
                if let crate::app::language_servers::ServiceLoadState::Failed(error) = state {
                    panic!(
                        "Maven language preparation failed: {error}; Java logs {:?}",
                        fixture.actor.state.lock().unwrap().logs.records("java")
                    );
                }
            }
            app.editor.read(cx).diagnostics().is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item.message.contains("Type mismatch"))
            })
        },
    );
    visual.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            let editor = app.editor.read(cx);
            assert!(Arc::ptr_eq(app.language_server_for_path(&path).unwrap(),app.language_server_for_path(&library).unwrap()),"Reactor documents must share one analysis transport and their unsaved working copies");
            assert_eq!(editor.text().to_string(), source);
            let errors = editor
                .diagnostics()
                .unwrap()
                .iter()
                .filter(|item| item.severity == gpui_base::input::DiagnosticSeverity::Error)
                .collect::<Vec<_>>();
            assert!(
                errors
                    .iter()
                    .all(|item| !item.message.contains("cannot be resolved")),
                "{errors:?}"
            );
            app.editor.update(cx, |editor, cx| {
                let position = editor
                    .text()
                    .offset_to_position(source.find("Api").unwrap() + 1);
                editor.set_cursor_position(position, window, cx);
            });
        })
    });
    visual.simulate_keystrokes("f12");
    eprintln!("Native Maven: waiting for binary definition");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Maven dependency readonly navigation",
        60,
        |app, cx| {
            app.active_text_tab_index().is_some_and(|index| {
                app.tabs[index].virtual_document.is_some()
                    && app.tabs[index].text.as_ref().is_some_and(|tab| {
                        tab.session.is_readonly()
                            && tab.editor.read(cx).text().to_string().contains("class Api")
                    })
            })
        },
    );
    let content = visual.update(|_, cx| fixture.app.read(cx).editor.read(cx).text().to_string());
    eprintln!("Native Maven: readonly definition ready");
    visual.simulate_input("must stay readonly");
    visual.update(|_, cx| {
        assert_eq!(
            fixture.app.read(cx).editor.read(cx).text().to_string(),
            content
        )
    });
    // FileWatch deliberately disables native threads under cfg(test). Feed its ordinary completed
    // reconciliation through the production UI path; native watcher delivery has separate coverage.
    let pom = fixture.workspace.join("pom.xml");
    let previous = fixture
        .actor
        .state
        .lock()
        .unwrap()
        .language_services
        .values()
        .filter_map(|service| service.as_ref().ok().cloned())
        .collect::<Vec<_>>();
    let changed = std::fs::read_to_string(&pom).unwrap().replace(
        "<artifactId>native</artifactId>",
        "<artifactId>changed</artifactId>",
    );
    std::fs::write(pom, changed).unwrap();
    eprintln!("Native Maven: reconciling POM");
    visual.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            app.apply_reconciliation(
                crate::editor::file_watch::Reconciliation {
                    file_events: vec![
                        plugin_runtime::plugin_protocol::workspace::FileEventKind::Modified {
                            path: "pom.xml".into(),
                        },
                    ],
                    snapshot: None,
                    documents: vec![],
                    renames: vec![],
                    native: true,
                },
                window,
                cx,
            )
        })
    });
    eprintln!("Native Maven: awaiting stale model");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Maven POM watcher invalidation",
        30,
        |_, _| {
            fixture
                .actor
                .state
                .lock()
                .unwrap()
                .views
                .get("maven/project")
                .is_some_and(|view| {
                    serde_json::to_string(view)
                        .unwrap()
                        .contains("Model is stale")
                })
        },
    );
    assert!(!previous.is_empty());
    assert!(previous.iter().all(|plan| !plan.is_active()));
    eprintln!("Native Maven: stale model observed; refreshing");
    {
        let state = fixture.actor.state.lock().unwrap();
        fixture
            .actor
            .tx
            .send(Work::Invoke {
                plugin: "maven".into(),
                command: "import".into(),
                arguments: serde_json::json!({"pom":"pom.xml","profiles":[],"offline":true}),
                context: None,
                expected_epoch: state.command_epoch("maven", "import").unwrap(),
                origin: state
                    .availabilities
                    .get("maven")
                    .cloned()
                    .map(|availability| super::demands::CommandOrigin {
                        availability,
                        guard: None,
                    }),
            })
            .unwrap();
    }
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Refreshed Maven module model",
        90,
        |_, _| {
            fixture
                .actor
                .state
                .lock()
                .unwrap()
                .views
                .get("maven/project")
                .is_some_and(|view| {
                    serde_json::to_string(view)
                        .unwrap()
                        .contains("sample:changed:1")
                })
        },
    );
    visual.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.open_file(path.clone(), window, cx))
    });
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Fresh Java context after Maven refresh",
        180,
        |app, _| {
            app.language_server_for_path(&path)
                .is_some_and(|server| server.is_active())
                && app
                    .language_service_states
                    .get("context:app/src/main/java/Main.java")
                    == Some(&crate::app::language_servers::ServiceLoadState::Ready)
        },
    );
    // A new source revision needs a new semantic reply from the refreshed JVM, not old diagnostics.
    let refreshed = source.replace(
        "int broken = \"wrong\";",
        "int broken = afterRefreshMissing;",
    );
    visual.simulate_keystrokes("ctrl-a");
    visual.simulate_input(&refreshed);
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Fresh semantic reply after Maven refresh",
        60,
        |app, cx| {
            app.editor.read(cx).diagnostics().is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item.message.contains("afterRefreshMissing"))
            })
        },
    );
    shutdown(&fixture, visual);
}

/// Pump a finite number of runnable tasks per frame. The upstream scheduler's drain helper also
/// advances to future timers, so a polling native actor can prevent it returning to deadline checks.
fn wait(
    app: &Entity<EditorApp>,
    actor: &Worker,
    visual: &mut VisualTestContext,
    label: &str,
    seconds: u64,
    predicate: impl Fn(&EditorApp, &gpui::App) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        visual.executor().advance_clock(Duration::from_millis(20));
        for _ in 0..128 {
            if !visual.executor().tick() {
                break;
            }
        }
        if visual.update(|window, cx| {
            app.read(cx)
                .extensions
                .clone()
                .update(cx, |panel, cx| panel.poll(cx));
            window.draw(cx).clear(cx);
            predicate(app.read(cx), cx)
        }) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{label}: language={:?}, Maven logs={:?}",
            visual.update(|_, cx| app.read(cx).language_service_states.clone()),
            actor
                .state
                .lock()
                .unwrap()
                .logs
                .records("maven")
                .iter()
                .map(|record| &record.message)
                .collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
