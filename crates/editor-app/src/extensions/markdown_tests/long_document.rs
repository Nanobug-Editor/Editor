//! A repository-sized instruction document exercises real Markdown loading, scrolling and redraw.
use super::*;
use harness::NativeMarkdown;

/// The actual package must keep a long instruction file interactive after its initial publication.
#[gpui::test]
#[ignore = "opt-in real Markdown Package/WASM; NANOBUG_MARKDOWN_SDK_PACKAGE selects a digest-checked candidate"]
fn delivered_markdown_long_agents_document_keeps_frames_and_scroll(cx: &mut TestAppContext) {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../AGENTS.md"),
    )
    .unwrap();
    let (mut fixture, ui) = NativeMarkdown::mount(cx, &[("AGENTS.md", &source)]);
    let opened = Instant::now();
    fixture.open("AGENTS.md", ui);
    let open_time = opened.elapsed();
    eprintln!(
        "long Markdown: {} bytes, native open {:?}",
        source.len(),
        open_time
    );
    let workspace = fixture.directory.path().canonicalize().unwrap();
    for record in fixture
        .manager
        .runtime_logs()
        .records_since(0, &workspace.to_string_lossy())
        .1
    {
        if record.source == "wasm/compile" {
            eprintln!("long Markdown compilation: {}", record.message);
        }
    }
    assert!(
        open_time < Duration::from_secs(15),
        "cold plugin compilation delayed native document opening: {open_time:?}"
    );
    let mut frames = Vec::new();
    for frame in 0..10 {
        let elapsed = ui.update(|window, cx| {
            fixture.app.update(cx, |_, cx| cx.notify());
            let start = Instant::now();
            window.draw(cx).clear(cx);
            start.elapsed()
        });
        if frame >= 3 {
            frames.push(elapsed);
        }
    }
    frames.sort();
    eprintln!(
        "long Markdown native redraw: median={:?}, frames={frames:?}",
        frames[3]
    );
    assert!(
        frames[3] < Duration::from_millis(40),
        "long Markdown stalls whole-window redraw: {frames:?}"
    );
    let position = ui.update(|_, cx| fixture.app.read(cx).editor.read(cx).input_bounds().center());
    ui.simulate_event(gpui_kit::ScrollEvent {
        source: gpui_kit::ScrollSource::Pointer,
        position,
        delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-1000.))),
        ..Default::default()
    });
    ui.run_until_parked();
    fixture.settle(ui);
    super::synchronized_scroll::assert_source_matches_preview(&fixture, ui);
    let position = ui
        .debug_bounds("plugin-ui-preview-scroll")
        .unwrap()
        .center();
    ui.simulate_event(gpui_kit::ScrollEvent {
        source: gpui_kit::ScrollSource::Pointer,
        position,
        delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-600.))),
        ..Default::default()
    });
    ui.run_until_parked();
    fixture.settle(ui);
    super::synchronized_scroll::assert_source_matches_preview(&fixture, ui);
    // Queue consecutive native wheels across reflow before delivering the guest's receipts.
    // The last manually scrolled pane must remain the driver after all publications settle.
    for (cycle, source_side) in [true, false, true, false].into_iter().enumerate() {
        for step in 0..6 {
            let position = if source_side {
                ui.update(|_, cx| fixture.app.read(cx).editor.read(cx).input_bounds().center())
            } else {
                ui.debug_bounds("plugin-ui-preview-scroll")
                    .unwrap()
                    .center()
            };
            ui.simulate_event(gpui_kit::ScrollEvent {
                source: gpui_kit::ScrollSource::Pointer,
                position,
                delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-40.))),
                ..Default::default()
            });
            if step == 2 {
                ui.simulate_resize(size(px(1380. + cycle as f32 * 10.), px(900.)));
            }
            ui.run_until_parked();
        }
        fixture.settle(ui);
        super::synchronized_scroll::assert_source_matches_preview(&fixture, ui);
    }
}
