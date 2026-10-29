//! First-frame native text and delayed real WASM colors/folds share the production adapter.
use super::*;
use crate::language::{highlighter::LanguageRegistry, providers::Providers};
use gpui_kit::{TestAppContext, VisualTestContext, gpui};
use plugin_schema::PluginManifest;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

const SOURCE: &str = "<root>\n  <!-- 中文🙂\n       visible first\n       comment -->\n  <item id=\"value\">\n    <leaf/>\n  </item>\n</root>";
const LATEST: &str = "<latest id=\"最新🙂\"/>";

/// A project-qualified fixed package avoids global language selection and native grammar fallback.
struct Fixture {
    _store: tempfile::TempDir,
    _workspace: tempfile::TempDir,
    providers: Providers,
    calls: Arc<AtomicUsize>,
    retire_on_create: Arc<AtomicBool>,
}

impl Fixture {
    /// Observe the existing public parser factory while still constructing a real isolated WASM parser.
    fn new() -> Self {
        let store = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let providers = Providers::new(store.path(), workspace.path());
        let root = crate::test_packages::root("xml").canonicalize().unwrap();
        let declaration =
            PluginManifest::parse(&std::fs::read_to_string(root.join("plugin.toml")).unwrap())
                .unwrap();
        providers.refresh(
            store.path(),
            vec![(
                declaration.plugin.id,
                root.clone(),
                declaration.language_definitions,
                declaration.highlighters,
            )],
            Vec::new(),
        );
        let provider = providers
            .grammars()
            .into_iter()
            .find(|provider| provider.declaration.language == "xml")
            .unwrap();
        let (grammar, query) = crate::language::plugins::prepare_dynamic(&provider).unwrap();
        crate::language::plugins::publish_project_dynamic(
            &providers,
            "xml",
            grammar.clone(),
            query,
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let observed_calls = calls.clone();
        let retire_on_create = Arc::new(AtomicBool::new(false));
        let retire = retire_on_create.clone();
        let owner = providers.clone();
        LanguageRegistry::singleton().register_parser_factory(
            &providers.parser_key("xml"),
            Arc::new(move || {
                observed_calls.fetch_add(1, Ordering::SeqCst);
                let parser = crate::language::plugins::readonly_parser(grammar.clone())?;
                // Revoke after real initialization so the delayed result genuinely carries the
                // old provider. No sleeps or assumptions about test-executor OS threads are needed.
                if retire.swap(false, Ordering::SeqCst) {
                    owner.retire();
                }
                Ok(parser)
            }),
        );
        Self {
            _store: store,
            _workspace: workspace,
            providers,
            calls,
            retire_on_create,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.providers.retire();
    }
}

/// Read-only observation of the exact syntax object used by the native adapter, never another parser.
type ObservedSyntax = Rc<RefCell<Option<Rc<RefCell<SyntaxHighlighter>>>>>;

/// Mount the normal native source renderer before scheduling grammar work under test.
struct View {
    editor: Entity<EditorState>,
}

impl Render for View {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("initial-grammar-native-source")
            .debug_selector(|| "initial-grammar-native-source".into())
            .size_full()
            .text_size(px(14.))
            .child(editor_ui::native_editor_scrollbars(&self.editor))
    }
}

/// The window begins with plain text, so add_window_view's initial executor drain cannot parse XML.
fn open_view(cx: &mut TestAppContext) -> (Entity<View>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let (view, visual) = cx.add_window_view(|window, cx| View {
        editor: cx.new(|cx| {
            EditorState::new(window, cx)
                .language("text")
                .line_number(true)
                .folding(true)
                .default_value(SOURCE)
        }),
    });
    visual.simulate_resize(size(px(480.), px(240.)));
    (view, visual)
}

/// Use Base's ordinary factory seam and the production Adapter; only its shared reader is observed.
fn attach(editor: &Entity<EditorState>, fixture: &Fixture, cx: &mut App) -> ObservedSyntax {
    let observed = Rc::new(RefCell::new(None));
    let captured = observed.clone();
    let providers = fixture.providers.clone();
    editor.update(cx, |editor, cx| {
        editor.set_highlighter_factory(
            Rc::new(move |language| {
                let syntax = Rc::new(RefCell::new(plugin_syntax_in(&providers, language)));
                *captured.borrow_mut() = Some(syntax.clone());
                Some(Box::new(Adapter {
                    language: language.to_owned().into(),
                    providers: providers.clone(),
                    syntax,
                    parser_cancelled: Rc::new(Cell::new(false)),
                    folds: Rc::new(RefCell::new(None)),
                    markers: Rc::new(RefCell::new(Default::default())),
                    generation: Rc::new(Cell::new(0)),
                    pending: None,
                }))
            }),
            cx,
        );
        editor.set_highlighter("xml", cx);
    });
    observed
}

/// Borrow the live adapter reader only after native drawing instantiated the selected highlighter.
fn syntax(observed: &ObservedSyntax) -> Rc<RefCell<SyntaxHighlighter>> {
    observed
        .borrow()
        .as_ref()
        .expect("native XML adapter")
        .clone()
}

/// Check actual query colors through the same theme resolver as the native source renderer.
fn assert_colored(observed: &ObservedSyntax, source: &str, cx: &App) {
    let syntax = syntax(observed);
    let syntax = syntax.borrow();
    let tree = syntax.tree().expect("background WASM parse must finish");
    assert_eq!(tree.root_node().end_byte(), source.len());
    let offset = source.find("id=").unwrap() + 4;
    let styles = syntax.styles(&(0..source.len()), cx.theme().highlight_theme.as_ref());
    assert!(
        styles
            .iter()
            .any(|(range, style)| range.contains(&offset) && style.color.is_some()),
        "the live native adapter must color the XML attribute value: {styles:?}"
    );
}

/// First draw must expose text/caret geometry without constructing a parser; idle completion colors it.
#[gpui::test]
fn native_initial_grammar_paints_text_before_parser_then_colors_and_folds(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    let (view, visual) = open_view(cx);
    let (editor, observed) = visual.update(|window, cx| {
        let editor = view.read(cx).editor.clone();
        let observed = attach(&editor, &fixture, cx);
        // Force this frame inside the same UI turn, before the test executor may run queued work.
        window.refresh();
        window.draw(cx).clear(cx);
        assert_eq!(
            fixture.calls.load(Ordering::SeqCst),
            0,
            "first native frame created a parser"
        );
        assert_eq!(editor.read(cx).text().to_string(), SOURCE);
        assert!(editor.read(cx).range_to_bounds(&(0..0)).is_some());
        assert!(syntax(&observed).borrow().tree().is_none());
        assert!(!syntax(&observed).borrow().is_prepared());
        (editor, observed)
    });
    assert!(
        visual
            .debug_bounds("initial-grammar-native-source")
            .is_some()
    );
    // A cold open has no 150 ms edit debounce: draining scheduled tasks is enough.
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
        assert_colored(&observed, SOURCE, cx);
        let folds = grammar_folds(&syntax(&observed).borrow());
        assert!(!folds.is_empty());
        for fold in folds {
            assert!(
                editor
                    .read(cx)
                    .display_map()
                    .is_fold_candidate(fold.start_line)
            );
        }
        assert_eq!(editor.read(cx).text().to_string(), SOURCE);
    });
}

