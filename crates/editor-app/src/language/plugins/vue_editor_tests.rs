//! Real EditorState drawing and input exercise package grammars in both native themes.
//! Language services are deliberately outside this grammar projection test; S1 tests their actor.
//! Programmatic marked-text callbacks verify the native IME bridge, not an OS candidate window.

use crate::language::highlighter::SyntaxHighlighter;
use crate::*;
use gpui_kit::{EntityInputHandler as _, TestAppContext, VisualTestContext, gpui};
use plugin_runtime::Package;
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

/// Observe actual capture names at the public theme-resolution seam without replacing styles.
struct ObservedStyles<'a> {
    theme: &'a dyn gpui_base::input::HighlightStyleResolver,
    captures: std::sync::Mutex<Vec<String>>,
}

impl gpui_base::input::HighlightStyleResolver for ObservedStyles<'_> {
    fn style(&self, name: &str) -> Option<gpui_kit::HighlightStyle> {
        self.captures.lock().unwrap().push(name.to_owned());
        self.theme.style(name)
    }
}

/// Complete production asynchronous grammar publication and the editor's first-frame setup.
fn paint_until_ready(app: &Entity<EditorApp>, visual: &mut VisualTestContext) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        visual.executor().advance_clock(Duration::from_millis(50));
        visual.run_until_parked();
        let ready = visual.update(|window, cx| {
            window.simulate_next_frame(cx);
            window.draw(cx).clear(cx);
            let app = app.read(cx);
            assert!(
                app.dynamic_languages
                    .entries
                    .iter()
                    .all(|(_, state)| state.is_ok()),
                "grammar load failure: {:?}",
                app.dynamic_languages.entries
            );
            app.dynamic_languages.entries.len() == 9
                && app
                    .dynamic_languages
                    .entries
                    .iter()
                    .all(|(_, state)| matches!(state, Ok(true)))
                && app.editor.read(cx).language_name().as_ref() != "text"
        });
        if ready {
            return;
        }
        if Instant::now() >= deadline {
            visual.update(|_, cx| {
                let state = app.read(cx);
                panic!(
                    "native grammar projection did not become ready: language={}, grammars={:?}, selections={:?}",
                    state.editor.read(cx).language_name(),
                    state.dynamic_languages.entries,
                    state.providers.rows().into_iter().map(|row| (row.key, row.selected)).collect::<Vec<_>>()
                );
            });
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Native layout, theme resolution, keyboard and IME all use the single active EditorState.
#[gpui::test]
#[ignore = "set NANOBUG_VUE_PACKAGE and NANOBUG_TYPESCRIPT_PACKAGE; run without other GPUI tests"]
fn vue_editor_native_highlights_themes_keyboard_and_ime(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::controls::init(cx);
        crate::bind_editor_shell_keys(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let path = directory.path().join("App.vue");
    let source = "<template><p>中文😀</p></template>\r\n<script setup lang=\"ts\">\r\nconst message: string = 'hello';\r\n</script>\r\n<style lang=\"scss\">\r\n$tone: red; .item { color: $tone; }\r\n</style>\r\n";
    std::fs::write(&path, source).unwrap();
    // Install actual packages before the UI creates its provider projection. Its initial native
    // catalog reads this same isolated store, without a hand-edited registry or foreign namespace.
    // Installation prepares static resources without starting an engine.
    {
        let environment = plugin_runtime::plugin_protocol::Environment {
            workspace: directory.path().display().to_string(),
            ..Default::default()
        };
        let mut manager =
            plugin_runtime::Manager::open(profile.path().join("runtime-plugins"), environment)
                .unwrap();
        for variable in ["NANOBUG_VUE_PACKAGE", "NANOBUG_TYPESCRIPT_PACKAGE"] {
            let package =
                Package::read(&PathBuf::from(std::env::var_os(variable).expect(variable))).unwrap();
            manager
                .install(&package, package.manifest.permissions.clone())
                .unwrap();
        }
    }
    let session = SessionState::load_in_profile(directory.path(), profile.path());
    let workspace = Workspace::open(directory.path()).unwrap();
    let slot = Rc::new(RefCell::new(None));
    let captured = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app =
            cx.new(|cx| EditorApp::new_with_session(workspace, None, Some(session), window, cx));
        *captured.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    visual.simulate_resize(size(px(1100.), px(760.)));
    visual.run_until_parked();
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.sync_dynamic_languages(cx);
            app.open_file(path.clone(), window, cx);
            window.activate_window();
        })
    });
    paint_until_ready(&app, visual);
    let mut palettes = Vec::new();
    for dark in [false, true] {
        visual.update(|window, cx| {
            apply_theme(builtin_theme(dark), cx);
            app.read(cx).editor.clone().update(cx, |editor, cx| {
                editor.focus(window, cx);
                editor.set_selected_range(0..0, cx);
            });
            window.draw(cx).clear(cx);
        });
        assert!(
            visual.debug_bounds("editor-source-pane").is_some(),
            "source editor must be painted"
        );
        let colors = visual.update(|window, cx| {
            let editor = app.read(cx).editor.read(cx);
            assert!(editor.focus_handle(cx).is_focused(window));
            assert_eq!(editor.text().to_string(), source);
            assert!(editor.range_to_bounds(&(0..0)).is_some());
            // This is the same public syntax factory/style resolver used by native input painting,
            // fed the current EditorState snapshot; it does not replace its mutable document.
            let parser_key = app.read(cx).providers.parser_key(&editor.language_name());
            let mut highlighter = SyntaxHighlighter::new(&parser_key);
            highlighter.update(None, editor.text(), None);
            assert!(highlighter.tree().is_some());
            let resolver = ObservedStyles {
                theme: cx.theme().highlight_theme.as_ref(),
                captures: Default::default(),
            };
            let styles = highlighter.styles(&(0..source.len()), &resolver);
            eprintln!(
                "visible grammar: dark={dark}, captures={:?}, variable={:?}",
                resolver.captures.lock().unwrap(),
                resolver.theme.style("variable.other.member")
            );
            ["string", "$tone"]
                .into_iter()
                .map(|needle| {
                    let offset = source.find(needle).unwrap();
                    styles
                        .iter()
                        .find(|(range, style)| range.contains(&offset) && style.color.is_some())
                        .map(|(_, style)| style.color.unwrap())
                        .unwrap_or_else(|| panic!("{needle} has no visible native syntax color (dark={dark}); styles={styles:?}"))
                })
                .collect::<Vec<_>>()
        });
        palettes.push(colors);
        // simulate_input delivers keystrokes separately; one key is one unambiguous Undo unit.
        // Multi-character composition is exercised through the native IME bridge below.
        visual.simulate_input("输");
        visual.run_until_parked();
        visual.update(|_, cx| {
            assert!(
                app.read(cx)
                    .editor
                    .read(cx)
                    .text()
                    .to_string()
                    .starts_with('输')
            )
        });
        visual.simulate_keystrokes("ctrl-z");
        visual.run_until_parked();
        visual.update(|_, cx| assert_eq!(app.read(cx).editor.read(cx).text().to_string(), source));
    }
    assert_ne!(
        palettes[0], palettes[1],
        "native syntax colors must resolve the active palette"
    );
    let editor = visual.update(|_, cx| app.read(cx).editor.clone());
    visual.update(|window, cx| {
        editor.update(cx, |editor, cx| {
            editor.replace_and_mark_text_in_range(None, "拼", Some(0..1), window, cx)
        })
    });
    visual.update(|window, cx| {
        editor.update(cx, |editor, cx| {
            editor.replace_text_in_range(None, "标签😀", window, cx)
        })
    });
    visual.run_until_parked();
    visual.update(|_, cx| {
        assert!(
            app.read(cx)
                .editor
                .read(cx)
                .text()
                .to_string()
                .starts_with("标签😀")
        )
    });
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        source,
        "input never saves the file implicitly"
    );
}
