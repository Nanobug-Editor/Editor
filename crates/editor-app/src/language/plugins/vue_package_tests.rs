//! Actual independent packages exercise native SFC injection, selection and withdrawal.
//! These ignored tests require explicit archives; they never build or install dependencies.

use super::*;
use crate::language::{code_highlighting, providers::Providers};
use plugin_runtime::Package;
use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use tree_sitter::StreamingIterator as _;

/// Keep extracted immutable grammar resources alive for the entire provider projection.
struct GrammarPackage {
    directory: tempfile::TempDir,
    id: String,
    manifest: PluginManifest,
}

impl GrammarPackage {
    /// Package::read validates the archive; only declared grammar/query resources are extracted.
    fn read(path: &Path) -> Self {
        let package = Package::read(path).unwrap();
        let declaration = package.manifest.contributions.as_ref().unwrap();
        let manifest =
            PluginManifest::parse(std::str::from_utf8(&package.files[declaration]).unwrap())
                .unwrap();
        let directory = tempfile::tempdir().unwrap();
        for grammar in &manifest.highlighters {
            for relative in [&grammar.grammar, &grammar.highlights]
                .into_iter()
                .chain(grammar.injections.iter())
            {
                let key = relative.to_string_lossy().replace('\\', "/");
                let target = directory.path().join(relative);
                fs::create_dir_all(target.parent().unwrap()).unwrap();
                fs::write(target, &package.files[&key]).unwrap();
            }
        }
        Self {
            directory,
            id: package.manifest.id,
            manifest,
        }
    }

    /// Use exactly the declarations read from the package, never aliases invented by a fixture.
    fn entry(
        &self,
    ) -> (
        String,
        PathBuf,
        Vec<plugin_schema::LanguageDefinition>,
        Vec<Highlighter>,
    ) {
        (
            self.id.clone(),
            self.directory.path().canonicalize().unwrap(),
            self.manifest.language_definitions.clone(),
            self.manifest.highlighters.clone(),
        )
    }
}

fn vue_package() -> GrammarPackage {
    GrammarPackage::read(&PathBuf::from(
        std::env::var_os("NANOBUG_VUE_PACKAGE").expect("built Vue archive"),
    ))
}

/// Real native query matches must identify each block's unchanged source bytes and target language.
fn injection_targets(grammar: Arc<LoadedGrammar>, source: &str) -> Vec<(String, String)> {
    let (mut parser, language) = readonly_parser(grammar.clone()).unwrap();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(source, None).unwrap();
    assert!(
        !tree.root_node().has_error(),
        "{}",
        tree.root_node().to_sexp()
    );
    let query = tree_sitter::Query::new(&language, grammar.readonly_injections().0).unwrap();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
    let mut result = Vec::new();
    while let Some(found) = matches.next() {
        let target = query
            .property_settings(found.pattern_index)
            .iter()
            .find(|property| property.key.as_ref() == "injection.language")
            .and_then(|property| property.value.as_ref())
            .unwrap();
        for capture in found.captures {
            if query.capture_names()[capture.index as usize] == "injection.content" {
                result.push((
                    target.to_string(),
                    source[capture.node.byte_range()].to_owned(),
                ));
            }
        }
    }
    result
}

/// Missing block grammars stay plain; removing Vue invalidates already captured asynchronous work.
#[test]
#[ignore = "set NANOBUG_VUE_PACKAGE to a built independent Vue archive"]
fn vue_package_native_queries_missing_blocks_and_withdrawal() {
    let vue = vue_package();
    let store = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let providers = Providers::new(store.path(), workspace.path());
    let mut only_vue = vue.entry();
    // Explicit provider withdrawal models the same selected snapshot as disabling block providers.
    only_vue.3.retain(|grammar| grammar.language == "vue");
    providers.refresh(store.path(), vec![only_vue], Vec::new());
    let selection = code_highlighting::selected_in(&providers, "vue").unwrap();
    let (grammar, _) = prepare_dynamic(&selection.provider).unwrap();
    for (tag, lang, expected, body) in [
        ("script", "js", "javascript", "const item = 123;"),
        ("script", "ts", "typescript", "const item: number = 123;"),
        (
            "script",
            "jsx",
            "javascriptreact",
            "const item = <Child />;",
        ),
        (
            "script",
            "tsx",
            "typescriptreact",
            "const item: JSX.Element = <Child />;",
        ),
        ("style", "css", "css", ".item { color: red; }"),
        (
            "style",
            "scss",
            "scss",
            "$tone: red; .item { color: $tone; }",
        ),
        ("style", "sass", "sass", ".item\n  color: red\n"),
        (
            "style",
            "less",
            "less",
            "@tone: red; .item { color: @tone; }",
        ),
    ] {
        let source =
            format!("<template><Child /></template>\r\n<{tag} lang=\"{lang}\">{body}</{tag}>");
        assert_eq!(
            injection_targets(grammar.clone(), &source),
            vec![(expected.into(), body.into())]
        );
        assert!(code_highlighting::selected_in(&providers, expected).is_none());
        let tokens = code_highlighting::highlight(
            &selection,
            &source,
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(10),
        );
        assert!(
            tokens
                .iter()
                .any(|token| token.capture == "tag" && &source[token.range.clone()] == "Child")
        );
        let start = source.find(body).unwrap();
        assert!(
            !tokens
                .iter()
                .any(|token| token.range.start >= start && token.range.end <= start + body.len()),
            "missing {expected} must not borrow an upstream parser: {tokens:?}"
        );
    }
    for source in [
        "<script lang=\"coffee\">x = 1</script>",
        "<style lang=\"stylus\">x = 1</style>",
    ] {
        assert!(injection_targets(grammar.clone(), source).is_empty());
    }
    providers.refresh(store.path(), Vec::new(), Vec::new());
    assert!(code_highlighting::selected_in(&providers, "vue").is_none());
    assert!(
        code_highlighting::highlight(
            &selection,
            "<template><Child /></template>",
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(10)
        )
        .is_empty()
    );
}

