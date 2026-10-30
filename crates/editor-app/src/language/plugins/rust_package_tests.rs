//! Independent Rust packages exercise comment boundaries through the native grammar/style seam.
//! Candidate archives use the shared hash-checked fixture override; no grammar ships with the host.

use super::*;
use crate::{ActiveTheme as _, apply_theme, builtin_theme};
use gpui_base::input::{HighlightStyleResolver as _, Rope};
use gpui_kit::{TestAppContext, gpui};

/// Every byte of a comment, including doc markers, must retain its theme's comment color.
#[gpui::test]
#[ignore = "set NANOBUG_TEST_PACKAGE_INPUTS to a built independent Rust archive ledger"]
fn rust_package_comment_delimiters(cx: &mut TestAppContext) {
    let root = crate::test_packages::root("rust");
    register_plugin(&root).unwrap();
    let source = "/// Built-in packaging 中文😀\r\n//! Run controls\r\n/** Outer block doc */\r\n/*! Inner block doc */\r\n/* Ordinary /* nested */ comment */\r\n// Plain comment\r\nfn main() {}\r\n";
    let mut highlighter = crate::language::highlighter::SyntaxHighlighter::new("rust");
    highlighter.update(None, &Rope::from(source), None);
    let tree = highlighter.tree().expect("loaded packaged WASM grammar");
    assert!(
        !tree.root_node().has_error(),
        "{}",
        tree.root_node().to_sexp()
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::init(cx);
        editor_ui::typography::init(cx);
        for dark in [false, true] {
            apply_theme(builtin_theme(dark), cx);
            let theme = cx.theme().highlight_theme.as_ref();
            let expected = theme.style("comment").unwrap().color;
            let styles = highlighter.styles(&(0..source.len()), theme);
            let failures: Vec<_> = source[..source.find("fn main").unwrap()]
                .char_indices()
                .filter(|(_, character)| !matches!(character, '\r' | '\n'))
                .filter_map(|(offset, character)| {
                    let actual = styles.iter().find(|(range, _)| range.contains(&offset));
                    (actual.and_then(|(_, style)| style.color) != expected)
                        .then_some((offset, character, actual))
                })
                .collect();
            assert!(
                failures.is_empty(),
                "dark={dark}, wrong comment styles: {failures:?}; tree={}",
                tree.root_node().to_sexp()
            );
        }
    });
}

