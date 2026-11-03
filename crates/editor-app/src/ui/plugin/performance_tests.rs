//! Long source-mapped trees exercise the native frame path shared by every plugin.
use super::*;
use gpui_base::Root;
use gpui_kit::{TestAppContext, gpui, px, size};
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

#[cfg(windows)]
mod native;

/// A valid near-quota readonly tree represents any plugin using mapped native rich text.
fn long_document() -> Document {
    // Each block consumes one node plus four HTML tags from the shared 2048-item quota.
    let blocks = (0..400).map(|index| {
        let mut node = ui::Node::rich_text(
            format!("paragraph-{index}"),
            format!("<p>Section {index}: native <strong>plugin content</strong> and 中文正文 repeat across a long document.</p>"),
        );
        node.source_range = Some(ui::SourceRange { start: index * 128, end: (index + 1) * 128 });
        node
    }).collect();
    let mut document = Document::new(
        ui::Node::scroll("scroll", ui::Node::column("body", blocks).padding(24.)).grow(),
    )
    .revision(1);
    document.source = Some(plugin_runtime::plugin_protocol::api::DocumentVersion {
        id: "long-document".into(),
        path: "notes.md".into(),
        revision: 1,
    });
    document.validate().unwrap();
    document
}

/// Repainting an unchanged long document must fit an interactive frame budget after initial layout.
/// Force real native draws, as window movement and surrounding host chrome can request them.
#[gpui::test]
fn long_plugin_document_repaint_stays_interactive(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::typography::init(cx);
        crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(false), cx);
    });
    let document = long_document();
    let capture = Rc::new(RefCell::new(None));
    let slot = capture.clone();
    let (_, ui) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            PluginView::new(
                "ordinary-provider".into(),
                document,
                Environment::default(),
                |_, _| {},
                window,
                cx,
            )
        });
        *slot.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = capture.borrow_mut().take().unwrap();
    ui.simulate_resize(size(px(1000.), px(800.)));
    let mut samples = Vec::new();
    for frame in 0..10 {
        let elapsed = ui.update(|window, cx| {
            view.update(cx, |_, cx| cx.notify());
            let start = Instant::now();
            window.draw(cx).clear(cx);
            start.elapsed()
        });
        if frame >= 3 {
            samples.push(elapsed);
        }
    }
    samples.sort();
    eprintln!(
        "long plugin native redraw: median={:?}, samples={samples:?}",
        samples[3]
    );
    assert!(
        samples[3] < Duration::from_millis(40),
        "unchanged plugin content stalls native window redraw: {samples:?}"
    );
}