/// Actual selected child parsers must color their distinctive syntax inside unchanged SFC ranges.
#[test]
#[ignore = "set NANOBUG_VUE_PACKAGE and NANOBUG_TYPESCRIPT_PACKAGE to built independent archives"]
fn vue_package_native_block_grammars_and_captures() {
    let vue = vue_package();
    let typescript = GrammarPackage::read(&PathBuf::from(
        std::env::var_os("NANOBUG_TYPESCRIPT_PACKAGE").expect("built TypeScript archive"),
    ));
    let store = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let providers = Providers::new(store.path(), workspace.path());
    providers.refresh(
        store.path(),
        vec![vue.entry(), typescript.entry()],
        Vec::new(),
    );
    for (tag, lang, target, body, token) in [
        ("script", "js", "javascript", "const item = 123;", "123"),
        (
            "script",
            "ts",
            "typescript",
            "const item: number = 123;",
            "number",
        ),
        (
            "script",
            "jsx",
            "javascriptreact",
            "const item = <Child />;",
            "Child",
        ),
        (
            "script",
            "tsx",
            "typescriptreact",
            "const item: string = <Child />;",
            "string",
        ),
        ("style", "css", "css", ".item { color: red; }", "color"),
        (
            "style",
            "scss",
            "scss",
            "$tone: red; .item { color: $tone; }",
            "$tone",
        ),
        ("style", "sass", "sass", ".item\n  color: red\n", "color"),
        (
            "style",
            "less",
            "less",
            "@tone: red; .item { color: @tone; }",
            "@tone",
        ),
    ] {
        let child = code_highlighting::selected_in(&providers, target)
            .unwrap_or_else(|| panic!("actual package lacks {target} grammar"));
        let (grammar, _) = prepare_dynamic(&child.provider).unwrap();
        let (mut parser, language) = readonly_parser(grammar).unwrap();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(body, None).unwrap();
        assert!(
            !tree.root_node().has_error(),
            "{target}: {}",
            tree.root_node().to_sexp()
        );
        let source =
            format!("<template><p>中文😀</p></template>\r\n<{tag} lang=\"{lang}\">{body}</{tag}>");
        let selection = code_highlighting::selected_in(&providers, "vue").unwrap();
        let captures = code_highlighting::highlight(
            &selection,
            &source,
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(10),
        );
        let begin = source.find(body).unwrap();
        assert!(
            captures.iter().any(|capture| capture.range.start >= begin
                && source[capture.range.clone()].contains(token)),
            "{target} injection lacks {token}: {captures:?}"
        );
        assert!(
            captures
                .iter()
                .all(|capture| source.is_char_boundary(capture.range.start)
                    && source.is_char_boundary(capture.range.end))
        );
    }
    // Adding an actual independently packaged competing JS provider must retain the earlier
    // selected provider, rather than allowing package/install order to silently replace it.
    let earlier = code_highlighting::selected_in(&providers, "javascript").unwrap();
    let alternate = GrammarPackage::read(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../plugin-runtime/tests/fixtures/packages/javascript-0.3.1.zip"),
    );
    providers.refresh(
        store.path(),
        vec![vue.entry(), typescript.entry(), alternate.entry()],
        Vec::new(),
    );
    assert_eq!(
        code_highlighting::selected_in(&providers, "javascript")
            .unwrap()
            .provider
            .owner,
        earlier.provider.owner
    );
    let retained = code_highlighting::selected_in(&providers, "vue").unwrap();
    providers.retire();
    assert!(
        code_highlighting::highlight(
            &retained,
            "<script>const x = 123;</script>",
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(10)
        )
        .is_empty()
    );
}
