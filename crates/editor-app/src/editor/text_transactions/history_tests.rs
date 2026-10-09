//! Native keys verify transaction units rather than implementation calls or a mirrored undo stack.

use super::*;
use gpui_kit::{TestAppContext, gpui};
use std::cell::RefCell;

/// A generated insertion must remain separate from typing on either side of its native Undo unit.
#[gpui::test]
fn proposal_undo_stays_atomic_between_native_typing(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        theme::apply_theme(theme::builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("proposal.txt");
    std::fs::write(&path, "中文🙂\r\n").unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| EditorApp::new(workspace, Some(path), window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    visual.simulate_resize(size(px(1100.), px(700.)));
    visual.run_until_parked();
    visual.update(|window, cx| {
        app.read(cx).editor.clone().update(cx, |editor, cx| {
            editor.set_selected_range(0..0, cx);
            editor.focus(window, cx);
        });
    });
    visual.simulate_input("a");
    visual.run_until_parked();
    visual.update(|window, cx| {
        let editor = app.read(cx).editor.clone();
        let source = editor.read(cx).text().to_string();
        let point = lsp_types::Position::new(0, 1);
        editor.update(cx, |editor, cx| {
            apply_language_proposal(
                editor,
                &source,
                &[TextEdit::new(lsp_types::Range::new(point, point), "生成".into())],
                "a生成".len(),
                window,
                cx,
            )
            .unwrap();
        });
    });
    visual.run_until_parked();
    visual.simulate_input("b");
    visual.run_until_parked();
    let text = |visual: &mut gpui_kit::VisualTestContext| {
        visual.update(|_, cx| app.read(cx).editor.read(cx).text().to_string())
    };
    assert_eq!(text(visual), "a生成b中文🙂\r\n");
    visual.simulate_keystrokes("ctrl-z");
    assert_eq!(text(visual), "a生成中文🙂\r\n", "one Undo removes subsequent typing");
    visual.simulate_keystrokes("ctrl-z");
    assert_eq!(text(visual), "a中文🙂\r\n", "one Undo removes the generated insertion");
    visual.simulate_keystrokes("ctrl-z");
    assert_eq!(text(visual), "中文🙂\r\n", "earlier typing retains its own unit");
    visual.simulate_keystrokes("ctrl-y ctrl-y ctrl-y");
    assert_eq!(text(visual), "a生成b中文🙂\r\n");
    assert_eq!(std::fs::read_to_string(directory.path().join("proposal.txt")).unwrap(), "中文🙂\r\n");
    assert!(visual.update(|_, cx| app.read(cx).tabs[0].is_dirty()));
}