/// Representative missing captures must resolve in both themes, with ordinary literals unchanged.
#[gpui::test]
#[ignore = "set NANOBUG_TEST_PACKAGE_INPUTS to a built independent Rust archive ledger"]
fn rust_package_syntax_coverage(cx: &mut TestAppContext) {
    let root = crate::test_packages::root("rust").canonicalize().unwrap();
    register_plugin(&root).unwrap();
    let manifest =
        PluginManifest::parse(&fs::read_to_string(root.join("plugin.toml")).unwrap()).unwrap();
    let (grammar, query) = load_plugin_language(&root, &manifest.highlighters[0]).unwrap();
    let (_parser, language) = readonly_parser(grammar).unwrap();
    let query = tree_sitter::Query::new(&language, &query).unwrap();
    let cases: &[(&str, &[(&str, &str)])] = &[
        (
            "use crate::module::Item; use super::Other; pub(crate) fn test() {}",
            &[
                ("crate", "keyword"),
                ("super", "keyword"),
                ("test", "function"),
            ],
        ),
        (
            "fn main() { std::mem::drop(1); let _ = Vec::<i32>::new(); let _ = list.iter().collect::<Vec<_>>(); }",
            &[
                ("drop", "function"),
                ("new", "function"),
                ("collect", "function.method"),
                ("i32", "type.builtin"),
            ],
        ),
        (
            "macro_rules! sample { () => { () }; } fn main() { std::println!(\"hello\"); }",
            &[
                ("macro_rules!", "keyword"),
                ("sample", "function.macro"),
                ("println", "function.macro"),
                ("\"hello\"", "string"),
            ],
        ),
        (
            "fn borrow<'a>(value: &'a str) -> &'a str { value }",
            &[
                ("'a", "label"),
                ("value", "variable.parameter"),
                ("str", "type.builtin"),
            ],
        ),
        (
            "const LIMIT: usize = 8; static STATE: bool = true; fn main() { 'again: loop { break 'again; } }",
            &[
                ("LIMIT", "constant"),
                ("STATE", "constant"),
                ("'again", "label"),
                ("8", "number"),
                ("true", "boolean"),
            ],
        ),
        (
            "fn calc(mut x: i32, y: i32) -> bool { x += y; x <<= 1; let _ = 0..=10; x >= y && x != 0 || x == 3 }",
            &[
                ("mut", "keyword"),
                ("->", "operator"),
                ("+=", "operator"),
                ("<<=", "operator"),
                ("..=", "operator"),
                (">=", "operator"),
                ("&&", "operator"),
                ("!=", "operator"),
                ("||", "operator"),
                ("==", "operator"),
            ],
        ),
        (
            "#[doc = \"中文😀\"] fn main() { let _ = r#\"/* raw */ ! /\"#; let _ = b\"byte\\n\"; let _ = c\"text\"; let _ = '\\n'; let _ = 1.25; }",
            &[
                ("\"中文😀\"", "string"),
                ("r#\"/* raw */ ! /\"#", "string"),
                ("b\"byte\\n\"", "string"),
                ("c\"text\"", "string"),
                ("'\\n'", "string"),
                ("1.25", "number"),
            ],
        ),
    ];
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::init(cx);
        editor_ui::typography::init(cx);
        let mut failures = Vec::new();
        for dark in [false, true] {
            apply_theme(builtin_theme(dark), cx);
            let theme = cx.theme().highlight_theme.as_ref();
            for capture in query.capture_names() {
                if theme.style(capture).is_none() {
                    failures.push(format!(
                        "dark={dark}, declared capture {capture:?} has no visible theme style"
                    ));
                }
            }
            for (source, expectations) in cases {
                let mut highlighter = crate::language::highlighter::SyntaxHighlighter::new("rust");
                highlighter.update(None, &Rope::from(*source), None);
                let tree = highlighter.tree().unwrap();
                assert!(
                    !tree.root_node().has_error(),
                    "{}: {}",
                    source,
                    tree.root_node().to_sexp()
                );
                let styles = highlighter.styles(&(0..source.len()), theme);
                for (needle, capture) in *expectations {
                    let start = source.find(needle).unwrap();
                    let expected = theme.style(capture).unwrap().color;
                    let wrong = (start..start + needle.len()).any(|offset| {
                        styles
                            .iter()
                            .find(|(range, _)| range.contains(&offset))
                            .and_then(|(_, style)| style.color)
                            != expected
                    });
                    if wrong {
                        failures.push(format!("dark={dark}, {needle:?} should be {capture}"));
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    });
}

/// Read-only code projections must keep the same doc markers and exact UTF-8 source coordinates.
#[test]
#[ignore = "set NANOBUG_TEST_PACKAGE_INPUTS to a built independent Rust archive ledger"]
fn rust_package_readonly_highlights() {
    use crate::language::{code_highlighting, providers::Providers};
    use std::{
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };
    let root = crate::test_packages::root("rust").canonicalize().unwrap();
    let manifest =
        PluginManifest::parse(&fs::read_to_string(root.join("plugin.toml")).unwrap()).unwrap();
    let store = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let providers = Providers::new(store.path(), workspace.path());
    providers.refresh(
        store.path(),
        vec![(
            "rust".into(),
            root,
            manifest.language_definitions,
            manifest.highlighters,
        )],
        vec![],
    );
    let selection = code_highlighting::selected_in(&providers, "rust").unwrap();
    let source = "//! 中文😀\r\n/// Documentation\r\n/** Block */\r\nmod network {}\r\nfn main() { std::mem::drop(1); let _ = 1 >= 0; }\r\n";
    let tokens = code_highlighting::highlight(
        &selection,
        source,
        &AtomicBool::new(false),
        Instant::now() + Duration::from_secs(30),
    );
    for (offset, character) in source[..source.find("mod network").unwrap()].char_indices() {
        if matches!(character, '\r' | '\n') {
            continue;
        }
        assert!(
            tokens
                .iter()
                .any(|token| token.range.contains(&offset) && token.capture.starts_with("comment")),
            "missing readonly comment color at byte {offset}"
        );
    }
    for (needle, expected) in [
        ("network", "namespace"),
        ("drop", "function"),
        (">=", "operator"),
    ] {
        let start = source.find(needle).unwrap();
        assert!(
            tokens
                .iter()
                .any(|token| token.range == (start..start + needle.len())
                    && token.capture == expected),
            "missing readonly {expected} capture for {needle}"
        );
    }
}