/// Consecutive document replacements before the scheduled parse cannot restore old colors or folds.
#[gpui::test]
fn native_initial_grammar_fast_replacement_keeps_latest_text(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    let (view, visual) = open_view(cx);
    let (editor, observed) = visual.update(|window, cx| {
        let editor = view.read(cx).editor.clone();
        let observed = attach(&editor, &fixture, cx);
        window.refresh();
        window.draw(cx).clear(cx);
        for source in ["<intermediate>\n  <old/>\n</intermediate>", LATEST] {
            editor.update(cx, |editor, cx| editor.set_value(source, window, cx));
            window.refresh();
            window.draw(cx).clear(cx);
        }
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
        assert_eq!(editor.read(cx).text().to_string(), LATEST);
        (editor, observed)
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
        assert_colored(&observed, LATEST, cx);
        assert!(grammar_folds(&syntax(&observed).borrow()).is_empty());
        assert!(!editor.read(cx).display_map().is_fold_candidate(0));
        assert_eq!(editor.read(cx).text().to_string(), LATEST);
        assert!(editor.read(cx).range_to_bounds(&(0..0)).is_some());
    });
}

/// A real initialized parser cannot publish after its project provider is retired during preparation.
#[gpui::test]
fn native_initial_grammar_provider_retirement_rejects_delayed_parse(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    fixture.retire_on_create.store(true, Ordering::SeqCst);
    let epoch = fixture.providers.feature_generation();
    let (view, visual) = open_view(cx);
    let (editor, observed) = visual.update(|window, cx| {
        let editor = view.read(cx).editor.clone();
        let observed = attach(&editor, &fixture, cx);
        window.refresh();
        window.draw(cx).clear(cx);
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
        (editor, observed)
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
        assert_ne!(fixture.providers.feature_generation(), epoch);
        let syntax = syntax(&observed);
        let syntax = syntax.borrow();
        assert!(
            syntax.tree().is_none(),
            "a retired grammar published its old tree"
        );
        assert!(
            syntax
                .styles(&(0..SOURCE.len()), cx.theme().highlight_theme.as_ref())
                .is_empty()
        );
        assert!(!editor.read(cx).display_map().is_fold_candidate(0));
        assert_eq!(editor.read(cx).text().to_string(), SOURCE);
        assert!(editor.read(cx).range_to_bounds(&(0..0)).is_some());
    });
}
