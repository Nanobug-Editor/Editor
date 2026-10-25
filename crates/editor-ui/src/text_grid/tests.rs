//! Native input ownership and UTF-16 regressions independent from any terminal or plugin view.

use super::*;
use gpui_base::Root;
use gpui_kit::{AppContext as _, TestAppContext, gpui};
use std::cell::RefCell;

/// UTF-16 ranges round outward around emoji and never split native string storage.
#[test]
fn composition_ranges_preserve_scalars() {
    assert_eq!(input::composition_range("你😀a", 1..2), (3..7, 1..3));
    assert_eq!(input::composition_range("你😀a", 99..100), (8..8, 4..4));
    assert_eq!(input::composition_range("", 0..0), (0..0, 0..0));
}

/// A native grid owns preedit/focus and emits committed text only once, without a document copy.
#[gpui::test]
fn native_grid_ime_commit_cancel_and_pointer_modifiers(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let grid = cx.new(|cx| {
            TextGrid::new(
                GridDrawing::default(),
                move |event, _| sink.borrow_mut().push(event),
                window,
                cx,
            )
        });
        *capture.borrow_mut() = Some(grid.clone());
        Root::new(grid, window, cx)
    });
    let grid = slot.borrow_mut().take().unwrap();
    visual.simulate_resize(size(px(500.), px(300.)));
    visual.run_until_parked();
    visual.update(|window, cx| {
        grid.update(cx, |grid, cx| {
            grid.focus.focus(window, cx);
            grid.replace_and_mark_text_in_range(None, "你😀", Some(3..3), window, cx);
            assert!(grid.is_composing());
            grid.replace_text_in_range(None, "你好😀", window, cx);
            assert!(!grid.is_composing());
            grid.replace_and_mark_text_in_range(None, "取消", None, window, cx);
            grid.unmark_text(window, cx);
            assert!(!grid.is_composing());
        });
        window.draw(cx).clear(cx);
    });
    visual.simulate_click(
        point(px(50.), px(60.)),
        Modifiers {
            control: true,
            alt: true,
            ..Default::default()
        },
    );
    visual.simulate_event(KeyDownEvent {
        keystroke: gpui_kit::Keystroke {
            key: "e".into(),
            key_char: Some("€".into()),
            modifiers: Modifiers {
                control: true,
                alt: true,
                ..Default::default()
            },
        },
        is_held: false,
        prefer_character_input: true,
    });
    visual.simulate_input("€");
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|event| matches!(event, GridEvent::Text { text } if text == "你好😀"))
            .count(),
        1
    );
    assert!(
        !events
            .borrow()
            .iter()
            .any(|event| matches!(event, GridEvent::Text { text } if text.contains("取消")))
    );
    assert!(events.borrow().iter().any(|event| matches!(event, GridEvent::Pointer { phase: GridPointerPhase::Down, modifiers, .. } if modifiers.control && modifiers.alt)));
    assert!(
        !events.borrow().iter().any(
            |event| matches!(event, GridEvent::Key { key, ctrl: true, alt: true, .. } if key == "e")
        ),
        "AltGr character input must not also emit an ESC/control sequence"
    );
}
