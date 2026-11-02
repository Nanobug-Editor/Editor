//! Real Windows paints exercise the production adapter while grammar preparation is held pending.
use crate::language::highlighter::{GrammarConfig, LanguageRegistry};
use crate::language::providers::Providers;
use crate::*;
use gpui_kit::{AnyWindowHandle, AsyncApp, WindowBounds};
use std::{
    cell::RefCell,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

/// The local source component paints the same editor/adapter used by newly loaded text tabs.
struct LoadingView {
    /// The sole native text, layout, selection and Undo owner.
    editor: Entity<EditorState>,
    /// Retain the ordinary grammar/folding adapter throughout preparation and publication.
    _controller: crate::outline::folding::Controller,
}

impl Render for LoadingView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .text_size(px(14.))
            .child(editor_ui::source_editor::Editor::new(&self.editor).tools(false))
    }
}

/// Releasing on scope exit also unblocks preparation when an image assertion fails.
struct ParseGate(Arc<AtomicBool>);

impl Drop for ParseGate {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

/// A deliberately unavailable grammar exposes cold preparation and ordinary fallback deterministically.
/// No grammar or service is installed globally; `root` owns only this test's provider preferences.
fn pending_provider(root: &Path, gate: &ParseGate, entered: Arc<AtomicUsize>) -> Providers {
    let providers = Providers::new(root, root);
    providers.refresh(
        root,
        vec![(
            "pending-fixture".into(),
            root.into(),
            Vec::new(),
            vec![plugin_schema::Highlighter {
                id: "syntax".into(),
                language: "pending-fixture".into(),
                grammar_name: "fixture".into(),
                grammar: "grammar.wasm".into(),
                highlights: "highlights.scm".into(),
                injections: None,
                injection_languages: Vec::new(),
                tree_sitter_abi: 15,
            }],
        )],
        Vec::new(),
    );
    let key = providers.parser_key("pending-fixture");
    let mut config = GrammarConfig::plain(key.clone());
    config.highlights = "(identifier) @variable".into();
    LanguageRegistry::singleton().register(&key, &config);
    let release = gate.0.clone();
    LanguageRegistry::singleton().register_parser_factory(
        &key,
        Arc::new(move || {
            entered.fetch_add(1, Ordering::Release);
            let deadline = Instant::now() + Duration::from_secs(5);
            while !release.load(Ordering::Acquire) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(1));
            }
            anyhow::bail!("fixture grammar unavailable after preparation")
        }),
    );
    providers
}

/// Count actual body ink, excluding the gutter/caret, and optionally save the full rendered frame.
fn body_ink(window: &mut Window, name: &str) -> anyhow::Result<usize> {
    let image = window.render_to_image()?;
    let scale = window.scale_factor();
    let background = image
        .get_pixel((570. * scale) as u32, (260. * scale) as u32)
        .0;
    let mut ink = 0;
    for y in (75. * scale) as u32..(250. * scale) as u32 {
        for x in (150. * scale) as u32..(540. * scale) as u32 {
            let pixel = image.get_pixel(x, y).0;
            if (0..3).any(|channel| pixel[channel].abs_diff(background[channel]) > 40) {
                ink += 1;
            }
        }
    }
    if let Some(output) = std::env::var_os("NANOBUG_LOAD_CAPTURE") {
        image.save(Path::new(&output).join(format!("{name}.png")))?;
    }
    Ok(ink)
}

/// Observe plain, pending and settled frames with both native palettes, preserving the same buffer.
async fn exercise(cx: &mut AsyncApp) -> anyhow::Result<()> {
    for dark in [false, true] {
        let directory = tempfile::tempdir()?;
        let gate = ParseGate(Arc::new(AtomicBool::new(false)));
        let entered = Arc::new(AtomicUsize::new(0));
        let providers = pending_provider(directory.path(), &gate, entered.clone());
        let source = (0..30)
            .map(|row| format!("const message = '中文😀'; // loaded row {row}\n"))
            .collect::<String>();
        let original = source.clone();
        let (handle, view): (AnyWindowHandle, Entity<LoadingView>) = cx.update(|cx| {
            apply_theme(builtin_theme(dark), cx);
            let options = gpui_kit::WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(100.), px(100.)),
                    size(px(600.), px(400.)),
                ))),
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                window.set_window_title("Nanobug — file loading paint regression");
                let editor = cx.new(|cx| {
                    EditorState::new(window, cx)
                        .language("text")
                        .line_number(true)
                });
                editor.update(cx, |editor, cx| editor.set_value(source, window, cx));
                let controller =
                    crate::outline::folding::Controller::attach(&editor, providers, cx);
                cx.new(|_| LoadingView {
                    editor,
                    _controller: controller,
                })
            })
        })?;
        cx.background_executor()
            .timer(Duration::from_millis(120))
            .await;
        let baseline = handle.update(cx, |_, window, _| {
            body_ink(window, &format!("{dark}-plain"))
        })??;
        anyhow::ensure!(
            baseline > 100,
            "native baseline must contain body glyphs: {baseline}"
        );
        handle.update(cx, |_, _, cx| {
            view.read(cx).editor.clone().update(cx, |editor, cx| {
                editor.set_highlighter("pending-fixture", cx)
            });
        })?;
        let mut pending = Vec::new();
        for frame in 0..8 {
            cx.background_executor()
                .timer(Duration::from_millis(20))
                .await;
            pending.push(handle.update(cx, |_, window, _| {
                body_ink(window, &format!("{dark}-pending-{frame}"))
            })??);
        }
        anyhow::ensure!(
            entered.load(Ordering::Acquire) > 0,
            "exercise the real cold grammar worker"
        );
        gate.0.store(true, Ordering::Release);
        cx.background_executor()
            .timer(Duration::from_millis(150))
            .await;
        let settled = handle.update(cx, |_, window, cx| {
            assert_eq!(view.read(cx).editor.read(cx).text().to_string(), original);
            body_ink(window, &format!("{dark}-settled"))
        })??;
        handle.update(cx, |_, window, _| window.remove_window())?;
        eprintln!(
            "native loading dark={dark}: baseline={baseline}, pending={pending:?}, settled={settled}"
        );
        anyhow::ensure!(
            pending.iter().all(|ink| *ink >= baseline * 9 / 10),
            "loaded body disappeared during grammar preparation: {pending:?}"
        );
        anyhow::ensure!(
            settled >= baseline * 9 / 10,
            "grammar fallback must retain the complete body"
        );
    }
    Ok(())
}

/// Run separately from TestPlatform tests so the real D3D and DirectWrite state owns the UI loop.
#[test]
#[ignore = "opens isolated native Windows/D3D windows; run explicitly with --test-threads=1"]
fn deferred_highlighter_native_loading_keeps_body_visible() {
    let result = Rc::new(RefCell::new(None));
    let completed = result.clone();
    gpui_kit::application()
        .with_assets(AppAssets)
        .with_quit_mode(gpui_kit::QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            typography::init(cx);
            editor_ui::init(cx);
            cx.set_reduce_motion(true);
            cx.spawn(async move |cx| {
                *completed.borrow_mut() =
                    Some(exercise(cx).await.map_err(|error| format!("{error:#}")));
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
    result
        .borrow_mut()
        .take()
        .expect("native loading test completed")
        .unwrap();
}
