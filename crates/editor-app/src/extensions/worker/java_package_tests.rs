//! Independently packaged Java enters the production grammar loader, actor and native document path.
use super::vue_test_support::{input, setup_for_workspace, shutdown, wait};
use super::*;
use gpui_base::input::RopeExt;
use gpui_kit::{TestAppContext, gpui};
mod code_actions;
mod debugging;
mod execution;
mod rename;

/// The package's actual grammar and query files must parse Java through the host's WASM loader.
#[test]
#[ignore = "Build Java with --plugin-package and set NANOBUG_JAVA_PACKAGE to the resulting ZIP"]
fn java_package_grammar_uses_the_native_loader() {
    let package = Package::read(&input("NANOBUG_JAVA_PACKAGE")).unwrap();
    let declarations = plugin_schema::PluginManifest::parse(
        std::str::from_utf8(&package.files[package.manifest.contributions.as_ref().unwrap()])
            .unwrap(),
    )
    .unwrap();
    let declaration = declarations.highlighters.first().unwrap().clone();
    let directory = tempfile::tempdir().unwrap();
    for relative in [&declaration.grammar, &declaration.highlights] {
        let target = directory.path().join(relative);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(
            target,
            &package.files[&relative.to_string_lossy().replace('\\', "/")],
        )
        .unwrap();
    }
    let provider = crate::language::providers::GrammarProvider {
        owner: package.manifest.id,
        root: directory.path().canonicalize().unwrap(),
        declaration,
    };
    let (grammar, _) = crate::language::plugins::prepare_dynamic(&provider).unwrap();
    let (mut parser, language) = crate::language::plugins::readonly_parser(grammar).unwrap();
    parser.set_language(&language).unwrap();
    let tree = parser
        .parse("public class Main { String message = \"中文😀\"; }", None)
        .unwrap();
    assert!(!tree.root_node().has_error());
    assert_eq!(tree.root_node().kind(), "program");
    for (version, source) in [
        (8, "class Main { Runnable task = () -> {}; }"),
        (11, "class Main { void test() { var value = \"text\"; } }"),
        (17, "record Point(int x, int y) {}"),
        (
            21,
            "class Main { int value(Object input) { return switch(input) { case String text when !text.isEmpty() -> text.length(); default -> 0; }; } }",
        ),
        (25, "void main() { java.lang.IO.println(\"中文\"); }"),
    ] {
        let tree = parser.parse(source, None).unwrap();
        assert!(
            !tree.root_node().has_error(),
            "Java {version}: {}",
            tree.root_node().to_sexp()
        );
    }
    if std::env::var_os("NANOBUG_JAVA_MODULE_IMPORT").is_some() {
        let tree = parser
            .parse(
                "import module java.base; void main() { IO.println(\"hello\"); }",
                None,
            )
            .unwrap();
        assert!(
            !tree.root_node().has_error(),
            "Java 25 module import: {}",
            tree.root_node().to_sexp()
        );
    }
}

/// Unsaved Unicode input receives real semantic diagnostics on its native UTF-8 range.
#[gpui::test]
#[ignore = "Build Java with --plugin-package and set NANOBUG_JAVA_PACKAGE to the resulting ZIP"]
fn java_package_analyzes_native_unsaved_unicode_sources(cx: &mut TestAppContext) {
    let workspace = tempfile::Builder::new()
        .prefix("Java 中文 空格 ")
        .tempdir()
        .unwrap();
    std::fs::write(workspace.path().join("Main.java"), "public class Main {}\n").unwrap();
    let (fixture, visual) =
        setup_for_workspace(cx, &["NANOBUG_JAVA_PACKAGE"], workspace.path().into());
    let path = fixture.workspace.join("Main.java").canonicalize().unwrap();
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java provider discovery",
        30,
        |app, _| {
            app.providers
                .language_servers()
                .get(&app.language_for_path(&path))
                .is_some_and(Option::is_some)
        },
    );
    visual.update(|window, cx| {
        window.activate_window();
        fixture
            .app
            .update(cx, |app, cx| app.open_file(path.clone(), window, cx));
        window.draw(cx).clear(cx);
    });
    let source = "public class Main { String label = \"library\"; void test() { /* 中文😀 */ int number = \"wrong\"; } }\n";
    visual.simulate_keystrokes("ctrl-a");
    visual.simulate_input(source);
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java native semantic diagnostic",
        180,
        |app, cx| {
            for state in app.language_service_states.values() {
                if let crate::app::language_servers::ServiceLoadState::Failed(error) = state {
                    panic!("Java package preparation failed: {error}");
                }
            }
            app.editor.read(cx).diagnostics().is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item.message.contains("Type mismatch"))
            })
        },
    );
    visual.update(|_, cx| {
        let app = fixture.app.read(cx);
        let editor = app.editor.read(cx);
        assert_eq!(editor.text().to_string(), source);
        let diagnostic = editor
            .diagnostics()
            .unwrap()
            .iter()
            .find(|item| item.message.contains("Type mismatch"))
            .unwrap();
        assert_eq!(source.get(diagnostic.range.clone()), Some("\"wrong\""));
        assert!(editor.lsp().completion_provider.is_some());
        assert!(editor.lsp().hover_provider.is_some());
    });
    // The published definition action must open library content in the ordinary readonly editor.
    visual.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            app.editor.update(cx, |editor, cx| {
                let position = editor
                    .text()
                    .offset_to_position(source.find("String").unwrap() + 2);
                editor.set_cursor_position(position, window, cx);
            });
        });
    });
    visual.simulate_keystrokes("f12");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "JDK readonly source navigation",
        60,
        |app, cx| {
            app.active_text_tab_index().is_some_and(|index| {
                app.tabs[index].virtual_document.is_some()
                    && app.tabs[index].text.as_ref().is_some_and(|tab| {
                        tab.session.is_readonly()
                            && tab
                                .editor
                                .read(cx)
                                .text()
                                .to_string()
                                .contains("class String")
                    })
            })
        },
    );
    let library = visual.update(|_, cx| fixture.app.read(cx).editor.read(cx).text().to_string());
    visual.simulate_input("must not modify library");
    visual.update(|_, cx| {
        assert_eq!(
            fixture.app.read(cx).editor.read(cx).text().to_string(),
            library
        );
    });
    visual.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.open_file(path.clone(), window, cx));
    });
    visual.simulate_keystrokes("ctrl-s");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Save source before closing its library origin",
        30,
        |app, _| {
            app.tabs
                .iter()
                .find(|tab| tab.path() == path)
                .is_some_and(|tab| !tab.is_dirty())
        },
    );
    visual.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            let resource = app
                .tabs
                .iter()
                .find_map(|tab| tab.virtual_document.as_ref())
                .unwrap()
                .resource
                .clone();
            assert!(
                resource
                    .content_origin()
                    .unwrap()
                    .location
                    .uri
                    .starts_with("jdt:")
            );
            assert!(
                resource
                    .content_origin()
                    .unwrap()
                    .location
                    .selection
                    .is_some()
            );
            app.close_tab(path.clone(), window, cx);
            assert!(
                app.tabs.iter().all(|tab| tab.path() != path),
                "The initiating source actually closes"
            );
            assert!(
                resource.is_live(),
                "A readonly library tab outlives its initiating source"
            );
        });
    });
    shutdown(&fixture, visual);
}
