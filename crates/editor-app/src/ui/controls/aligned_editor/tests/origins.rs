//! Native padding and wrapped pane headers must share one actual screen-space glyph origin.
use super::*;

/// Read native source rectangles so unequal heights/soft wraps cannot pass by comparing row
/// numbers alone. Projection must agree across panes without changing either native document.
fn assert_common_rows(view: &Entity<Fixture>, cx: &App) {
    let fixture = view.read(cx);
    assert_eq!(fixture.group.status(cx), AlignmentStatus::Ready);
    let group = fixture.group.state.read(cx);
    let first = group.frames[0].as_ref().unwrap();
    let second = group.frames[1].as_ref().unwrap();
    assert!((first.viewport.top() - second.viewport.top()).abs() < px(0.5));
    assert!((first.viewport.size.height - second.viewport.size.height).abs() < px(0.5));
    for row in [0, 6, 22, 31] {
        let mut actual = Vec::new();
        for index in 0..2 {
            let text = content(index == 1);
            assert_eq!(fixture.editors[index].read(cx).value().as_ref(), text);
            let byte = byte_at(&text, row);
            let native = fixture.editors[index]
                .read(cx)
                .range_to_bounds(&(byte..byte))
                .unwrap();
            actual.push(group.project(index, native).unwrap().top());
        }
        assert!(
            (actual[0] - actual[1]).abs() < px(0.5),
            "shared source row {row}: {actual:?}"
        );
    }
}

#[gpui::test]
fn aligned_editor_common_glyph_origin_preserves_padding_headers_ime_and_last_row(
    cx: &mut TestAppContext,
) {
    let (view, ui) = mount(cx);
    ui.update(|_, cx| {
        let editor = view.read(cx).editors[1].clone();
        editor.update(cx, |state, cx| {
            state.set_editor_paddings(gpui_kit::Edges {
                top: px(8.),
                ..Default::default()
            });
            cx.notify();
        });
    });
    settle(ui);
    ui.update(|_, cx| {
        assert_common_rows(&view, cx);
        let group = view.read(cx).group.state.read(cx);
        let native_insets = group
            .measurements
            .iter()
            .map(|m| m.as_ref().unwrap().native_inset_y)
            .collect::<Vec<_>>();
        assert!((native_insets[1] - native_insets[0] - px(8.)).abs() < px(0.5));
    });
    // Moving the taller header between panes also proves the common origin can decrease again;
    // it is derived from this native frame, never accumulated as padding in EditorState.
    for headers in [[px(0.), px(42.)], [px(34.), px(0.)]] {
        view.update(ui, |fixture, cx| {
            fixture.headers = headers;
            cx.notify();
        });
        settle(ui);
        ui.update(|_, cx| assert_common_rows(&view, cx));
    }
    let target = byte_at(&content(false), 0) + 8;
    let click = ui.update(|_, cx| {
        let bounds = view
            .read(cx)
            .group
            .project_range(0, target..target, cx)
            .unwrap();
        point(bounds.left() + px(0.4), bounds.center().y)
    });
    ui.simulate_click(click, Default::default());
    ui.run_until_parked();
    settle(ui);
    ui.update(|window, cx| {
        assert_eq!(view.read(cx).editors[0].read(cx).cursor(), target);
        painted_caret(&view, window, cx);
        let (editor, frame, factory) = {
            let fixture = view.read(cx);
            let group = fixture.group.state.read(cx);
            (
                fixture.editors[0].clone(),
                group.frames[0].clone().unwrap(),
                group.sources[0].input_factory.clone().unwrap(),
            )
        };
        let delegate = factory(editor.read(cx).text_bounds().unwrap(), window, cx);
        let mut handler = input::AlignedInputHandler {
            delegate,
            editor,
            projection: Some(frame),
            suppress_geometry: false,
        };
        handler.replace_and_mark_text_in_range(None, "拼😀", Some(1..3), window, cx);
    });
    settle(ui);
    ui.update(|window, cx| {
        let (editor, frame, factory) = {
            let fixture = view.read(cx);
            let group = fixture.group.state.read(cx);
            (
                fixture.editors[0].clone(),
                group.frames[0].clone().unwrap(),
                group.sources[0].input_factory.clone().unwrap(),
            )
        };
        let delegate = factory(editor.read(cx).text_bounds().unwrap(), window, cx);
        let viewport = frame.viewport;
        let mut handler = input::AlignedInputHandler {
            delegate,
            editor,
            projection: Some(frame),
            suppress_geometry: false,
        };
        let marked = handler.marked_text_range(window, cx).unwrap();
        let ime = handler.bounds_for_range(marked, window, cx).unwrap();
        assert!(
            viewport.contains(&ime.center()),
            "IME bounds must follow the painted common-origin source: {ime:?} in {viewport:?}"
        );
        assert_eq!(handler.element_bounds(window, cx), Some(viewport));
        painted_caret(&view, window, cx);
        handler.replace_text_in_range(None, "中文😀", window, cx);
    });
    settle(ui);
    ui.simulate_keystrokes("ctrl-z");
    settle(ui);
    ui.update(|window, cx| {
        assert_common_rows(&view, cx);
        let editor = view.read(cx).editors[0].clone();
        editor.update(cx, |state, cx| {
            let eof = state.text().len();
            state.set_selected_range(eof..eof, cx);
            state.focus(window, cx);
        });
    });
    settle(ui);
    ui.update(|_, cx| {
        let group = view.read(cx).group.clone();
        assert!(group.reveal_offset(0, content(false).len(), cx));
    });
    settle(ui);
    ui.update(|window, cx| {
        let fixture = view.read(cx);
        let group = fixture.group.state.read(cx);
        let frame = group.frames[0].as_ref().unwrap();
        let eof = content(false).len();
        let visible = fixture.group.project_range(0, eof..eof, cx).unwrap();
        assert!(
            frame.viewport.contains(&visible.center()),
            "the final real row must remain visible after removing native padding"
        );
        assert_eq!(group.scroll.get(), group.max_scroll());
        painted_caret(&view, window, cx);
    });
    for _ in 0..3 {
        ui.update(|window, cx| {
            window.draw(cx).clear(cx);
            assert_eq!(
                window.simulate_next_frame(cx),
                0,
                "common-origin measurement must settle"
            );
        });
        ui.run_until_parked();
    }
}
